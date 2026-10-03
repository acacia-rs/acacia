//! Tick lifecycle (bedsim `Simulate`, `simulateCore`, `attemptTeleport`, `tickState`).

use crate::aabb::Aabb;
use crate::constants::*;
use crate::input::{Input, Surroundings};
use crate::math::Vec3;
use crate::state::PlayerState;
use crate::world::{LiquidKind, WorldView};

pub(crate) struct Sim<'w, W: WorldView + ?Sized> {
    pub w: &'w W,
}

/// Which path the tick took.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Normal,
    /// A queued teleport was applied; nothing else moved.
    Teleport,
    /// Part of the movement volume is not loaded; the player stays in place with zero velocity.
    Unloaded,
    Immobile,
}

/// What `PlayerAuthInput` needs from one simulated tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickOutput {
    /// Feet position at the end of the tick.
    pub position: Vec3,
    /// Eye position (feet + pose eye height) for `PlayerAuthInput.position`.
    pub eye_position: Vec3,
    /// Tick-end velocity (after gravity and drag): `PlayerAuthInput.delta`. Idle on ground: (0, -0.0784, 0).
    pub delta: Vec3,
    pub on_ground: bool,
    /// `VerticalCollision` flag.
    pub vertical_collision: bool,
    /// `HorizontalCollision` flag.
    pub horizontal_collision: bool,
    /// A ground jump fired this tick (`StartJumping`).
    pub jumped: bool,
    /// A queued teleport was consumed this tick (`HandledTeleport`).
    pub teleported: bool,
    /// Move vector after sneak/item slowdown (`PlayerAuthInput.move_vector`).
    pub move_vector: [f32; 2],
    pub outcome: Outcome,
}

/// Whether the player's box touches water where it stands now (before the next tick).
pub fn touching_water<W: WorldView + ?Sized>(st: &PlayerState, world: &W) -> bool {
    !Sim { w: world }.touching_liquid_blocks(st, LiquidKind::Water).is_empty()
}

/// Adds one tick of liquid current to the velocity without moving: BDS keeps pushing a player it
/// holds still after a teleport.
pub fn apply_current<W: WorldView + ?Sized>(st: &mut PlayerState, world: &W) {
    let sim = Sim { w: world };
    sim.prepare_collision_box(st);
    for kind in [LiquidKind::Water, LiquidKind::Lava] {
        let cells = sim.touching_liquid_blocks(st, kind);
        if !cells.is_empty() {
            sim.apply_liquid_flow(st, &cells, kind);
            return;
        }
    }
}

/// Simulates one 50 ms client tick.
pub fn tick<W: WorldView + ?Sized>(st: &mut PlayerState, input: &Input, world: &W) -> TickOutput {
    let sim = Sim { w: world };
    st.jumped = false;
    let stand_fits = !st.swimming || sim.can_fit_height_known(st, st.standing_height) != (false, true);
    let env = Surroundings {
        in_water: touching_water(st, world),
        swim_start_submerged: sim.swim_start_submerged(st),
        swim_surfacing: sim.swim_surfacing(st),
        stand_fits,
    };
    let frame = input.frame(st, &env);
    sim.prepare_collision_box(st);
    let pose = PoseSnapshot::take(st);
    st.liquid_box = Some(st.bounding_box());
    let (known, move_vector) = sim.apply_input(st, &frame);
    sim.update_freeze(st);
    let outcome = if known || st.pending_teleport.is_some() {
        sim.simulate_core(st)
    } else {
        freeze(st);
        Outcome::Unloaded
    };
    st.liquid_box = None;
    if outcome == Outcome::Unloaded {
        pose.restore(st);
    } else {
        st.air_speed = effective_air_speed(st);
        tick_state(st);
    }
    st.sprint_movement_blocked &= outcome == Outcome::Normal;
    TickOutput {
        position: st.pos,
        eye_position: st.eye_position(),
        delta: st.vel,
        on_ground: st.on_ground,
        vertical_collision: st.collide_y,
        horizontal_collision: st.collide_x || st.collide_z,
        jumped: st.jumped,
        teleported: outcome == Outcome::Teleport,
        move_vector,
        outcome,
    }
}

/// Pose fields restored when an unloaded tick could not be simulated.
struct PoseSnapshot {
    shape: Option<crate::state::CollisionShape>,
    size: Vec3,
    sneaking: bool,
    crawling: bool,
    swimming: bool,
    swim_amount: f32,
    gliding: bool,
}

impl PoseSnapshot {
    fn take(st: &PlayerState) -> Self {
        Self {
            shape: st.shape,
            size: st.size,
            sneaking: st.sneaking,
            crawling: st.crawling,
            swimming: st.swimming,
            swim_amount: st.swim_amount,
            gliding: st.gliding,
        }
    }

    fn restore(self, st: &mut PlayerState) {
        st.shape = self.shape;
        st.size = self.size;
        st.sneaking = self.sneaking;
        st.crawling = self.crawling;
        st.swimming = self.swimming;
        st.swim_amount = self.swim_amount;
        st.gliding = self.gliding;
    }
}

/// Zeroes velocity and drops retained water evidence for a tick that observed nothing.
fn freeze(st: &mut PlayerState) {
    st.set_vel([0.0; 3]);
    st.swim_water_grace_ticks = 0;
    st.swim_water_contact = false;
    st.stuck_speed_multiplier = [0.0; 3];
}

pub(crate) fn land_teleport(st: &mut PlayerState, pos: Vec3) {
    st.set_pos(pos);
    let bb = st.fresh_box();
    st.remember_box(bb);
    st.supporting_block = None;
    st.set_vel([0.0; 3]);
    st.jump_delay = 0;
    freeze(st);
}

pub(crate) fn effective_air_speed(st: &PlayerState) -> f32 {
    if st.sprinting { SPRINT_AIR_SPEED } else { WALK_AIR_SPEED }
}

fn tick_state(st: &mut PlayerState) {
    if st.glide_boost_ticks > 0 {
        st.glide_boost_ticks -= 1;
    }
    if st.dolphin_boost_ticks > 0 {
        st.dolphin_boost_ticks -= 1;
        if st.dolphin_boost_ticks <= 0 {
            st.dolphin_boost_ticks = 0;
            st.swim_speed_multiplier = DEFAULT_SWIM_SPEED_MULTIPLIER;
        }
    }
    st.knockback = None;
    if st.jump_delay > 0 {
        st.jump_delay -= 1;
    }
    st.swim_exit_jump_delay = st.swim_exit_jump_delay.saturating_sub(1);
    st.stopped_swimming_this_tick = false;
}

/// Block-aligned volume containing every lookup normal movement performs.
fn probe_area(bb: Aabb) -> Aabb {
    let g = bb.grow(1.0);
    Aabb::new(
        g.min[0].floor(),
        g.min[1].floor(),
        g.min[2].floor(),
        g.max[0].ceil() + 1.0,
        g.max[1].ceil() + 1.0,
        g.max[2].ceil() + 1.0,
    )
}

impl<W: WorldView + ?Sized> Sim<'_, W> {
    /// The supporting block if the box still lies over its collision shape (see `over_supporting_block`).
    pub(crate) fn support_under_box(&self, st: &PlayerState) -> Option<crate::math::BlockPos> {
        let sp = st.supporting_block?;
        let mut boxes = Vec::new();
        self.w.block_collisions(sp, &mut boxes);
        crate::motion::over_supporting_block(st, &boxes).then_some(sp)
    }

    fn simulate_core(&self, st: &mut PlayerState) -> Outcome {
        st.sprint_movement_blocked = false;
        st.ensure_pose_heights();
        if let Some(pos) = st.pending_teleport.take() {
            land_teleport(st, pos);
            return Outcome::Teleport;
        }
        self.prepare_collision_box(st);
        let current = st.bounding_box();
        if !self.loaded(&current) {
            freeze(st);
            return Outcome::Unloaded;
        }
        if st.immobile {
            freeze(st);
            return Outcome::Immobile;
        }
        let sweep = st.knockback.unwrap_or(st.vel);
        if !self.loaded(&probe_area(current.extend(sweep))) {
            freeze(st);
            return Outcome::Unloaded;
        }
        let pre = st.clone();
        if !self.simulate_movement(st) {
            *st = pre;
            freeze(st);
            return Outcome::Unloaded;
        }
        Outcome::Normal
    }

    /// Mid-tick check that the post-acceleration sweep is loaded.
    pub(crate) fn sweep_loaded(&self, st: &PlayerState) -> bool {
        self.loaded(&probe_area(st.bounding_box().extend(st.vel)))
    }
}
