use crate::aabb::Aabb;
use crate::block_factors::{friction, speed_factor};
use crate::collide::{collide, no_collision, supporting_block, Blocks};
use crate::mth::{cos, length_sqr, sin, DEG_TO_RAD};
use crate::player::{Player, STEP_HEIGHT};

const GRAVITY: f64 = 0.08;
const BELOW_MOVEMENT_OFFSET: f32 = 0.500001;
const BACK_OFF_STEP: f64 = 0.05;

fn modified_friction(friction: f32, modifier: f32) -> f32 {
    (1.0 - (1.0 - friction) * modifier).clamp(0.0, 1.0)
}

impl Player {
    fn move_relative(&mut self, speed: f32) {
        let input = [self.xxa as f64, 0.0, self.zza as f64];
        let length = length_sqr(input);
        if length < 1.0E-7 {
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

    fn can_fall(&self, world: &impl Blocks, dx: f64, dz: f64, height: f64) -> bool {
        let b = self.bounding_box();
        no_collision(
            world,
            &Aabb {
                min: [b.min[0] + 1.0E-7 + dx, b.min[1] - height - 1.0E-7, b.min[2] + 1.0E-7 + dz],
                max: [b.max[0] - 1.0E-7 + dx, b.min[1], b.max[2] - 1.0E-7 + dz],
            },
        )
    }

    fn back_off_from_edge(&self, world: &impl Blocks, delta: [f64; 3]) -> [f64; 3] {
        let step = STEP_HEIGHT;
        let above_ground = self.on_ground
            || (self.fall_distance < step as f64 && !self.can_fall(world, 0.0, 0.0, (step as f64) - self.fall_distance));
        if delta[1] > 0.0 || !self.keys.shift || !above_ground {
            return delta;
        }
        let (mut dx, mut dz) = (delta[0], delta[2]);
        let step_x = signum(dx) * BACK_OFF_STEP;
        let step_z = signum(dz) * BACK_OFF_STEP;
        let reduce = |v: &mut f64, by: f64| *v = if v.abs() <= BACK_OFF_STEP { 0.0 } else { *v - by };
        while dx != 0.0 && self.can_fall(world, dx, 0.0, step as f64) {
            if dx.abs() <= BACK_OFF_STEP {
                dx = 0.0;
                break;
            }
            dx -= step_x;
        }
        while dz != 0.0 && self.can_fall(world, 0.0, dz, step as f64) {
            if dz.abs() <= BACK_OFF_STEP {
                dz = 0.0;
                break;
            }
            dz -= step_z;
        }
        while dx != 0.0 && dz != 0.0 && self.can_fall(world, dx, dz, step as f64) {
            reduce(&mut dx, step_x);
            reduce(&mut dz, step_z);
        }
        [dx, delta[1], dz]
    }

    fn update_support(&mut self, world: &impl Blocks, movement: [f64; 3]) {
        if !self.on_ground {
            self.on_ground_no_blocks = false;
            self.supporting = None;
            return;
        }
        let b = self.bounding_box();
        let area = Aabb { min: [b.min[0], b.min[1] - 1.0E-6, b.min[2]], max: [b.max[0], b.min[1], b.max[2]] };
        let mut found = supporting_block(world, &area, self.pos);
        if found.is_some() || self.on_ground_no_blocks {
            self.supporting = found;
        } else {
            found = supporting_block(world, &area.moved([-movement[0], 0.0, -movement[2]]), self.pos);
            self.supporting = found;
        }
        self.on_ground_no_blocks = found.is_none();
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

    /// `Entity.move` for `MoverType.SELF`.
    fn move_self(&mut self, world: &impl Blocks) {
        let delta = self.back_off_from_edge(world, self.delta);
        let movement = collide(world, self.bounding_box(), delta, self.on_ground, STEP_HEIGHT);
        let moved = length_sqr(movement);
        if moved > 1.0E-7 || length_sqr(delta) - moved < 1.0E-7 {
            for axis in 0..3 {
                self.pos[axis] += movement[axis];
            }
        }
        let close = |a: f64, b: f64| (b - a).abs() < 1.0E-5_f32 as f64;
        let (x_hit, z_hit) = (!close(delta[0], movement[0]), !close(delta[2], movement[2]));
        self.horizontal_collision = x_hit || z_hit;
        self.vertical_collision = delta[1] != movement[1];
        self.on_ground = self.vertical_collision && delta[1] < 0.0;
        self.update_support(world, movement);
        self.minor_horizontal_collision = self.horizontal_collision && self.is_minor_collision(movement);
        if movement[1] < 0.0 {
            self.fall_distance -= movement[1] as f32 as f64;
        }
        if self.on_ground {
            self.fall_distance = 0.0;
        }
        if x_hit {
            self.delta[0] = -self.delta[0] * 0.0;
        }
        if z_hit {
            self.delta[2] = -self.delta[2] * 0.0;
        }
        if self.vertical_collision {
            // TODO: slime bounce; with no restitution this keeps vanilla's signed zero.
            self.delta[1] = (0.0 - self.delta[1]) * 1.0 * 0.0;
        }
        let factor = self.block_speed_factor(world) as f64;
        self.delta[0] *= factor;
        self.delta[2] *= factor;
    }

    fn block_speed_factor(&self, world: &impl Blocks) -> f32 {
        let here = speed_factor(world.block_state(self.block_pos()).block());
        if here as f64 == 1.0 {
            speed_factor(world.block_state(self.on_pos(world, BELOW_MOVEMENT_OFFSET)).block())
        } else {
            here
        }
    }

    pub(crate) fn travel_in_air(&mut self, world: &impl Blocks) {
        let below = self.on_pos(world, BELOW_MOVEMENT_OFFSET);
        let block_friction = if self.on_ground { modified_friction(friction(world.block_state(below).block()), 1.0) } else { 1.0 };
        let speed = if !self.on_ground {
            if self.sprinting { 0.025999999 } else { 0.02 }
        } else if block_friction as f64 > 0.6 {
            self.speed() * (0.21600002 / (block_friction * block_friction * block_friction))
        } else {
            self.speed()
        };
        self.move_relative(speed);
        self.move_self(world);
        let mut y = self.delta[1];
        if world.has_chunk(below) {
            y -= GRAVITY;
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
}

fn signum(v: f64) -> f64 {
    if v == 0.0 { 0.0 } else { v.signum() }
}
