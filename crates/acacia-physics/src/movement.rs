//! Ground/air travel (bedsim `simulateMovement`).

use crate::block_effects::{apply_ascendable_movement, apply_stuck_speed_multiplier};
use crate::constants::*;
use crate::math::{block_pos, sub};
use crate::motion::*;
use crate::sim::Sim;
use crate::state::PlayerState;
use crate::world::{LiquidKind, Traversal, WorldView};

impl<W: WorldView + ?Sized> Sim<'_, W> {
    /// Runs one tick of travel; false when part of the needed world is unknown.
    pub(crate) fn simulate_movement(&self, st: &mut PlayerState) -> bool {
        let mut vel = st.vel;
        for v in &mut vel {
            if v.abs() < 1e-8 {
                *v = 0.0;
            }
        }
        st.set_vel(vel);

        let was_swim_pose = st.swim_pose();
        let grace = DEFAULT_SWIM_WATER_GRACE_TICKS;
        st.swim_water_grace_ticks = st.swim_water_grace_ticks.min(grace);
        let water = self.touching_liquid_blocks(st, LiquidKind::Water);
        let lava = self.touching_liquid_blocks(st, LiquidKind::Lava);
        let in_water = !water.is_empty();
        st.swim_water_contact = in_water;
        if was_swim_pose && !st.swim_pose() && !st.gliding {
            let available = self.pose_collisions_available(st);
            if !self.restore_upright_pose(st, available) {
                return false;
            }
        }
        if in_water && st.swimming {
            st.swim_water_grace_ticks = grace;
            st.set_swimming_pose_flags();
        }

        let mut known = self.travel(st, in_water, &water, &lava);

        let had_swim_pose = st.swim_pose();
        if in_water {
            st.swim_water_grace_ticks = grace;
        } else if st.swim_water_grace_ticks > 0 {
            st.swim_water_grace_ticks -= 1;
        }
        if known && had_swim_pose && !st.swim_pose() && !st.gliding {
            let available = self.pose_collisions_available(st);
            known = self.restore_upright_pose(st, available);
        }
        known
    }

    fn travel(&self, st: &mut PlayerState, in_water: bool, water: &[[i32; 3]], lava: &[[i32; 3]]) -> bool {
        // Observed lava takes precedence over retained water evidence.
        let water_travel = in_water || (st.swimming && st.swim_water_grace_ticks > 0 && lava.is_empty());
        if water_travel || !lava.is_empty() {
            attempt_knockback(st);
            if water_travel {
                if st.gliding {
                    if !self.stop_gliding(st) {
                        return false;
                    }
                    st.glide_boost_ticks = 0;
                }
                let can_sink = st.want_down || !st.swimming && self.standing_head_in(st, LiquidKind::Water);
                return self.apply_liquid_flow(st, water, LiquidKind::Water)
                    && self.simulate_liquid_travel(st, LiquidKind::Water, in_water, can_sink);
            }
            return self.apply_liquid_flow(st, lava, LiquidKind::Lava)
                && self.simulate_liquid_travel(st, LiquidKind::Lava, true, true);
        }

        let mut under = self.w.block(block_pos(sub(st.pos, [0.0, 0.5, 0.0])));
        let mut block_friction = DEFAULT_AIR_FRICTION;
        let mut speed = st.air_speed;
        if st.on_ground {
            block_friction *= under.friction;
            let mut accel_mult = under.acceleration_friction_multiplier;
            if st.equipment.soul_speed > 0 && under.soul_speed_neutralizes {
                accel_mult = 1.0;
            }
            let accel_friction = (under.friction * accel_mult) * DEFAULT_AIR_FRICTION;
            // Vanilla's f32 order: dividing a precomputed cube is not equivalent.
            let ratio = (DEFAULT_AIR_FRICTION * DEFAULT_BLOCK_FRICTION) / accel_friction;
            speed = ((st.movement_speed * ratio) * ratio) * ratio;
        }
        if st.gliding && st.effects.levitation.is_some() && !self.stop_gliding(st) {
            return false;
        }
        if st.gliding {
            if st.equipment.elytra && !st.on_ground {
                return self.glide(st);
            }
            if !self.stop_gliding(st) {
                return false;
            }
        }

        attempt_knockback(st);
        move_relative(st, speed);
        self.attempt_jump(st);
        let mut inside = self.w.block(block_pos(st.pos));
        if inside.traversal == Traversal::None
            && let Some(sp) = st.supporting_block
            && self.w.block(sp).traversal == Traversal::Scaffolding
        {
            inside.traversal = Traversal::Scaffolding;
        }
        let scaffold_descend = apply_ascendable_movement(st, inside.traversal);

        let near_climbable = inside.climbable;
        if near_climbable {
            let mut v = st.vel;
            if v[1] < -CLIMB_SPEED {
                v[1] = -CLIMB_SPEED;
            }
            if st.effective_jumping {
                v[1] = CLIMB_SPEED;
            }
            if st.sneaking && v[1] < 0.0 {
                v[1] = 0.0;
            }
            st.set_vel(v);
        }

        let stuck = apply_stuck_speed_multiplier(st);
        if !self.sweep_loaded(st) || !self.avoid_edge(st) {
            return false;
        }
        let mut old_vel = st.vel;
        let old_on_ground = st.on_ground;
        let old_y = st.pos[1];
        if !self.try_collisions(st) {
            return false;
        }
        update_fall_distance(st, old_y);
        if scaffold_descend || near_climbable {
            st.fall_distance = 0.0;
        }

        under = match st.supporting_block {
            Some(sp) => self.w.block(sp),
            None => {
                let b = self.w.block(block_pos(sub(st.pos, [0.0, 0.2, 0.0])));
                let [x, y, z] = block_pos(st.pos);
                match b.air.then(|| self.w.block([x, y - 1, z])) {
                    Some(below) if below.fence_or_wall => below,
                    _ => b,
                }
            }
        };
        st.mov = st.vel;
        if stuck {
            st.set_vel([0.0; 3]);
            old_vel = [0.0; 3];
        }
        set_post_collision_motion(st, old_vel, old_on_ground, &under, st.gravity);

        let mut v = st.vel;
        if !scaffold_descend {
            if let Some(amp) = st.effects.levitation {
                let lev = LEVITATION_GRAVITY_MULTIPLIER * (amp + 1) as f32;
                v[1] += (lev - v[1]) * 0.2;
            } else if st.has_gravity {
                v[1] -= effective_gravity(st, v);
                v[1] *= NORMAL_GRAVITY_MULTIPLIER;
            }
        }
        v[0] *= block_friction;
        v[2] *= block_friction;
        // A wall climb sets the speed after the move and gravity, from this tick's collision (vanilla client).
        if self.w.block(block_pos(st.pos)).climbable && (st.collide_x || st.collide_z) {
            v[1] = CLIMB_SPEED;
        }
        st.set_vel(v);
        if self.support_under_box(st).is_some() {
            walk_on_block(st, &self.w.block(self.standing_on_block(st)), v[1]);
        }
        self.apply_inside_block_effects(st);
        self.apply_honey_wall_slide(st);
        self.apply_bubble_columns(st);
        true
    }

    fn glide(&self, st: &mut PlayerState) -> bool {
        st.on_ground = false;
        simulate_glide(st);
        let stuck = apply_stuck_speed_multiplier(st);
        if !self.sweep_loaded(st) {
            return false;
        }
        let old_y = st.pos[1];
        if !self.try_collisions(st) {
            return false;
        }
        update_fall_distance(st, old_y);
        st.mov = st.vel;
        if stuck {
            st.set_vel([0.0; 3]);
        }
        self.apply_inside_block_effects(st);
        self.apply_honey_wall_slide(st);
        self.apply_bubble_columns(st);
        true
    }

    fn attempt_jump(&self, st: &mut PlayerState) {
        if !st.jumping || !st.on_ground || st.jump_delay > 0 || st.swim_exit_jump_delay > 0 {
            return;
        }
        let mut height = st.jump_height;
        let inside = self.w.block(block_pos(st.pos));
        let below = self.w.block(block_pos(sub(st.pos, [0.0, 0.1, 0.0])));
        if inside.honey || below.honey {
            height *= 0.6;
        }
        let v = jump_impulse(st.vel, height, st.yaw, st.sprinting);
        st.jump_delay = JUMP_DELAY_TICKS;
        st.set_vel(v);
        st.jumped = true;
    }
}
