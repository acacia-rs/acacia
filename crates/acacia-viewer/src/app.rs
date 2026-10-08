//! The viewer's state, its frame loop and the title-bar overlay. Window events: app/window.rs; keys
//! and modes: app/keys.rs.

mod keys;
mod window;

use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_render::blocks::BlockTable;
use acacia_render::item::ItemModels;
use acacia_render::sky::{DAY_TICKS, SkyTextures, moon_phase};
use acacia_render::{Camera, FrameStats, Outline, Renderer};
use acacia_world::World;
use glam::DVec3;
use winit::window::{CursorGrabMode, Window};

use crate::debug_lines::{self, Facts};
use crate::input::FlyInput;
use crate::looks::Looks;
use crate::net::{Net, NetEvent};
use crate::player::Play;
use crate::settings::Settings;
use crate::shot::Shot;
use crate::smooth::Smoother;
use crate::ui::Ui;

const TITLE_EVERY: Duration = Duration::from_millis(500);

/// Who moves the camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The camera is the player's eye; input drives the bot.
    Play,
    /// A free camera; the bot stands where it is.
    Fly,
}

pub struct App {
    net: Net,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    camera: Camera,
    mode: Mode,
    input: FlyInput,
    play: Play,
    ui: Ui,
    /// F3.
    show_debug: bool,
    /// The last frame's figures and the frame rate, for the debug screen.
    stats: FrameStats,
    fps: f32,
    grabbed: bool,
    settings: Settings,
    looks: Looks,
    /// The shown world's block table, for picking.
    table: Option<Arc<BlockTable>>,
    fog_distance: f32,
    player: Option<DVec3>,
    entities: Smoother,
    /// The server's world time, once it sent one.
    time: Option<i32>,
    sky: Option<SkyTextures>,
    camera_placed: bool,
    status: String,
    last_frame: Instant,
    overlay: Overlay,
    shot: Option<Shot>,
    /// Why the bot's session ended before an unattended screenshot was taken; the viewer exits.
    pub failed: Option<String>,
}

/// Frame-rate and memory figures shown in the title.
struct Overlay {
    since: Instant,
    frames: u32,
    reports: u32,
}

/// Share of the gap to the server's time of day closed per second.
const TIME_EASE: f32 = 3.0;

/// Title updates between stats lines in the log (5 s).
const LOG_EVERY_TITLES: u32 = 10;

impl App {
    pub fn new(net: Net, radius: i32, sky: Option<SkyTextures>, settings: Settings, looks: Looks) -> Self {
        let play = Play::new(net.commands.clone());
        // Unattended screenshots place the camera themselves.
        let shot = Shot::from_env();
        let mode = if shot.is_some() { Mode::Fly } else { Mode::Play };
        let ui = Ui::new(&looks);
        App {
            ui,
            show_debug: std::env::var_os("ACACIA_DEBUG").is_some(),
            stats: FrameStats::default(),
            fps: 0.0,
            net,
            window: None,
            renderer: None,
            camera: Camera::new(DVec3::new(0.0, 100.0, 0.0)),
            mode,
            input: FlyInput::new(),
            play,
            grabbed: false,
            settings,
            looks,
            table: None,
            fog_distance: (radius * 16) as f32,
            player: None,
            entities: Smoother::default(),
            time: None,
            sky,
            camera_placed: false,
            status: "starting".into(),
            last_frame: Instant::now(),
            overlay: Overlay { since: Instant::now(), frames: 0, reports: 0 },
            shot,
            failed: None,
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
            tracing::info!(entities = self.entities.instances(self.camera.position).len(), "shot");
            r.screenshot(shot.path.clone());
            shot.taken = true;
        }
        false
    }

    /// Draws `world` with the chosen look, meshing it anew.
    fn show_world(&mut self, world: Arc<World>) {
        let Some(r) = &mut self.renderer else { return };
        let pack = self.looks.get(self.settings.look);
        r.look = pack.look;
        let table = Arc::new(pack.block_table(world.registry()));
        self.entities.set_items(ItemModels::new(pack.clone(), table.clone()));
        self.table = Some(table.clone());
        r.set_world(world, table, &pack.atlas);
    }

    fn poll_net(&mut self) {
        while let Ok(event) = self.net.events.try_recv() {
            match event {
                NetEvent::World(world) => {
                    self.show_world(world);
                    if let Some(shot) = &mut self.shot {
                        shot.world_at.get_or_insert_with(Instant::now);
                    }
                }
                NetEvent::Biomes(colors) => {
                    if let Some(r) = &mut self.renderer {
                        r.set_biomes(colors);
                    }
                }
                NetEvent::Player(p) => {
                    self.player = Some(p);
                    if !self.camera_placed {
                        self.camera.position = p;
                        self.camera_placed = true;
                    }
                }
                NetEvent::Me(me) => self.play.tick(me),
                NetEvent::Chat { sender, message, params } => self.ui.push_chat(sender.as_deref(), &message, &params),
                NetEvent::EntityModels(models) => {
                    self.entities.set_models(models.clone());
                    if let Some(r) = &mut self.renderer {
                        r.set_entity_models(models);
                    }
                }
                NetEvent::Entities(snapshot) => self.entities.push(snapshot),
                NetEvent::Time(time) => self.time = Some(time),
                NetEvent::BlockData(data) => {
                    if let Some(r) = &mut self.renderer {
                        r.set_block_data(data);
                    }
                }
                NetEvent::Status(s) => {
                    tracing::info!("{s}");
                    self.status = s;
                }
                NetEvent::Ended(reason) => {
                    tracing::warn!("session ended: {reason}");
                    if self.shot.as_ref().is_some_and(|s| !s.taken) {
                        self.failed = Some(reason.clone());
                    }
                    self.status = reason;
                }
            }
        }
    }

    /// Moves the camera by the mode's input; in play, finds the targeted block.
    fn steer(&mut self, now: Instant, dt: f32) -> Option<Outline> {
        match self.mode {
            Mode::Fly => {
                self.input.step(&mut self.camera, dt);
                None
            }
            Mode::Play => {
                let world = self.renderer.as_ref().and_then(|r| r.world().cloned());
                let entities = self.entities.hitboxes();
                self.play.frame(&mut self.camera, world.as_deref(), self.table.as_deref(), &entities, now);
                let mining = self.play.me.as_ref().and_then(|m| m.mining);
                self.play.target.as_ref().map(|t| {
                    let crack = mining.filter(|(block, _)| *block == t.block).map(|(_, progress)| (progress * 10.0).clamp(0.0, 9.0) as u8);
                    Outline { block: t.block, boxes: t.boxes.clone(), crack }
                })
            }
        }
    }

    fn frame(&mut self) {
        self.poll_net();
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        let outline = self.steer(now, dt);
        let Some(r) = &mut self.renderer else { return };
        r.set_outline(outline);
        r.fog_distance = self.fog_distance;
        r.cave_culling = self.settings.cave_culling;
        if let Some(time) = self.time {
            // The server sends the time every few seconds: ease towards it, the short way round the day.
            let ahead = (time as f32 - r.time + DAY_TICKS / 2.0).rem_euclid(DAY_TICKS) - DAY_TICKS / 2.0;
            r.time += ahead * (dt * TIME_EASE).min(1.0);
            r.moon_phase = moon_phase(i64::from(time));
        }
        self.camera.aspect = r.aspect();
        r.set_entities(self.entities.instances(self.camera.position));
        let size = self.window.as_ref().map_or([1, 1], |w| w.inner_size().into());
        let scale = acacia_ui::scale::gui_scale(size[0], size[1], self.settings.gui_scale);
        let world = r.world().cloned();
        let debug = self.show_debug.then(|| {
            let target = self.play.target.as_ref().filter(|_| self.mode == Mode::Play);
            let facts = Facts { camera: &self.camera, fps: self.fps, stats: &self.stats, look: self.settings.look, mode: self.mode, target, world: world.as_deref() };
            debug_lines::lines(&facts)
        });
        let ui = self.ui.draw(self.settings.look, self.play.me.as_ref(), debug, size, scale, now);
        let view = match self.mode == Mode::Play && self.play.view_turned() {
            true => Camera { yaw: self.camera.yaw + std::f32::consts::PI, pitch: -self.camera.pitch, ..self.camera },
            false => self.camera,
        };
        let stats = r.render(&view, Some(ui));
        self.stats = stats;
        self.overlay.frames += 1;
        let elapsed = self.overlay.since.elapsed();
        if elapsed >= TITLE_EVERY {
            let fps = self.overlay.frames as f32 / elapsed.as_secs_f32();
            self.fps = fps;
            let line = crate::overlay::title(fps, &stats, &self.status);
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
            self.play.release_all();
        }
    }
}
