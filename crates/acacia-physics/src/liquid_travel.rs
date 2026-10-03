//! Liquid travel step (bedsim `simulateLiquidTravel`).

use crate::block_effects::{apply_ascendable_movement, apply_stuck_speed_multiplier};
use crate::constants::*;
use crate::motion::{move_relative, set_post_collision_motion, walk_on_block};
use crate::sim::Sim;
use crate::state::PlayerState;
use crate::world::{BlockPhysics, LiquidKind, WorldView};

fn liquid_gravity(swimming: bool, water: bool) -> f32 {
    match (water, swimming) {
        (false, _) => 0.02,
        (true, true) => 0.0,
        (true, false) => 0.005,
    }
}

impl<W: WorldView + ?Sized> Sim<'_, W> {
    pub(crate) fn simulate_liquid_travel(&self, st: &mut PlayerState, kind: LiquidKind, touching: bool, can_sink: bool) -> bool {
        let initial_y = st.pos[1];
        let water = kind == LiquidKind::Water;
        // Captured before the swim-travel update, matching upstream ordering.
        let jumping = st.effective_jumping;
        if water {
            // Without `WantDown` a held jump wins over sneak (BDS); with it both apply and cancel out.
            if can_sink && st.pressing_descend && (!jumping || st.want_down) {
                st.set_vel([st.vel[0], st.vel[1] - 0.04, st.vel[2]]);
            }
            self.update_swim_travel(st);
        }
        if jumping {
            let mut v = st.vel;
            // A swimming jump lifts only with the middle of the swim box under water (BDS).
            let lifts = touching && self.point_under([st.pos[0], st.pos[1] + 0.3, st.pos[2]], kind);
            if st.swim_amount > 0.0 && st.swim_amount < 1.0 || water && st.swimming && !lifts {
                v[1] = 0.0;
            } else {
                v[1] += 0.04;
            }
            st.set_vel(v);
        }

        let mut speed = if st.lava_movement_speed == 0.0 { DEFAULT_LAVA_MOVEMENT_SPEED } else { st.lava_movement_speed };
        let mut depth_strider = 0f32;
        let mut swim_multiplier = DEFAULT_SWIM_SPEED_MULTIPLIER;
        if water {
            speed = if st.underwater_movement_speed == 0.0 {
                DEFAULT_UNDERWATER_MOVEMENT_SPEED
            } else {
                st.underwater_movement_speed
            };
            if st.swimming && st.swim_speed_multiplier != 0.0 {
                swim_multiplier = st.swim_speed_multiplier;
            }
            depth_strider = (st.equipment.depth_strider as f32).clamp(0.0, 3.0);
            let fraction = depth_strider / 3.0;
            if swim_multiplier > 1.0 {
                speed *= (0.7 + fraction * 0.3) * swim_multiplier;
            } else {
                if !st.on_ground {
                    depth_strider *= 0.5;
                }
                speed += (st.movement_speed - speed) * (depth_strider / 3.0);
            }
        }
        move_relative(st, speed);
        // Scaffolding climbs in liquids too (BDS fuzz: 0.15 up, then liquid drag).
        apply_ascendable_movement(st, self.traversal(st));
        // Cobwebs slow liquid travel too (BDS; bedsim only checks them on land).
        let in_cobweb = self.is_inside_cobweb(st);
        if in_cobweb {
            let (xz, y) = if st.effects.weaving { (0.5, 0.25) } else { (0.25, 0.05) };
            st.set_vel([st.vel[0] * xz, st.vel[1] * y, st.vel[2] * xz]);
        }
        let stuck = apply_stuck_speed_multiplier(st);
        // A sneaker standing in shallow liquid keeps to the edge too (BDS; bedsim only on land).
        if !self.sweep_loaded(st) || !self.avoid_edge(st) {
            return false;
        }
        let mut old_vel = st.vel;
        let old_on_ground = st.on_ground;
        if !self.try_collisions(st) {
            return false;
        }
        if stuck {
            st.mov = st.vel;
            st.set_vel([0.0; 3]);
            old_vel = [0.0; 3];
        }
        // Slime under liquid bounces too (BDS; bedsim lands on air).
        let under = st.supporting_block.map_or(BlockPhysics::AIR, |sp| self.w.block(sp));
        set_post_collision_motion(st, old_vel, old_on_ground, &under, liquid_gravity(st.swimming, water));
        if !stuck {
            st.mov = st.vel;
        }
        if in_cobweb {
            st.set_vel([0.0; 3]);
        }

        let mut v = st.vel;
        if water {
            let light = st.swimming || st.sprinting;
            let mut drag = if light || st.stopped_swimming_this_tick { 0.9 } else { WATER_DRAG };
            if depth_strider > 0.0 && swim_multiplier <= 1.0 {
                drag += (0.54600006 - drag) * (depth_strider / 3.0);
            }
            v[0] *= drag;
            v[1] *= 0.8;
            v[2] *= drag;
        } else {
            v = [v[0] * 0.5, v[1] * 0.5, v[2] * 0.5];
        }
        if let Some(amp) = st.effects.levitation {
            let target = LEVITATION_GRAVITY_MULTIPLIER * (amp + 1) as f32;
            v[1] += (target - v[1]) * 0.2;
        } else if st.has_gravity {
            v[1] -= liquid_gravity(st.swimming, water);
        }

        if st.collide_x || st.collide_z {
            let raised_box = st.bounding_box().translate([v[0], v[1] + 0.6 + initial_y - st.pos[1], v[2]]);
            if !self.loaded(&raised_box) {
                return false;
            }
            if !self.has_nearby_bboxes(st, &raised_box) && !self.contains_any_liquid(&raised_box) {
                v[1] = 0.3;
            }
        }
        st.set_vel(v);
        // Slime and honey slow walking in water too (BDS; bedsim only on land).
        if self.support_under_box(st).is_some() {
            walk_on_block(st, &self.w.block(self.standing_on_block(st)), v[1]);
        }
        self.apply_bubble_columns(st);
        self.apply_inside_block_effects(st);
        st.fall_distance = 0.0;
        true
    }
}
