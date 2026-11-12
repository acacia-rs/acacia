use acacia_registry::{Block, BlockTag};

use crate::aabb::Aabb;
use crate::block_factors::bounce;
use crate::collide::Blocks;
use crate::mth::{cos, length_sqr, sin, DEG_TO_RAD};
use crate::attributes::Attribute;
use crate::player::Player;
use crate::support::LEGACY_ON_POS;

const BACK_OFF_STEP: f64 = 0.05;

fn signum(v: f64) -> f64 {
    if v == 0.0 { 0.0 } else { v.signum() }
}

impl Player {
    fn can_fall(&self, world: &impl Blocks, dx: f64, dz: f64, height: f64) -> bool {
        let b = self.bounding_box();
        self.colliders(world).no_collision(&Aabb {
            min: [b.min[0] + 1.0E-7 + dx, b.min[1] - height - 1.0E-7, b.min[2] + 1.0E-7 + dz],
            max: [b.max[0] - 1.0E-7 + dx, b.min[1], b.max[2] - 1.0E-7 + dz],
        })
    }

    fn back_off_from_edge(&self, world: &impl Blocks, delta: [f64; 3]) -> [f64; 3] {
        let step = self.step_height() as f64;
        if self.flying || delta[1] > 0.0 || !self.keys.shift {
            return delta;
        }
        if !self.on_ground && !(self.fall_distance < step && !self.can_fall(world, 0.0, 0.0, step - self.fall_distance)) {
            return delta;
        }
        let (mut dx, mut dz) = (delta[0], delta[2]);
        let (step_x, step_z) = (signum(dx) * BACK_OFF_STEP, signum(dz) * BACK_OFF_STEP);
        let reduce = |v: &mut f64, by: f64| *v = if v.abs() <= BACK_OFF_STEP { 0.0 } else { *v - by };
        while dx != 0.0 && self.can_fall(world, dx, 0.0, step) {
            if dx.abs() <= BACK_OFF_STEP {
                dx = 0.0;
                break;
            }
            dx -= step_x;
        }
        while dz != 0.0 && self.can_fall(world, 0.0, dz, step) {
            if dz.abs() <= BACK_OFF_STEP {
                dz = 0.0;
                break;
            }
            dz -= step_z;
        }
        while dx != 0.0 && dz != 0.0 && self.can_fall(world, dx, dz, step) {
            reduce(&mut dx, step_x);
            reduce(&mut dz, step_z);
        }
        [dx, delta[1], dz]
    }

    fn update_support(&mut self, world: &impl Blocks, movement: [f64; 3]) {
        let mut support = self.support;
        support.update(&self.colliders(world), self.bounding_box(), self.pos, self.on_ground, movement);
        self.support = support;
    }

    fn is_minor_collision(&self, movement: [f64; 3]) -> bool {
        let radians = (self.yaw * DEG_TO_RAD) as f64;
        let (s, c) = (sin(radians) as f64, cos(radians) as f64);
        let (xxa, zza) = (self.xxa as f64, self.zza as f64);
        let (ax, az) = (xxa * c - zza * s, zza * c + xxa * s);
        let wanted = ax * ax + az * az;
        let moved = movement[0] * movement[0] + movement[2] * movement[2];
        if wanted < 1.0E-5_f32 as f64 || moved < 1.0E-5_f32 as f64 {
            return false;
        }
        ((ax * movement[0] + az * movement[2]) / (wanted * moved).sqrt()).acos() < 0.13962634_f32 as f64
    }

    fn update_fall_distance(&mut self, world: &impl Blocks, ya: f64) {
        if !self.in_water {
            self.update_fluids(world);
        }
        if !self.in_water && ya < 0.0 {
            self.fall_distance -= ya as f32 as f64;
        }
        if self.on_ground {
            self.fall_distance = 0.0;
        }
    }

    fn restitute(&mut self, block: Block, x_hit: bool, z_hit: bool, below: bool, movement: [f64; 3]) {
        let current = self.delta;
        let mut restitution = if self.keys.shift { 0.0 } else { self.attribute(Attribute::Bounciness) };
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
            let bounces = !(-current[1] <= self.gravity()) && !self.keys.shift && !block.is(BlockTag::SuppressesBounce);
            restitution = if bounces { restitution.max(bounce(block) as f64) } else { 0.0 };
        }
        let (gravity, drag) = if restitution > 0.0 {
            let portion = movement[1] / current[1];
            let air = self.air_drag(0.98) as f64;
            (portion * self.gravity(), 1.0 + portion * (air - 1.0))
        } else {
            (0.0, 1.0)
        };
        self.delta[1] = (gravity - current[1]) * drag * restitution;
    }

    fn block_speed_factor(&self, world: &impl Blocks) -> f32 {
        if self.flying || self.fall_flying {
            return 1.0;
        }
        let factor = self.support.speed_factor(world, self.pos);
        factor + self.attribute(Attribute::MovementEfficiency) as f32 * (1.0 - factor)
    }

    /// `Entity.move(MoverType.SELF, getDeltaMovement())`.
    pub(crate) fn move_self(&mut self, world: &impl Blocks) {
        self.move_by(world, self.delta);
    }

    /// `Entity.move` for `MoverType.SELF`.
    pub(crate) fn move_by(&mut self, world: &impl Blocks, delta: [f64; 3]) {
        self.move_as(world, delta, true);
    }

    /// `Entity.move` for `MoverType.SHULKER` or `SHULKER_BOX`: an opening lid shoving the player, which no sneak
    /// holds back at an edge.
    pub fn move_pushed(&mut self, world: &impl Blocks, delta: [f64; 3]) {
        self.move_as(world, delta, false);
    }

    fn move_as(&mut self, world: &impl Blocks, mut delta: [f64; 3], by_self: bool) {
        if length_sqr(self.stuck) > 1.0E-7 {
            for axis in 0..3 {
                delta[axis] *= self.stuck[axis];
            }
            self.stuck = [0.0; 3];
            self.delta = [0.0; 3];
        }
        let delta = if by_self { self.back_off_from_edge(world, delta) } else { delta };
        let movement = self.colliders(world).collide(self.bounding_box(), delta, self.on_ground, self.step_height());
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
        let moved_vertically = delta[1].abs() > 0.0;
        self.vertical_collision = delta[1] != movement[1];
        let below = self.vertical_collision && delta[1] < 0.0;
        self.on_ground = below;
        self.update_support(world, movement);
        self.minor_horizontal_collision = self.horizontal_collision && self.is_minor_collision(movement);
        let effect_block = world.block_state(self.on_pos(world, LEGACY_ON_POS)).block();
        self.update_fall_distance(world, movement[1]);
        if (moved_vertically && self.vertical_collision) || self.horizontal_collision {
            self.restitute(effect_block, x_hit, z_hit, below, movement);
        }
        let factor = self.block_speed_factor(world) as f64;
        self.delta[0] *= factor;
        self.delta[2] *= factor;
    }
}
