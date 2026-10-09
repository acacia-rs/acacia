//! Playing from the window: keys and mouse become the bot's controls and clicks, and the camera
//! sits at the player's eye, interpolated between ticks.

use std::time::{Duration, Instant};

use acacia_bot::interact::{BLOCK_REACH, ENTITY_REACH};
use acacia_bot::movement::Controls;
use acacia_render::Camera;
use acacia_render::blocks::BlockTable;
use acacia_world::World;
use glam::DVec3;
use tokio::sync::mpsc::UnboundedSender;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

use crate::control::{Command, Me};
use crate::pick::{self, EntityBox, Target};

const MOUSE_SENSITIVITY: f32 = 0.0025;
/// One client tick.
const TICK: Duration = Duration::from_millis(50);
/// A Space press is reported this long at least, so a quick tap reaches a 50 ms movement tick: BDS
/// toggles flight on two presses it sees (README "Flight" in acacia-physics).
const JUMP_LATCH: Duration = Duration::from_millis(70);
/// Java's `rightClickDelay`: a held use button repeats every 4 ticks.
const USE_REPEAT: Duration = Duration::from_millis(200);
const HOTBAR_KEYS: [KeyCode; 9] = [
    KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5,
    KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
];

pub struct Play {
    commands: UnboundedSender<Command>,
    held: Vec<KeyCode>,
    sent: Option<Controls>,
    eye: Option<EyeTrack>,
    pub me: Option<Me>,
    pub target: Option<Target>,
    /// The entity under the crosshair, when it is nearer than any block.
    entity: Option<u64>,
    attacking: bool,
    /// When the use button was last acted on while held.
    using: Option<Instant>,
    /// When a use in the air began (eating, drawing a bow): releasing the button sends `ReleaseItem`.
    using_item: Option<Instant>,
    /// 0 first person, 1 behind, 2 in front.
    perspective: u8,
    /// When the arm last started a swing.
    swung: Option<Instant>,
    /// When Space was last pressed (see `JUMP_LATCH`).
    jumped: Option<Instant>,
}

/// Java's arm swing: 6 ticks.
const SWING: Duration = Duration::from_millis(300);

/// Blocks between the eye and a third-person camera.
const THIRD_PERSON_DISTANCE: f32 = 4.0;

/// The eye position between two ticks.
struct EyeTrack {
    from: DVec3,
    to: DVec3,
    at: Instant,
}

impl EyeTrack {
    fn at(&self, now: Instant) -> DVec3 {
        let t = (now - self.at).as_secs_f64() / TICK.as_secs_f64();
        self.from.lerp(self.to, t.min(1.0))
    }
}

impl Play {
    pub fn new(commands: UnboundedSender<Command>) -> Self {
        Play { commands, held: Vec::new(), sent: None, eye: None, me: None, target: None, entity: None, attacking: false, using: None, using_item: None, perspective: 0, swung: None, jumped: None }
    }

    fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// The player after a tick: the eye moves there over the next tick.
    pub fn tick(&mut self, me: Me) {
        let now = Instant::now();
        let from = self.eye.as_ref().map_or(me.eye, |e| e.at(now));
        self.eye = Some(EyeTrack { from, to: me.eye, at: now });
        self.me = Some(me);
    }

    pub fn eye(&self, now: Instant) -> Option<DVec3> {
        self.eye.as_ref().map(|e| e.at(now))
    }

    pub fn key(&mut self, key: KeyCode, pressed: bool) {
        self.held.retain(|&k| k != key);
        if pressed {
            self.held.push(key);
            if key == KeyCode::Space {
                self.jumped = Some(Instant::now());
            }
            if key == KeyCode::ShiftLeft && self.riding() {
                self.send(Command::Dismount);
            }
            if let Some(slot) = HOTBAR_KEYS.iter().position(|&k| k == key) {
                self.send(Command::Hotbar(slot as u8));
            }
        }
    }

    fn riding(&self) -> bool {
        self.me.as_ref().is_some_and(|m| m.riding)
    }

    pub fn release_all(&mut self) {
        self.held.clear();
        self.button(MouseButton::Left, false);
        self.button(MouseButton::Right, false);
    }

    pub fn mouse(&self, camera: &mut Camera, dx: f64, dy: f64) {
        camera.look(dx as f32 * MOUSE_SENSITIVITY, dy as f32 * MOUSE_SENSITIVITY);
    }

    /// Wheel down moves right along the hotbar, as in both games.
    pub fn scroll(&self, lines: f32) {
        let Some(me) = &self.me else { return };
        let step = if lines < 0.0 { 1 } else if lines > 0.0 { 8 } else { return };
        self.send(Command::Hotbar((me.hotbar + step) % 9));
    }

    pub fn button(&mut self, button: MouseButton, pressed: bool) {
        match (button, pressed) {
            (MouseButton::Left, true) => {
                self.attacking = true;
                match (self.entity, &self.target) {
                    (Some(runtime_id), _) => self.send(Command::Entity { runtime_id, attack: true }),
                    (None, Some(t)) => self.send(Command::Mine(Some((t.block, t.face)))),
                    (None, None) => self.send(Command::Swing),
                }
            }
            (MouseButton::Left, false) if self.attacking => {
                self.attacking = false;
                self.send(Command::Mine(None));
            }
            (MouseButton::Right, true) => {
                self.using = Some(Instant::now());
                self.use_once();
            }
            (MouseButton::Right, false) if self.using.is_some() => {
                self.using = None;
                if self.using_item.take().is_some() {
                    self.send(Command::ReleaseItem);
                }
            }
            _ => {}
        }
    }

    fn use_once(&mut self) {
        match (self.entity, &self.target) {
            (Some(runtime_id), _) => self.send(Command::Entity { runtime_id, attack: false }),
            (None, Some(t)) => self.send(Command::UseOn(t.block, t.face)),
            (None, None) if self.using_item.is_none() => {
                self.using_item = Some(Instant::now());
                self.send(Command::UseItem);
            }
            (None, None) => {}
        }
    }

    /// How far through a swing the arm is (0 at rest); a held attack or use keeps it swinging.
    pub fn swing(&mut self, now: Instant) -> f32 {
        if (self.attacking || self.using.is_some()) && self.swung.is_none_or(|s| now - s >= SWING) {
            self.swung = Some(now);
        }
        self.swung.map_or(0.0, |s| ((now - s).as_secs_f32() / SWING.as_secs_f32()).min(1.0) % 1.0)
    }

    /// The held stack in first person; `None` from behind or in front.
    pub fn held_first_person(&self) -> Option<&crate::control::Stack> {
        let me = self.me.as_ref().filter(|_| self.perspective == 0)?;
        me.items.get(usize::from(me.hotbar))?.as_ref()
    }

    /// Seconds the held item has been in use in the air, if it is.
    pub fn item_use_secs(&self, now: Instant) -> Option<f32> {
        self.using_item.map(|since| now.saturating_duration_since(since).as_secs_f32())
    }

    pub fn attacking(&self) -> bool {
        self.attacking
    }

    /// F5: first person, then behind, then in front facing back.
    pub fn next_perspective(&mut self) {
        self.perspective = (self.perspective + 1) % 3;
    }

    /// Moves the camera to the eye (or behind it), finds the target and sends what changed.
    pub fn frame(&mut self, camera: &mut Camera, world: Option<&World>, table: Option<&BlockTable>, entities: &[EntityBox], now: Instant) {
        let Some(eye) = self.eye(now) else { return };
        let aim = camera.forward();
        let mut target = world.zip(table).and_then(|(w, t)| pick::pick(w, t, eye, aim, BLOCK_REACH));
        let entity = pick::pick_entity(entities, eye, aim, ENTITY_REACH)
            .filter(|&(_, at)| target.as_ref().is_none_or(|t| at < t.distance));
        self.entity = entity.map(|(id, _)| id);
        if self.entity.is_some() {
            target = None;
        }
        if self.attacking && target != self.target {
            self.send(Command::Mine(target.as_ref().map(|t| (t.block, t.face))));
        }
        self.target = target;
        if let Some(since) = self.using
            && now - since >= USE_REPEAT
        {
            self.using = Some(now);
            if self.target.is_some() {
                self.use_once();
            }
        }
        let controls = self.controls(camera);
        if self.sent != Some(controls) {
            self.sent = Some(controls);
            self.send(Command::Controls(controls));
        }
        camera.position = eye;
        if self.perspective != 0 {
            // The camera backs off along the look ray, stopping short of blocks (Java's `Camera.getMaxZoom`).
            let back = if self.perspective == 1 { -aim } else { aim };
            let room = world.zip(table).and_then(|(w, t)| pick::pick(w, t, eye, back, THIRD_PERSON_DISTANCE)).map_or(f64::from(THIRD_PERSON_DISTANCE), |hit| hit.distance - 0.1);
            camera.position = eye + back.as_dvec3() * room.max(0.0);
        }
    }

    /// The direction the camera looks: in front view, back at the player.
    pub fn view_turned(&self) -> bool {
        self.perspective == 2
    }

    fn controls(&self, camera: &Camera) -> Controls {
        let down = |k| self.held.contains(&k);
        let axis = |pos, neg| f32::from(u8::from(down(pos))) - f32::from(u8::from(down(neg)));
        Controls {
            forward: axis(KeyCode::KeyW, KeyCode::KeyS),
            strafe: axis(KeyCode::KeyA, KeyCode::KeyD),
            jump: down(KeyCode::Space) || self.jumped.is_some_and(|t| t.elapsed() < JUMP_LATCH),
            // Seated, Shift leaves the vehicle (`Play::key`); BDS refuses mounts while sneaking.
            sneak: down(KeyCode::ShiftLeft) && !self.riding(),
            sprint: down(KeyCode::ControlLeft),
            glide: false,
            fly: false,
            yaw: camera.yaw.to_degrees(),
            pitch: camera.pitch.to_degrees(),
        }
    }
}
