//! `AbstractBoat.tick` as the client runs it for the boat its player drives.

use acacia_registry::{Block, BlockTag};

use crate::aabb::Aabb;
use crate::block_factors::bounce;
use crate::collide::{Blocks, Colliders, Context};
use crate::fluid::FluidScan;
use crate::input::Keys;
use crate::lerp::Lerp;
use crate::inside::Move;
use crate::mth::{cos, length_sqr, sin, wrap_degrees, DEG_TO_RAD};
use crate::support::{Support, LEGACY_ON_POS};

pub const WIDTH: f32 = 1.375;
pub const HEIGHT: f32 = 0.5625;
const GRAVITY: f64 = 0.04;
/// `Avatar.DEFAULT_VEHICLE_ATTACHMENT`: a player's seat is this far above its feet.
const PLAYER_ATTACHMENT: f64 = 0.6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BoatStatus {
    InWater,
    UnderWater,
    UnderFlowingWater,
    OnLand,
    #[default]
    InAir,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Boat {
    pub pos: [f64; 3],
    pub delta: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub horizontal_collision: bool,
    /// Turn speed in degrees a tick; the rider turns with it.
    pub delta_rotation: f32,
    pub status: BoatStatus,
    /// Bamboo rafts seat their rider higher.
    pub raft: bool,
    /// Boxes of nearby entities a vehicle can't pass: solid ones and pushable mobs (`canVehicleCollide`).
    pub solid_entities: Vec<Aabb>,
    pub(crate) old_status: BoatStatus,
    pub(crate) out_of_control: f32,
    pub(crate) water_level: f64,
    pub(crate) land_friction: f32,
    pub(crate) last_yd: f64,
    vertical_collision: bool,
    pub(crate) support: Support,
    pub(crate) stuck: [f64; 3],
    pub(crate) old_pos: [f64; 3],
    pub(crate) moves: Vec<Move>,
    pub(crate) lerp: Lerp,
}

impl Boat {
    /// A boat at rest where the client sees it, with the status its last unridden tick found.
    pub fn new(world: &impl Blocks, pos: [f64; 3], yaw: f32, raft: bool) -> Boat {
        let mut boat = Boat { pos, yaw, raft, ..Boat::default() };
        boat.status = boat.read_status(world);
        boat
    }

    pub fn bounding_box(&self) -> Aabb {
        Aabb::of_entity(self.pos, WIDTH, HEIGHT)
    }

    /// Where `positionRider` puts a lone player's feet.
    pub fn rider_feet(&self) -> [f64; 3] {
        let ride_height = if self.raft { HEIGHT * 0.8888889 } else { HEIGHT / 3.0 };
        [self.pos[0], self.pos[1] + ride_height as f64 - PLAYER_ATTACHMENT, self.pos[2]]
    }

    /// `AbstractBoat.clampRotation` after `positionRider` turned the rider with the boat: the rider's yaw.
    pub fn turn_rider(&self, yaw: f32) -> f32 {
        let yaw = yaw + self.delta_rotation;
        let delta = wrap_degrees(yaw - self.yaw);
        let target = delta.clamp(-105.0, 105.0);
        yaw + target - delta
    }

    /// One tick driven with `keys` (the rider's keys of the previous tick); returns the paddles [left, right].
    pub fn tick(&mut self, world: &impl Blocks, keys: Keys) -> [bool; 2] {
        self.old_pos = self.pos;
        self.interpolate(world);
        self.old_status = self.status;
        self.status = self.read_status(world);
        let under = matches!(self.status, BoatStatus::UnderWater | BoatStatus::UnderFlowingWater);
        self.out_of_control = if under { self.out_of_control + 1.0 } else { 0.0 };
        FluidScan::of(world, self.bounding_box(), None, true).push(&mut self.delta, false);
        self.float(world);
        let paddles = self.control(keys);
        self.move_self(world);
        self.apply_block_effects(world);
        paddles
    }

    pub(crate) fn colliders<'a, W: Blocks>(&'a self, world: &'a W) -> Colliders<'a, W> {
        Colliders { world, context: Context { feet: self.pos[1], ..Context::default() }, entities: &self.solid_entities }
    }

    /// `floatBoat`.
    fn float(&mut self, world: &impl Blocks) {
        let landing = self.old_status == BoatStatus::InAir && !matches!(self.status, BoatStatus::InAir | BoatStatus::OnLand);
        if landing {
            self.water_level = self.pos[1] + HEIGHT as f64 * 1.0;
            let target = (self.water_level_above(world) - HEIGHT) as f64 + 0.101;
            if self.colliders(world).no_collision(&self.bounding_box().moved([0.0, target - self.pos[1], 0.0])) {
                self.pos[1] = target;
                self.delta[1] *= 0.0;
                self.last_yd = 0.0;
            }
            self.status = BoatStatus::InWater;
            return;
        }
        let (inv_friction, vspeed, buoyancy) = match self.status {
            BoatStatus::InWater => (0.9_f32, -GRAVITY, (self.water_level - self.pos[1]) / HEIGHT as f64),
            BoatStatus::UnderFlowingWater => (0.9, -7.0E-4, 0.0),
            BoatStatus::UnderWater => (0.45, -GRAVITY, 0.01_f32 as f64),
            BoatStatus::InAir => (0.9, -GRAVITY, 0.0),
            BoatStatus::OnLand => {
                let friction = self.land_friction;
                self.land_friction /= 2.0;
                (friction, -GRAVITY, 0.0)
            }
        };
        let d = self.delta;
        self.delta = [d[0] * inv_friction as f64, d[1] + vspeed, d[2] * inv_friction as f64];
        self.delta_rotation *= inv_friction;
        if buoyancy > 0.0 {
            self.delta[1] = (self.delta[1] + buoyancy * (GRAVITY / 0.65)) * 0.75;
        }
    }

    /// `controlBoat`.
    fn control(&mut self, keys: Keys) -> [bool; 2] {
        let mut acceleration = 0.0_f32;
        if keys.left {
            self.delta_rotation -= 1.0;
        }
        if keys.right {
            self.delta_rotation += 1.0;
        }
        if keys.right != keys.left && !keys.forward && !keys.backward {
            acceleration += 0.005;
        }
        self.yaw += self.delta_rotation;
        if keys.forward {
            acceleration += 0.04;
        }
        if keys.backward {
            acceleration -= 0.005;
        }
        let x = sin((-self.yaw * DEG_TO_RAD) as f64) * acceleration;
        let z = cos((self.yaw * DEG_TO_RAD) as f64) * acceleration;
        self.delta = [self.delta[0] + x as f64, self.delta[1] + 0.0, self.delta[2] + z as f64];
        [keys.right && !keys.left || keys.forward, keys.left && !keys.right || keys.forward]
    }

    /// `Entity.move` for `MoverType.SELF`, with the boat's `checkFallDamage`.
    fn move_self(&mut self, world: &impl Blocks) {
        let mut delta = self.delta;
        if length_sqr(self.stuck) > 1.0E-7 {
            for axis in 0..3 {
                delta[axis] *= self.stuck[axis];
            }
            self.stuck = [0.0; 3];
            self.delta = [0.0; 3];
        }
        let movement = self.colliders(world).collide(self.bounding_box(), delta, self.on_ground, 0.0);
        let moved = length_sqr(movement);
        if moved > 1.0E-7 || length_sqr(delta) - moved < 1.0E-7 {
            let from = self.pos;
            for axis in 0..3 {
                self.pos[axis] += movement[axis];
            }
            self.moves.push((from, self.pos, delta));
        }
        let close = |a: f64, b: f64| (b - a).abs() < 1.0E-5_f32 as f64;
        let (x_hit, z_hit) = (!close(delta[0], movement[0]), !close(delta[2], movement[2]));
        self.horizontal_collision = x_hit || z_hit;
        self.vertical_collision = delta[1] != movement[1];
        let below = self.vertical_collision && delta[1] < 0.0;
        self.on_ground = below;
        let mut support = self.support;
        support.update(&self.colliders(world), self.bounding_box(), self.pos, below, movement);
        self.support = support;
        let effect_block = world.block_state(self.support.on_pos(world, self.pos, LEGACY_ON_POS)).block();
        self.last_yd = self.delta[1];
        if (delta[1].abs() > 0.0 && self.vertical_collision) || self.horizontal_collision {
            self.restitute(effect_block, x_hit, z_hit, below, movement);
        }
        let factor = self.support.speed_factor(world, self.pos) as f64;
        self.delta[0] *= factor;
        self.delta[2] *= factor;
    }

    /// `restituteMovementAfterCollisions`: boats bounce only off blocks, at 0.8 of a living entity's bounce.
    fn restitute(&mut self, block: Block, x_hit: bool, z_hit: bool, below: bool, movement: [f64; 3]) {
        let current = self.delta;
        let mut restitution = 0.0;
        if x_hit {
            self.delta[0] = -current[0] * restitution;
        }
        if z_hit {
            self.delta[2] = -current[2] * restitution;
        }
        if !self.vertical_collision {
            return;
        }
        if below {
            let bounces = !(-current[1] <= GRAVITY) && !block.is(BlockTag::SuppressesBounce);
            restitution = if bounces { (bounce(block) * 0.8) as f64 } else { 0.0 };
        }
        let (gravity, drag) = if restitution > 0.0 {
            let portion = movement[1] / current[1];
            (portion * GRAVITY, 1.0 + portion * (1.0 - 1.0))
        } else {
            (0.0, 1.0)
        };
        self.delta[1] = (gravity - current[1]) * drag * restitution;
    }
}

#[cfg(test)]
#[path = "boat_tests.rs"]
mod tests;
