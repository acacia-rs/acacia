//! The viewer's state, its frame loop and the title-bar overlay. Window events: app/window.rs; keys
//! and modes: app/keys.rs.

mod events;
mod form;
mod keys;
mod menu;
mod screen;
mod view;
mod window;

use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_render::blocks::BlockTable;
use acacia_render::item::ItemModels;
use acacia_render::sky::{DAY_TICKS, SkyTextures, moon_phase};
use acacia_render::{Camera, FrameStats, Renderer};
use acacia_world::World;
use glam::DVec3;
use winit::window::{CursorGrabMode, Window};

use crate::audio::Audio;
use crate::control::Inventory;
use crate::debug_lines::{self, Facts};
use crate::forms::{FormScreen, Images};
use crate::input::FlyInput;
use crate::looks::Looks;
use crate::net::Net;
use crate::player::Play;
use crate::settings::{LookChoice, Settings};
use crate::shot::Shot;
use crate::smooth::Smoother;
use crate::ui::{Frame, Ui};
use menu::Menu;

const TITLE_EVERY: Duration = Duration::from_millis(500);
/// A frame's work before drawing that takes longer than this is logged, by stage.
const SLOW_FRAME: Duration = Duration::from_millis(250);

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
    audio: Audio,
    /// F3.
    show_debug: bool,
    /// Tab held: the player list shows.
    show_players: bool,
    players: Vec<String>,
    /// The inventory screen is open (E).
    screen_open: bool,
    /// The pause menu or options (Esc).
    menu: Option<Menu>,
    /// Disconnect was chosen: the window closes.
    pub quit: bool,
    inventory: Inventory,
    /// Cursor position in window pixels, and Shift held.
    mouse: [f64; 2],
    shift: bool,
    /// `ACACIA_ATTACK`: hold attack on whatever is targeted (unattended mining).
    auto_attack: bool,
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
    /// The server form shown, and its button images.
    form: Option<FormScreen>,
    form_images: Images,
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
        let mode = if shot.is_some() && std::env::var_os("ACACIA_PLAY").is_none() { Mode::Fly } else { Mode::Play };
        let (ui, audio) = (Ui::new(&looks), Audio::new(&looks));
        // Form button images name textures of the Bedrock pack, whichever look is shown.
        let bedrock_pack = looks.get(LookChoice::Bedrock).files().to_owned();
        App {
            ui,
            audio,
            show_debug: std::env::var_os("ACACIA_DEBUG").is_some(),
            show_players: false,
            players: Vec::new(),
            // For unattended screenshots of the inventory screen.
            screen_open: std::env::var_os("ACACIA_SCREEN").is_some(),
            // For unattended screenshots of the pause menu.
            menu: std::env::var_os("ACACIA_MENU").map(|_| Menu::Pause),
            quit: false,
            inventory: Inventory::default(),
            mouse: [0.0; 2],
            shift: false,
            auto_attack: std::env::var_os("ACACIA_ATTACK").is_some(),
            stats: FrameStats::default(),
            fps: 0.0,
            net,
            window: None,
            renderer: None,
            camera: {
                let mut camera = Camera::new(DVec3::new(0.0, 100.0, 0.0));
                Shot::look(&mut camera);
                camera
            },
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
            form: App::shot_form(),
            form_images: Images::new(&bedrock_pack),
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
        self.ui.set_world(self.settings.look, pack.clone(), table.clone());
        r.set_world(world, table, &pack.atlas);
    }

    fn frame(&mut self) {
        let start = Instant::now();
        self.poll_net();
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        let outline = self.steer(now, dt);
        let steered = Instant::now();
        let hand = self.hand(now);
        let held = Instant::now();
        if held - start > SLOW_FRAME {
            tracing::warn!(net = ?(now - start), steer = ?(steered - now), hand = ?(held - steered), "slow frame");
        }
        let Some(r) = &mut self.renderer else { return };
        r.set_outline(outline);
        r.fog_distance = self.fog_distance;
        r.cave_culling = self.settings.cave_culling;
        r.rain = self.play.me.as_ref().map_or(0.0, |m| m.rain);
        if let Some(time) = self.time {
            // The server sends the time every few seconds: ease towards it, the short way round the day.
            let ahead = (time as f32 - r.time + DAY_TICKS / 2.0).rem_euclid(DAY_TICKS) - DAY_TICKS / 2.0;
            r.time += ahead * (dt * TIME_EASE).min(1.0);
            r.moon_phase = moon_phase(i64::from(time));
        }
        self.camera.aspect = r.aspect();
        let mut instances = self.entities.instances(self.camera.position);
        instances.extend(hand);
        r.set_entities(instances);
        let world = r.world().cloned();
        let debug = self.show_debug.then(|| {
            let target = self.play.target.as_ref().filter(|_| self.mode == Mode::Play);
            let facts = Facts { camera: &self.camera, fps: self.fps, stats: &self.stats, look: self.settings.look, mode: self.mode, target, world: world.as_deref() };
            debug_lines::lines(&facts)
        });
        self.update_form();
        let (size, scale) = self.gui();
        let mouse = self.gui_mouse();
        let screen = self.screen_open.then_some((&self.inventory, self.layout()));
        let menu = self.menu.map(|m| self.menu_content(m));
        let menu = menu.as_ref().map(|(title, buttons)| (*title, buttons.as_slice()));
        let tags = self.name_tags([(size[0] / scale) as f32, (size[1] / scale) as f32], scale as f32);
        let Some(r) = &mut self.renderer else { return };
        let form = self.form.as_ref();
        let players = self.show_players.then_some(self.players.as_slice());
        let frame = Frame { look: self.settings.look, me: self.play.me.as_ref(), debug, screen, menu, form, players, tags, mouse, size, scale, now };
        let ui = self.ui.draw(frame);
        let view = match self.mode == Mode::Play && self.play.view_turned() {
            true => Camera { yaw: self.camera.yaw + std::f32::consts::PI, pitch: -self.camera.pitch, ..self.camera },
            false => self.camera,
        };
        let drawing = Instant::now();
        let stats = r.render(&view, Some(ui));
        if drawing.elapsed() > SLOW_FRAME {
            tracing::warn!(ui = ?(drawing - held), render = ?drawing.elapsed(), "slow draw");
        }
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
