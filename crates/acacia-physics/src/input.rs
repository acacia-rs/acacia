//! Client intent and its translation into bedsim's per-tick input flags (`applyInput`).

use crate::constants::*;
use crate::math::clamp;
use crate::sim::{Sim, effective_air_speed};
use crate::state::PlayerState;
use crate::world::WorldView;

/// Held controls for one tick. Edge flags (start/stop sprint, sneak, ...) are derived from the state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// Raw WASD vector in [-1, 1]: x = strafe (+1 = left, A), y = forward (+1 = W).
    pub move_vector: [f32; 2],
    /// Degrees; Bedrock yaw 0 faces +Z.
    pub yaw: f32,
    pub pitch: f32,
    pub jump: bool,
    pub sneak: bool,
    /// `WantDown`, which the vanilla client sends with every sneak: sinks in water of any depth (without
    /// it BDS sinks a sneaking player only with the head under, see `standing_head_in`).
    pub want_down: bool,
    pub sprint: bool,
    /// Swimming requested (vanilla: sprint while submerged).
    pub swim: bool,
    /// Elytra gliding requested.
    pub glide: bool,
    /// Using an item (eating, drawing a bow): movement slowed to 0.1225.
    pub using_item: bool,
}

/// bedsim `InputState` subset the port consumes.
pub(crate) struct Frame {
    move_vector: [f32; 2],
    pitch: f32,
    yaw: f32,
    start_sprinting: bool,
    stop_sprinting: bool,
    sprint_down: bool,
    start_sneaking: bool,
    stop_sneaking: bool,
    sneak_down: bool,
    want_down: bool,
    start_jumping: bool,
    jumping: bool,
    start_swimming: bool,
    stop_swimming: bool,
    start_gliding: bool,
    stop_gliding: bool,
    fly_tap: bool,
    start_flying: bool,
    stop_flying: bool,
    using_item: bool,
}

/// World tests the start/stop edges depend on, taken before the tick moves the player.
#[derive(Clone, Copy)]
pub(crate) struct Surroundings {
    pub in_water: bool,
    /// See `Sim::swim_start_submerged`.
    pub swim_start_submerged: bool,
    /// See `Sim::swim_surfacing`.
    pub swim_surfacing: bool,
    pub stand_fits: bool,
}

/// The sneak's share of the move input on its `ticks`-th tick: Swift Sneak adds from the third.
fn sneak_impulse(st: &PlayerState, ticks: i32) -> f32 {
    let mut sneak = MAX_SNEAK_IMPULSE;
    if ticks > 2 && st.equipment.swift_sneak != 0 {
        sneak += 0.15 * st.equipment.swift_sneak as f32;
    }
    clamp(sneak, 0.0, 1.0)
}

impl Input {
    /// Mirrors `toInputState` in tools/diffharness/main.go.
    pub(crate) fn frame(&self, st: &PlayerState, env: &Surroundings) -> Frame {
        let Surroundings { in_water, swim_start_submerged: eyes_in_water, swim_surfacing, stand_fits } = *env;
        // BDS `FlyTriggerIntentSystem`: a jump press within the window a first one opened toggles the flight.
        let fly_tap = self.jump && !st.pressing_jump && (st.flight.flying || st.flight.may_fly);
        let flies = st.flight.flying != (fly_tap && st.flight.trigger_ticks > 0);
        // A flyer's sneak key only descends (WantDown).
        let sneak = self.sneak && !flies;
        // BDS `IntentSprintTriggerSystem` tests the move input after normalising and the sneak slowdown
        // (also on the sneak's release tick, see `apply_input`; not in water, where sneak means sink).
        let slowed = (sneak || st.sneaking) && !in_water;
        let scale = if slowed { sneak_impulse(st, st.ticks_since_can_slowdown + 1) } else { 1.0 };
        let len = self.move_vector[0].hypot(self.move_vector[1]);
        let [strafe, forward] = self.move_vector.map(|c| c / len.max(1.0) * scale);
        let enough = if st.sprinting {
            forward > 0.0 && strafe.abs() <= SPRINT_MIN_IMPULSE && strafe.hypot(forward) >= SPRINT_MIN_IMPULSE
        } else {
            forward >= SPRINT_MIN_IMPULSE
        };
        // The sprint before the jump-in-water rule: it decides a swim start (vanilla capture 6-1).
        let wanted = self.sprint && enough && !st.sprint_movement_blocked;
        // A jump held last tick in water then ends that sprint or cancels its start (StartSprinting and
        // StopSprinting on one tick).
        let jump_in_water = in_water && !st.swimming && st.pressing_jump;
        let sprinting = wanted && !jump_in_water;
        // BDS `SwimTriggerSystem`: sprinting with the breathing point under water starts a swim (see
        // `swim_start_submerged`); it stops, only while standing up fits, on no movement, out of water or surfacing.
        let moving = self.move_vector != [0.0, 0.0];
        // Releasing the sprint input ends the swim and its sprint together (strict BDS fuzz: the server's
        // drag and gravity turn non-swimming on that tick); a ceiling too low to stand keeps both.
        let keep_swim = !stand_fits || in_water && moving && !swim_surfacing && self.sprint;
        let swim = self.swim || if st.swimming { keep_swim } else { wanted && eyes_in_water };
        // While swimming, the sprint ends only with the swim's input or out of water (with StopSwimming).
        let keeps = if st.swimming { in_water && (self.sprint || !stand_fits) } else { sprinting };
        let starts = !st.sprinting && wanted;
        Frame {
            move_vector: self.move_vector,
            pitch: self.pitch,
            yaw: self.yaw,
            start_sprinting: starts,
            stop_sprinting: if st.sprinting { !keeps } else { starts && jump_in_water },
            sprint_down: self.sprint,
            start_sneaking: sneak && !st.sneaking,
            stop_sneaking: !sneak && st.sneaking,
            sneak_down: sneak,
            want_down: self.want_down,
            start_jumping: self.jump,
            jumping: self.jump,
            start_swimming: swim && !st.swimming,
            stop_swimming: !swim && st.swimming,
            start_gliding: self.glide && !st.gliding,
            stop_gliding: !self.glide && st.gliding,
            fly_tap,
            start_flying: flies && !st.flight.flying,
            stop_flying: !flies && st.flight.flying,
            using_item: self.using_item,
        }
    }
}

impl<W: WorldView + ?Sized> Sim<'_, W> {
    /// Applies pose, sprint and impulse changes. Returns (pose world known, processed move vector).
    pub(crate) fn apply_input(&self, st: &mut PlayerState, f: &Frame) -> (bool, [f32; 2]) {
        let was_flight_pose = st.gliding;
        let was_crawling = st.crawling;
        st.ensure_pose_heights();
        let available = self.pose_collisions_available(st);
        let mut known = available;
        let can_fit = |st: &PlayerState, height: f32, known: &mut bool| {
            let (fits, k) = self.can_fit_height_known(st, height);
            *known &= k;
            k && fits
        };

        st.prev_pitch = st.pitch;
        st.pitch = f.pitch;
        st.yaw = f.yaw;
        st.pressing_sneak = f.sneak_down;
        st.pressing_sprint = f.sprint_down;
        st.pressing_ascend = f.jumping;
        st.pressing_descend = f.sneak_down;
        st.want_down = f.want_down;

        st.sprint_start_cancelled = f.start_sprinting && f.stop_sprinting;
        if f.start_sprinting || f.stop_sprinting {
            st.sprinting = f.start_sprinting && !f.stop_sprinting;
            st.refresh_movement_speed();
        }
        st.air_speed = effective_air_speed(st);
        if f.start_flying || f.stop_flying {
            st.flight.flying = f.start_flying;
        } else if f.fly_tap {
            st.flight.trigger_ticks = FLY_TRIGGER_TICKS;
        }

        // A crawl ends as soon as standing or sneaking fits (vanilla client: StopCrawling, StartSneaking).
        if st.crawling && !st.swimming && !self.restore_upright_pose(st, available) {
            known = false;
        }
        let was_sneaking = st.sneaking;
        if f.start_sneaking {
            st.sneaking = true;
            if !st.crawling {
                st.size[1] = st.sneaking_height;
            }
        } else if f.stop_sneaking {
            if st.crawling {
                st.sneaking = false;
            } else if available && can_fit(st, st.standing_height, &mut known) {
                st.sneaking = false;
                st.size[1] = st.standing_height;
            } else {
                st.sneaking = true;
                st.size[1] = st.sneaking_height;
            }
        } else if st.crawling {
            st.sneaking = false;
        } else if f.sneak_down {
            st.sneaking = true;
            st.size[1] = st.sneaking_height;
        } else if st.sneaking && (!available || !can_fit(st, st.standing_height, &mut known)) {
            st.size[1] = st.sneaking_height;
        } else {
            st.sneaking = false;
            st.size[1] = st.standing_height;
        }

        let was_swimming = st.swimming;
        st.swam_before_input = was_swimming;
        if f.stop_swimming {
            st.swimming = false;
            st.swim_exit_jump_delay = JUMP_DELAY_TICKS;
            if !self.restore_upright_pose(st, available) {
                known = false;
            }
        } else if f.start_swimming {
            st.swimming = true;
            if st.swim_pose() || available && can_fit(st, st.standing_height, &mut known) {
                st.set_swimming_pose_flags();
            }
        }
        let step = if was_swimming { 0.1 } else { -0.1 };
        st.swim_amount = clamp(st.swim_amount + step, 0.0, 1.0);

        let mut max_impulse = 1.0f32;
        if f.using_item {
            max_impulse *= MAX_CONSUMING_IMPULSE;
        }
        // BDS keeps the sneak slowdown on the tick sneaking stops (bedsim drops it at once), and skips
        // it in water, where sneak means sink.
        let in_water = !self.touching_liquid_blocks(st, crate::world::LiquidKind::Water).is_empty();
        let sneak_held = was_sneaking || st.sneaking;
        // A crawl slows from the tick after it starts (vanilla capture). Sneak ticks in water still count
        // towards Swift Sneak (strict BDS fuzz).
        if sneak_held || was_crawling || st.gliding {
            st.ticks_since_can_slowdown += 1;
            if !in_water || was_crawling || st.gliding {
                max_impulse *= sneak_impulse(st, st.ticks_since_can_slowdown);
            }
        } else {
            st.ticks_since_can_slowdown = 0;
        }
        let mv = [
            clamp(f.move_vector[0], -1.0, 1.0) * max_impulse,
            clamp(f.move_vector[1], -1.0, 1.0) * max_impulse,
        ];

        st.jumping = f.start_jumping;
        st.pressing_jump = f.jumping;
        st.effective_jumping = f.jumping;
        st.jump_height = if st.jump_strength <= 0.0 { DEFAULT_JUMP_HEIGHT } else { st.jump_strength };
        if let Some(amp) = st.effects.jump_boost {
            st.jump_height += (amp + 1) as f32 * 0.1;
        }
        if !st.pressing_jump {
            st.jump_delay = 0;
        }
        if st.gravity == 0.0 {
            st.gravity = NORMAL_GRAVITY;
        }
        st.slow_falling = st.effects.slow_falling;

        if f.stop_gliding {
            st.gliding = false;
        } else if f.start_gliding {
            st.gliding = true;
        }
        if was_flight_pose && !st.gliding && !st.swim_pose() {
            known = self.restore_upright_pose(st, available) && known;
        }
        st.impulse = [mv[0] * 0.98, mv[1] * 0.98];
        (known, mv)
    }
}
