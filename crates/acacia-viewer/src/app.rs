//! Window, event handling and the title-bar overlay.

use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_render::{Camera, FrameStats, Renderer};
use glam::DVec3;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::input::FlyInput;
use crate::net::{Net, NetEvent};

const TITLE_EVERY: Duration = Duration::from_millis(500);

pub struct App {
    net: Net,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    camera: Camera,
    input: FlyInput,
    grabbed: bool,
    vsync: bool,
    fog_distance: f32,
    player: Option<DVec3>,
    camera_placed: bool,
    status: String,
    last_frame: Instant,
    overlay: Overlay,
    shot: Option<Shot>,
}

/// Unattended capture (`ACACIA_SCREENSHOT=out.png`, `ACACIA_SHOT_AFTER=secs` after the world
/// arrives, `ACACIA_LOOK=yaw,pitch` in degrees, `ACACIA_RISE=blocks` above the bot); exits after.
struct Shot {
    path: std::path::PathBuf,
    after: Duration,
    world_at: Option<Instant>,
    taken: bool,
}

impl Shot {
    fn from_env() -> Option<Shot> {
        let path = std::env::var_os("ACACIA_SCREENSHOT")?.into();
        let after = std::env::var("ACACIA_SHOT_AFTER").ok().and_then(|s| s.parse().ok()).unwrap_or(15.0);
        Some(Shot { path, after: Duration::from_secs_f32(after), world_at: None, taken: false })
    }

    fn place(camera: &mut Camera, player: DVec3) {
        let rise: f64 = std::env::var("ACACIA_RISE").ok().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        camera.position = player + DVec3::Y * rise;
        if let Some((yaw, pitch)) = std::env::var("ACACIA_LOOK").ok().and_then(|s| {
            let (y, p) = s.split_once(',')?;
            Some((y.parse::<f32>().ok()?, p.parse::<f32>().ok()?))
        }) {
            (camera.yaw, camera.pitch) = (yaw.to_radians(), pitch.to_radians());
        }
    }
}

/// Frame-rate and memory figures shown in the title.
struct Overlay {
    since: Instant,
    frames: u32,
    reports: u32,
}

/// Title updates between stats lines in the log (5 s).
const LOG_EVERY_TITLES: u32 = 10;

impl App {
    pub fn new(net: Net, radius: i32) -> Self {
        App {
            net,
            window: None,
            renderer: None,
            camera: Camera::new(DVec3::new(0.0, 100.0, 0.0)),
            input: FlyInput::new(),
            grabbed: false,
            vsync: true,
            fog_distance: (radius * 16) as f32,
            player: None,
            camera_placed: false,
            status: "starting".into(),
            last_frame: Instant::now(),
            overlay: Overlay { since: Instant::now(), frames: 0, reports: 0 },
            shot: Shot::from_env(),
        }
    }

    /// Returns true once the unattended screenshot was taken and the viewer should exit.
    fn drive_shot(&mut self) -> bool {
        let (Some(shot), Some(r)) = (&mut self.shot, &mut self.renderer) else { return false };
        if shot.taken {
            return true;
        }
        if shot.world_at.is_some_and(|t| t.elapsed() >= shot.after) {
            if let Some(p) = self.player {
                Shot::place(&mut self.camera, p);
            }
            r.screenshot(shot.path.clone());
            shot.taken = true;
        }
        false
    }

    fn poll_net(&mut self) {
        while let Ok(event) = self.net.events.try_recv() {
            match event {
                NetEvent::World { world, table, textures } => {
                    if let Some(r) = &mut self.renderer {
                        r.set_world(world, table, &textures);
                    }
                    if let Some(shot) = &mut self.shot {
                        shot.world_at.get_or_insert_with(Instant::now);
                    }
                }
                NetEvent::Player(p) => {
                    self.player = Some(p);
                    if !self.camera_placed {
                        self.camera.position = p;
                        self.camera_placed = true;
                    }
                }
                NetEvent::Status(s) => {
                    tracing::info!("{s}");
                    self.status = s;
                }
            }
        }
    }

    fn frame(&mut self) {
        self.poll_net();
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        self.input.step(&mut self.camera, dt);
        let Some(r) = &mut self.renderer else { return };
        r.fog_distance = self.fog_distance;
        self.camera.aspect = r.aspect();
        let stats = r.render(&self.camera);
        self.overlay.frames += 1;
        let elapsed = self.overlay.since.elapsed();
        if elapsed >= TITLE_EVERY {
            let fps = self.overlay.frames as f32 / elapsed.as_secs_f32();
            let line = title(fps, &stats, &self.status);
            if let Some(w) = &self.window {
                w.set_title(&line);
            }
            self.overlay.reports += 1;
            if self.overlay.reports.is_multiple_of(LOG_EVERY_TITLES) {
                tracing::info!("{line}");
            }
            self.overlay = Overlay { since: now, frames: 0, reports: self.overlay.reports };
        }
    }

    fn grab(&mut self, on: bool) {
        let Some(w) = &self.window else { return };
        let mode = if on { CursorGrabMode::Locked } else { CursorGrabMode::None };
        let grabbed = w.set_cursor_grab(mode).or_else(|_| w.set_cursor_grab(if on { CursorGrabMode::Confined } else { mode })).is_ok();
        w.set_cursor_visible(!on);
        self.grabbed = on && grabbed;
        if !on {
            self.input.release_all();
        }
    }

    fn key(&mut self, code: KeyCode, pressed: bool) {
        match (code, pressed) {
            (KeyCode::Escape, true) => self.grab(false),
            (KeyCode::KeyF, true) => {
                if let Some(p) = self.player {
                    self.camera.position = p;
                }
            }
            (KeyCode::KeyV, true) => {
                self.vsync = !self.vsync;
                if let Some(r) = &mut self.renderer {
                    r.set_vsync(self.vsync);
                }
            }
            _ => self.input.key(code, pressed),
        }
    }
}

fn title(fps: f32, s: &FrameStats, status: &str) -> String {
    format!(
        "Acacia | {fps:.0} fps | {} | {} MB GPU buffers | {} sections ({} drawn), {}k quads | {} meshing | {status}",
        memory(),
        s.gpu_bytes >> 20,
        s.sections,
        s.drawn,
        s.quads / 1000,
        s.pending,
    )
}

/// Working set (what Windows keeps resident; it trims this freely) and committed private memory.
fn memory() -> String {
    let (ws, commit) = memory_stats::memory_stats().map_or((0, 0), |m| (m.physical_mem >> 20, m.virtual_mem >> 20));
    #[cfg(feature = "profile")]
    {
        let heap: Vec<String> = crate::heap::live().iter().zip(crate::heap::ROLES).map(|(b, r)| format!("{r} {:.1}", *b as f64 / 1048576.0)).collect();
        format!("{ws} MB resident, {commit} MB committed, heap MB: {}", heap.join(" "))
    }
    #[cfg(not(feature = "profile"))]
    format!("{ws} MB resident, {commit} MB committed")
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("Acacia").with_inner_size(winit::dpi::LogicalSize::new(1280, 720));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let size = window.inner_size();
        match Renderer::new(window.clone(), (size.width, size.height)) {
            Ok(r) => self.renderer = Some(r),
            Err(e) => {
                tracing::error!(%e, "renderer");
                event_loop.exit();
            }
        }
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if self.drive_shot() {
                    event_loop.exit();
                }
                self.frame();
            }
            WindowEvent::Focused(false) => self.grab(false),
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } if !self.grabbed => self.grab(true),
            WindowEvent::MouseWheel { delta, .. } => self.input.scroll(match delta {
                MouseScrollDelta::LineDelta(_, y) => y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
            }),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(code, event.state == ElementState::Pressed);
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let (DeviceEvent::MouseMotion { delta: (dx, dy) }, true) = (event, self.grabbed) {
            self.input.mouse(&mut self.camera, dx, dy);
        }
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.net.shutdown();
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}
