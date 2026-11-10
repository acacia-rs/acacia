//! A mount the player drives, as the vanilla client ticks it before its rider: `LivingEntity.tick`, with
//! `travelRidden` in `aiStep`. The body moves by [`Player`] physics with the mount's size and speed.

use acacia_registry::BlockTag;

use crate::aabb::Aabb;
use crate::collide::Blocks;
use crate::fluid::fluid_at;
use crate::lerp::Lerp;
use crate::mount_body::MountBody;
use crate::mth::{cos, sin, DEG_TO_RAD};
use crate::player::Player;

/// `Avatar.DEFAULT_VEHICLE_ATTACHMENT`.
const PLAYER_ATTACHMENT: f64 = 0.6;

/// A player's feet on a vehicle at `pos` turned to `yaw`, from the vehicle's passenger attachment `seat`.
pub fn seat_feet(pos: [f64; 3], seat: [f64; 3], yaw: f32) -> [f64; 3] {
    let radians = f64::from(-yaw * DEG_TO_RAD);
    let (s, c) = (f64::from(sin(radians)), f64::from(cos(radians)));
    let [x, y, z] = seat;
    [pos[0] + x * c + z * s, pos[1] + y - PLAYER_ATTACHMENT, pos[2] + z * c - x * s]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountKind {
    /// Horses, donkeys, mules and zombie horses, which float when ridden through water.
    Horse,
    SkeletonHorse,
    Camel,
    Pig,
    Strider,
    HappyGhast,
    Nautilus,
}

/// The rider as its last `aiStep` left it, which the mount reads on its tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rider {
    pub xxa: f32,
    pub zza: f32,
    pub jumping: bool,
    pub sprinting: bool,
    pub yaw: f32,
    pub pitch: f32,
}

/// `ItemBasedSteering` of a pig or strider: a boost the server starts through `DATA_BOOST_TIME`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Boost {
    pub time: i32,
    pub total: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mount {
    pub kind: MountKind,
    pub body: Player,
    /// Synced attributes; the strider's suffocation modifier is applied on top.
    pub movement_speed: f64,
    pub jump_strength: f64,
    pub flying_speed: f64,
    /// Where the rider's seat sits relative to the mount's feet, before turning by its yaw.
    pub seat: [f64; 3],
    /// A horse rearing (flag 32, or set here by a rider's jump).
    pub standing: bool,
    pub stand_counter: i32,
    pub boost: Option<Boost>,
    pub dash_cooldown: i32,
    pub dashing: bool,
    /// A camel's `LAST_POSE_CHANGE_TICK` (negative while sitting) and the client's game time.
    pub last_pose_change: i64,
    pub game_time: i64,
    pub(crate) jump_pending: f32,
    pub(crate) allow_stand_sliding: bool,
    pub(crate) suffocating: bool,
    lerp: Lerp,
}

impl Mount {
    /// The mount an entity type drives as, with its default attributes; `None` for one the client doesn't drive.
    /// `seat` is the type's first passenger attachment.
    pub fn new(name: &str, size: (f32, f32), seat: [f32; 3], pos: [f64; 3], yaw: f32) -> Option<Mount> {
        use MountKind::*;
        let (kind, speed, jump) = match name {
            "horse" | "zombie_horse" => (Horse, 0.225_f32, 0.7),
            "donkey" | "mule" => (Horse, 0.175, 0.5),
            "skeleton_horse" => (SkeletonHorse, 0.2, 0.7),
            "camel" | "camel_husk" => (Camel, 0.09, 0.42_f32 as f64),
            "pig" => (Pig, 0.25, 0.0),
            "strider" => (Strider, 0.175, 0.0),
            "happy_ghast" => (HappyGhast, 0.05, 0.0),
            "nautilus" => (Nautilus, 1.0, 0.0),
            "zombie_nautilus" => (Nautilus, 1.1, 0.0),
            _ => return None,
        };
        // A lone camel rider sits on its standing body anchor, half a block forward.
        let seat = if kind == Camel { [0.0, size.1 - 0.375, 0.5] } else { seat };
        let mut body = Player { pos, yaw, ..Player::default() };
        body.mount = Some(MountBody {
            width: size.0,
            height: size.1,
            step: if kind == Camel { 1.5 } else { 1.0 },
            speed: 0.0,
            input: [0.0; 3],
            climbs: matches!(kind, Pig | Strider | Nautilus),
            water_slowdown: if kind == SkeletonHorse { 0.96 } else { 0.8 },
            pushed_by_fluids: kind != Nautilus,
            stands_on_lava: kind == Strider,
        });
        Some(Mount {
            kind,
            body,
            movement_speed: speed as f64,
            jump_strength: jump,
            flying_speed: 0.05_f32 as f64,
            seat: seat.map(f64::from),
            standing: false,
            stand_counter: 0,
            boost: None,
            dash_cooldown: 0,
            dashing: false,
            last_pose_change: 0,
            game_time: 0,
            jump_pending: 0.0,
            allow_stand_sliding: false,
            suffocating: false,
            lerp: Lerp::default(),
        })
    }

    /// `positionRider`: the rider's feet on the turned seat.
    // TODO: a rearing horse's stand animation and a strider's walk bob shift the seat; neither is modelled.
    pub fn rider_feet(&self) -> [f64; 3] {
        seat_feet(self.body.pos, self.seat, self.body.yaw)
    }

    pub fn interpolate_to(&mut self, pos: [f64; 3], yaw: f32, pitch: f32) {
        let b = &self.body;
        self.lerp.start((b.pos, b.yaw, b.pitch), (pos, yaw, pitch));
    }

    /// One tick under `rider`, whose input is last tick's.
    pub fn tick(&mut self, world: &impl Blocks, rider: Rider) {
        self.interpolate(world);
        if self.kind == MountKind::Strider {
            self.suffocating = !self.warm(world);
        }
        self.body.base_tick(world);
        self.body.delta = self.body.delta.map(|v| if v.abs() < 0.003 { 0.0 } else { v });
        let input = self.ridden_input(rider);
        self.tick_ridden(world, rider, input);
        let speed = self.ridden_speed(rider);
        let body = self.body.mount.as_mut().expect("a mount body");
        (body.speed, body.input) = (speed, input);
        self.travel(world);
        self.body.apply_block_effects(world);
        self.after_tick(world);
    }

    fn interpolate(&mut self, world: &impl Blocks) {
        let mut pose = (self.body.pos, self.body.yaw, self.body.pitch);
        let (width, height) = self.body.size();
        let colliders = self.body.colliders(world);
        self.lerp.step(&mut pose, |at| colliders.no_collision(&Aabb::of_entity(at, width, height)));
        (self.body.pos, self.body.yaw, self.body.pitch) = pose;
    }

    /// `Strider.tick`'s warmth: lava in or under it.
    fn warm(&self, world: &impl Blocks) -> bool {
        let warm = |pos| world.block_state(pos).block().is(BlockTag::StriderWarmBlocks);
        warm(self.body.block_pos()) || warm(self.body.on_pos(world, crate::support::LEGACY_ON_POS)) || self.body.in_lava()
    }

    /// `LivingEntity.travel`, or the flying and swimming overrides.
    fn travel(&mut self, world: &impl Blocks) {
        if self.kind == MountKind::Camel && self.refuses_to_move() && self.body.on_ground {
            self.body.delta = [0.0, self.body.delta[1], 0.0];
            let body = self.body.mount.as_mut().expect("a mount body");
            body.input = [0.0, body.input[1], 0.0];
        }
        let body = &mut self.body;
        let stands_here = self.kind == MountKind::Strider && fluid_at(world, body.block_pos()).is_some_and(|f| f.lava);
        let in_fluid = (body.in_water || body.in_lava()) && !stands_here;
        match self.kind {
            MountKind::HappyGhast => self.travel_flying(world),
            MountKind::Nautilus if in_fluid && body.in_water => {
                body.move_relative(body.speed());
                body.move_self(world);
                body.delta = body.delta.map(|v| v * 0.9);
            }
            _ if in_fluid => {
                body.travel_in_fluid(world);
                let floats = self.kind == MountKind::Horse || self.kind == MountKind::Camel;
                if floats && body.fluids.height(false) > body.jump_threshold() {
                    body.add_y(0.04_f32 as f64);
                }
            }
            _ => body.travel_in_air(world),
        }
    }

    /// `HappyGhast.travel`: `travelFlying` at five thirds of its flying speed, with no gravity.
    fn travel_flying(&mut self, world: &impl Blocks) {
        let speed = (self.flying_speed as f32) * 5.0 / 3.0;
        let body = &mut self.body;
        let drag = if body.in_water { 0.8_f32 as f64 } else if body.in_lava() { 0.5 } else { 0.91_f32 as f64 };
        body.move_relative(speed);
        body.move_self(world);
        body.delta = body.delta.map(|v| v * drag);
    }

    /// The type's own `tick` after `LivingEntity.tick`.
    fn after_tick(&mut self, world: &impl Blocks) {
        match self.kind {
            MountKind::Horse | MountKind::SkeletonHorse | MountKind::Camel => {
                if self.stand_counter > 0 {
                    self.stand_counter -= 1;
                    if self.stand_counter <= 0 {
                        self.standing = false;
                    }
                }
                if !self.standing {
                    self.allow_stand_sliding = false;
                }
            }
            _ => {}
        }
        match self.kind {
            MountKind::Camel => {
                let in_liquid = self.body.in_water || self.body.in_lava();
                if self.dashing && self.dash_cooldown < 50 && (self.body.on_ground || in_liquid) {
                    self.dashing = false;
                }
                self.dash_cooldown = (self.dash_cooldown - 1).max(0);
            }
            MountKind::Nautilus => {
                if self.dashing && self.dash_cooldown < 35 {
                    self.dashing = false;
                }
                self.dash_cooldown = (self.dash_cooldown - 1).max(0);
            }
            MountKind::Strider => self.float_strider(world),
            _ => {}
        }
    }

    /// `Strider.floatStrider`, after the tick's move: it holds the surface or bobs up out of the lava.
    fn float_strider(&mut self, world: &impl Blocks) {
        let body = &mut self.body;
        if !body.in_lava() {
            return;
        }
        let here = body.block_pos();
        let above = fluid_at(world, [here[0], here[1] + 1, here[2]]).is_some_and(|f| f.lava);
        if body.pos[1] > here[1] as f64 + 0.5 - 1.0E-5_f32 as f64 && !above {
            body.on_ground = true;
        } else {
            body.delta = [body.delta[0] * 0.5 + 0.0, body.delta[1] * 0.5 + 0.05, body.delta[2] * 0.5 + 0.0];
        }
    }
}

#[cfg(test)]
#[path = "mount_tests.rs"]
mod tests;
