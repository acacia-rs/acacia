use crate::collide::Blocks;
use crate::mth::{look_vector, sin, DEG_TO_RAD};
use crate::player::Player;

const WATER_SLOWDOWN: f32 = 0.8;
const SWIM_SPEED: f32 = 0.02;

impl Player {
    fn fluid_falling(&self, gravity: f64, falling: bool, movement: [f64; 3]) -> [f64; 3] {
        if gravity == 0.0 || self.sprinting {
            return movement;
        }
        let y = if falling && (movement[1] - 0.005).abs() >= 0.003 && (movement[1] - gravity / 16.0).abs() < 0.003 {
            -0.003
        } else {
            movement[1] - gravity / 16.0
        };
        [movement[0], y, movement[2]]
    }

    fn jump_out_of_fluid(&mut self, world: &impl Blocks, old_y: f64) {
        let m = self.delta;
        if self.horizontal_collision && self.is_free(world, [m[0], m[1] + 0.6_f32 as f64 - self.pos[1] + old_y, m[2]]) {
            self.delta[1] = 0.3_f32 as f64;
        }
    }

    fn travel_in_water(&mut self, world: &impl Blocks, gravity: f64, falling: bool, old_y: f64) {
        let mut slow = if self.sprinting { 0.9 } else { self.mount.map_or(WATER_SLOWDOWN, |m| m.water_slowdown) };
        if self.effects.dolphins_grace {
            slow = 0.96;
        }
        self.move_relative(SWIM_SPEED);
        self.move_self(world);
        if self.horizontal_collision && self.on_climbable(world) {
            self.delta[1] = 0.2;
        }
        let d = self.delta;
        let movement = [d[0] * slow as f64, d[1] * 0.8_f32 as f64, d[2] * slow as f64];
        self.delta = self.fluid_falling(gravity, falling, movement);
        self.jump_out_of_fluid(world, old_y);
    }

    fn travel_in_lava(&mut self, world: &impl Blocks, gravity: f64, falling: bool, old_y: f64) {
        self.move_relative(0.02);
        self.move_self(world);
        if self.fluids.height(true) <= self.jump_threshold() {
            let d = self.delta;
            self.delta = self.fluid_falling(gravity, falling, [d[0] * 0.5, d[1] * 0.8_f32 as f64, d[2] * 0.5]);
        } else {
            self.delta = self.delta.map(|v| v * 0.5);
        }
        if gravity != 0.0 {
            self.add_y(-gravity / 4.0);
        }
        self.jump_out_of_fluid(world, old_y);
    }

    pub(crate) fn travel_in_fluid(&mut self, world: &impl Blocks) {
        let falling = self.delta[1] <= 0.0;
        let old_y = self.pos[1];
        let gravity = self.gravity();
        if self.in_water {
            self.travel_in_water(world, gravity, falling, old_y);
        } else {
            self.travel_in_lava(world, gravity, falling, old_y);
        }
    }

    fn fall_flying_movement(&self, mut m: [f64; 3]) -> [f64; 3] {
        let look = look_vector(self.pitch, self.yaw);
        let lean = self.pitch * DEG_TO_RAD;
        let look_h = (look[0] * look[0] + look[2] * look[2]).sqrt();
        let move_h = (m[0] * m[0] + m[2] * m[2]).sqrt();
        let lift = (lean as f64).cos().powi(2);
        m = [m[0] + 0.0, m[1] + self.gravity() * (-1.0 + lift * 0.75), m[2] + 0.0];
        if m[1] < 0.0 && look_h > 0.0 {
            let convert = m[1] * -0.1 * lift;
            m = [m[0] + look[0] * convert / look_h, m[1] + convert, m[2] + look[2] * convert / look_h];
        }
        if lean < 0.0 && look_h > 0.0 {
            let convert = move_h * -(sin(lean as f64) as f64) * 0.04;
            m = [m[0] - look[0] * convert / look_h, m[1] + convert * 3.2, m[2] - look[2] * convert / look_h];
        }
        if look_h > 0.0 {
            m[0] += (look[0] / look_h * move_h - m[0]) * 0.1;
            m[2] += (look[2] / look_h * move_h - m[2]) * 0.1;
        }
        [m[0] * 0.99_f32 as f64, m[1] * 0.98_f32 as f64, m[2] * 0.99_f32 as f64]
    }

    pub(crate) fn travel_fall_flying(&mut self, world: &impl Blocks) {
        if self.on_climbable(world) {
            self.travel_in_air(world);
            self.fall_flying = false;
            return;
        }
        self.delta = self.fall_flying_movement(self.delta);
        self.move_self(world);
    }
}
