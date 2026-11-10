use crate::aabb::Aabb;
use crate::block_factors::friction;
use crate::collide::Blocks;
use crate::mth::{cos, length_sqr, look_vector, sin, DEG_TO_RAD};
use crate::player::Player;
use crate::support::BELOW_MOVEMENT_OFFSET;

pub(crate) fn modified_friction(friction: f32, modifier: f32) -> f32 {
    (1.0 - (1.0 - friction) * modifier).clamp(0.0, 1.0)
}

const CLIMB_LIMIT: f64 = 0.15_f32 as f64;

impl Player {
    pub(crate) fn move_relative(&mut self, speed: f32) {
        let input = [self.xxa as f64, 0.0, self.zza as f64];
        let length = length_sqr(input);
        if length < 1.0E-7 {
            self.delta = self.delta.map(|v| v + 0.0);
            return;
        }
        let unit = if length > 1.0 { input.map(|v| v / length.sqrt()) } else { input };
        let scaled = unit.map(|v| v * speed as f64);
        let radians = (self.yaw * DEG_TO_RAD) as f64;
        let (s, c) = (sin(radians) as f64, cos(radians) as f64);
        self.delta[0] += scaled[0] * c - scaled[2] * s;
        self.delta[1] += scaled[1];
        self.delta[2] += scaled[2] * c + scaled[0] * s;
    }

    /// `Player.travel`, ending in `LivingEntity.travel`.
    pub(crate) fn travel(&mut self, world: &impl Blocks) {
        if self.swimming {
            let look_y = look_vector(self.pitch, self.yaw)[1];
            let multiplier = if look_y < -0.2 { 0.085 } else { 0.06 };
            if look_y <= 0.0 || self.keys.jump || self.fluid_above_feet(world) {
                self.add_y((look_y - self.delta[1]) * multiplier);
            }
        }
        let held_y = self.delta[1];
        if (self.in_water || self.in_lava()) && !self.flying {
            self.travel_in_fluid(world);
        } else if self.fall_flying {
            self.travel_fall_flying(world);
        } else {
            self.travel_in_air(world);
        }
        if self.flying {
            self.delta[1] = held_y * 0.6;
        }
    }

    fn air_speed(&self) -> f32 {
        if self.flying {
            if self.sprinting { self.flying_speed * 2.0 } else { self.flying_speed }
        } else if self.sprinting {
            0.025999999
        } else {
            0.02
        }
    }

    fn climb_clamp(&mut self, world: &impl Blocks) {
        if !self.on_climbable(world) {
            return;
        }
        self.fall_distance = 0.0;
        self.delta[0] = self.delta[0].clamp(-CLIMB_LIMIT, CLIMB_LIMIT);
        self.delta[2] = self.delta[2].clamp(-CLIMB_LIMIT, CLIMB_LIMIT);
        self.delta[1] = self.delta[1].max(-CLIMB_LIMIT);
        let on_scaffold = world.block_state(self.block_pos()).block() == acacia_registry::Block::Scaffolding;
        if self.delta[1] < 0.0 && !on_scaffold && self.keys.shift {
            self.delta[1] = 0.0;
        }
    }

    pub(crate) fn travel_in_air(&mut self, world: &impl Blocks) {
        let below = self.on_pos(world, BELOW_MOVEMENT_OFFSET);
        let block_friction = if self.on_ground { modified_friction(friction(world.block_state(below).block()), 1.0) } else { 1.0 };
        let speed = if !self.on_ground {
            self.air_speed()
        } else if block_friction as f64 > 0.6 {
            self.speed() * (0.21600002 / (block_friction * block_friction * block_friction))
        } else {
            self.speed()
        };
        self.move_relative(speed);
        self.climb_clamp(world);
        self.move_self(world);
        let walks_on_snow = self.was_in_powder_snow && self.snow_boots;
        if (self.horizontal_collision || self.keys.jump) && (self.on_climbable(world) || walks_on_snow) {
            self.delta[1] = 0.2;
        }
        let mut y = self.delta[1];
        if let Some(amp) = self.effects.levitation {
            y += (0.05 * (amp as f64 + 1.0) - self.delta[1]) * 0.2;
        } else if world.has_chunk(below) {
            y -= self.gravity();
        } else if self.pos[1] > world.min_y() as f64 {
            y = -0.1;
        } else {
            y = 0.0;
        }
        let air = modified_friction(0.91, 1.0);
        let horizontal = (block_friction * air) as f64;
        let vertical = modified_friction(0.98, 1.0) as f64;
        self.delta = [self.delta[0] * horizontal, y * vertical, self.delta[2] * horizontal];
    }

    pub(crate) fn is_free(&self, world: &impl Blocks, by: [f64; 3]) -> bool {
        let moved: Aabb = self.bounding_box().moved(by);
        self.colliders(world).no_collision(&moved) && !crate::fluid::contains_any_liquid(world, &moved)
    }
}
