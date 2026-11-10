use crate::collide::Blocks;
use crate::input::{Keys, Pose};
use crate::player::Player;

impl Player {
    fn base_tick(&mut self, world: &impl Blocks) {
        self.was_in_powder_snow = self.in_powder_snow;
        self.in_powder_snow = false;
        self.was_eye_in_water = self.fluids.eyes_in_water();
        self.update_fluids(world);
        self.update_swimming(world);
    }

    fn update_swimming(&mut self, world: &impl Blocks) {
        self.swimming = !self.flying
            && if self.swimming {
                self.sprinting && self.in_water
            } else {
                self.sprinting && self.under_water() && self.water_at_block(world)
            };
    }

    fn start_gliding_if_asked(&mut self, world: &impl Blocks, was_jumping: bool, toggled_flight: bool) {
        let can_glide = !self.flying && !self.on_ground && self.effects.levitation.is_none() && self.has_glider;
        let in_liquid = self.in_water || self.in_lava();
        if self.keys.jump && !toggled_flight && !was_jumping && !self.on_climbable(world) && !self.fall_flying && can_glide && !in_liquid {
            self.fall_flying = true;
        }
    }

    /// One tick of a firework rocket attached to the player (`FireworkRocketEntity.tick`): the client ticks the rocket
    /// after the player, so this runs after [`Player::tick`] for every rocket alive that tick.
    pub fn firework_boost(&mut self) {
        if !self.fall_flying {
            return;
        }
        let look = crate::mth::look_vector(self.pitch, self.yaw);
        for (axis, l) in look.into_iter().enumerate() {
            self.delta[axis] += l * 0.1 + (l * 1.5 - self.delta[axis]) * 0.5;
        }
    }

    fn apply_flight_input(&mut self) {
        if !self.flying {
            return;
        }
        let direction = self.keys.jump as i32 - self.keys.shift as i32;
        if direction != 0 {
            self.add_y((direction as f32 * self.flying_speed * 3.0) as f64);
        }
    }

    fn clamp_small_delta(&mut self) {
        if self.delta[0] * self.delta[0] + self.delta[2] * self.delta[2] < 9.0E-6 {
            self.delta[0] = 0.0;
            self.delta[2] = 0.0;
        }
        if self.delta[1].abs() < 0.003 {
            self.delta[1] = 0.0;
        }
    }

    fn jump_in_liquid(&mut self) {
        self.add_y(0.04_f32 as f64);
    }

    fn handle_jump(&mut self, world: &impl Blocks) {
        if !self.keys.jump || self.flying {
            self.no_jump_delay = 0;
            return;
        }
        let lava = self.in_lava();
        let height = self.fluids.height(lava);
        let threshold = self.jump_threshold();
        let in_water = self.in_water && height > 0.0;
        if in_water && !(self.on_ground && height <= threshold) {
            self.jump_in_liquid();
        } else if lava && !(self.on_ground && self.fluids.height(true) <= threshold) {
            self.jump_in_liquid();
        } else if (self.on_ground || (in_water && height <= threshold)) && self.no_jump_delay == 0 {
            self.jump_from_ground(world);
            self.no_jump_delay = 10;
        }
    }

    fn update_pose(&mut self, world: &impl Blocks) {
        if !self.fits(world, Pose::Swimming) {
            return;
        }
        let desired = if self.swimming {
            Pose::Swimming
        } else if self.fall_flying {
            Pose::FallFlying
        } else if self.keys.shift && !self.flying {
            Pose::Crouching
        } else {
            Pose::Standing
        };
        self.pose = if self.fits(world, desired) {
            desired
        } else if self.fits(world, Pose::Crouching) {
            Pose::Crouching
        } else {
            Pose::Swimming
        };
    }

    /// One client tick, in `LocalPlayer.tick` order: base tick, `aiStep`, then the pose update.
    pub fn tick(&mut self, world: &impl Blocks, keys: Keys) {
        self.base_tick(world);
        if self.sprint_trigger > 0 {
            self.sprint_trigger -= 1;
        }
        let was_jumping = self.keys.jump;
        let was_shift = self.keys.shift;
        let had_forward = self.has_forward_impulse();
        self.crouching =
            !self.flying && !self.swimming && self.fits(world, Pose::Crouching) && (was_shift || !self.fits(world, Pose::Standing));
        self.read_keys(keys);
        self.push_out_of_blocks(world);
        self.update_sprint(was_shift, had_forward);
        let toggled = self.update_flight_toggle(world, was_jumping);
        self.start_gliding_if_asked(world, was_jumping, toggled);
        if self.in_water && keys.shift && !self.flying {
            self.add_y(-0.04_f32 as f64);
        }
        self.apply_flight_input();

        if self.jump_trigger > 0 {
            self.jump_trigger -= 1;
        }
        if self.flying {
            self.fall_distance = 0.0;
        }
        if self.no_jump_delay > 0 {
            self.no_jump_delay -= 1;
        }
        self.clamp_small_delta();
        self.apply_input();
        self.handle_jump(world);
        if self.fall_flying && self.delta[1] > -0.5 && self.fall_distance > 1.0 {
            self.fall_distance = 1.0;
        }
        if self.effects.slow_falling || self.effects.levitation.is_some() {
            self.fall_distance = 0.0;
        }
        self.travel(world);
        self.apply_block_effects(world);
        if self.on_ground && self.flying {
            self.flying = false;
        }
        self.update_pose(world);
    }
}
