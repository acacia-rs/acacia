//! A copy of the simulation that steps ahead without touching the live one, for planning and prediction:
//! Bedrock's [`Sim`].

use acacia_physics::constants::{
    CLIMB_SPEED, DEFAULT_GROUND_DRAG, DEFAULT_MOVEMENT_SPEED, DEFAULT_PLAYER_HEIGHT_OFFSET, JUMP_DELAY_TICKS, NORMAL_GRAVITY,
    NORMAL_GRAVITY_MULTIPLIER, SNEAKING_PLAYER_HEIGHT, STEP_HEIGHT,
};
use acacia_physics::{self as physics, Aabb, BlockPos, LiquidKind, PlayerState, TickOutput, Vec3, WorldView};

use super::keys::KeyLatch;
use super::sim::{Body, Envelope, Live, Motion, Sim, SimWorld, StepIn, Traits};
use super::{Controls, Movement};

/// Bedrock's movement constants.
pub const BEDROCK: Envelope = Envelope {
    walk_speed: 4.317,
    sprint_speed: 5.612,
    swim_speed: 1.960,
    sneak_speed: 1.3,
    climb_speed: CLIMB_SPEED,
    gravity: NORMAL_GRAVITY,
    air_drag: NORMAL_GRAVITY_MULTIPLIER,
    ground_drag: DEFAULT_GROUND_DRAG,
    friction: 0.6,
    step_height: STEP_HEIGHT,
    jump_clear: 1.2,
    jump_rise: 1.25,
    eye_height: DEFAULT_PLAYER_HEIGHT_OFFSET,
    movement_speed: DEFAULT_MOVEMENT_SPEED,
    // Measured by `pathfind::tests::hops`.
    leap_vmax: 0.7,
    hop_speed: 0.3563,
    hop_ticks: 12.0,
    cost_heuristic: 3.563,
};

/// The simulation and key latches only: no server history, teleports or packets.
#[derive(Debug, Clone, PartialEq)]
pub struct Rollout {
    pub st: PlayerState,
    pub(crate) keys: KeyLatch,
    pub using_item: bool,
}

impl Rollout {
    /// A rollout from `st` with no keys held before.
    pub fn new(st: PlayerState) -> Self {
        Self { st, keys: KeyLatch::default(), using_item: false }
    }

    /// Simulates one tick with `c` held, as [`Movement::tick`] does.
    pub fn step(&mut self, c: &Controls, world: &(impl WorldView + ?Sized)) -> TickOutput {
        let input = self.keys.advance(c, &self.st, world, self.using_item, c.sneak);
        physics::tick(&mut self.st, &input, world)
    }
}

impl Sim for Rollout {
    fn envelope() -> &'static Envelope {
        &BEDROCK
    }

    fn standing(feet: Vec3) -> Self {
        let mut st = PlayerState::new(feet);
        st.on_ground = true;
        Self::new(st)
    }

    fn body(&self) -> Body {
        let st = &self.st;
        Body {
            pos: st.pos,
            vel: st.vel,
            yaw: st.yaw,
            on_ground: st.on_ground,
            h_collision: st.collide_x || st.collide_z,
            bonk: st.collide_y && !st.on_ground,
            fall: st.fall_distance,
        }
    }

    fn motion(&self) -> Motion {
        let st = &self.st;
        Motion {
            pos: st.pos,
            vel: st.vel,
            yaw: st.yaw,
            on_ground: st.on_ground,
            sprinting: st.sprinting,
            sneaking: st.sneaking,
            pressing_jump: st.pressing_jump,
            jump_delay: st.jump_delay.min(JUMP_DELAY_TICKS) as u8,
        }
    }

    fn synth(&self, m: &Motion) -> Self {
        let base = &self.st;
        let mut st = PlayerState::new(m.pos);
        st.vel = m.vel;
        st.last_vel = st.vel;
        st.yaw = m.yaw;
        st.on_ground = m.on_ground;
        st.sprinting = m.sprinting;
        st.sneaking = m.sneaking;
        if st.sneaking {
            st.size[1] = SNEAKING_PLAYER_HEIGHT;
        }
        st.pressing_jump = m.pressing_jump;
        st.jump_delay = u64::from(m.jump_delay);
        st.effects = base.effects;
        st.equipment = base.equipment;
        st.flight = base.flight;
        st.jump_strength = base.jump_strength;
        st.freeze = base.freeze;
        st.set_movement_attribute(base.movement_attribute);
        Self::new(st)
    }

    fn set_motion(&mut self, pos: Vec3, vel: Vec3, yaw: f32) {
        (self.st.pos, self.st.vel, self.st.yaw) = (pos, vel, yaw);
    }

    fn traits(&self) -> Traits {
        let e = &self.st.effects;
        Traits { speed: self.st.movement_attribute, jump_boost: e.jump_boost.is_some(), altered_fall: e.levitation.is_some() || e.slow_falling }
    }
}

impl<W: WorldView + ?Sized> StepIn<W> for Rollout {
    fn step(&mut self, c: &Controls, w: &W) {
        Rollout::step(self, c, w);
    }
}

impl<W: WorldView + ?Sized> SimWorld for W {
    fn hazard(&self, p: BlockPos) -> bool {
        let b = self.block(p);
        b.cobweb || b.liquid.is_some_and(|l| l.kind == LiquidKind::Lava)
    }

    fn collides(&self, area: &Aabb) -> bool {
        let mut boxes = Vec::new();
        self.collisions(area, &mut boxes);
        !boxes.is_empty()
    }

    fn friction(&self, p: BlockPos) -> f32 {
        self.block(p).friction
    }
}

impl Movement {
    /// The simulation as of now, to step ahead of the live one (`None` before movement starts).
    pub fn fork(&self) -> Option<Rollout> {
        let mut st = self.physics.clone()?;
        st.equipment = self.equipment;
        st.equipment.elytra = self.elytra;
        Some(Rollout { st, keys: self.keys.clone(), using_item: self.using_item })
    }

    /// The live simulation state (`None` before movement starts).
    pub fn state(&self) -> Option<&PlayerState> {
        self.physics.as_ref()
    }
}

impl Live for Movement {
    type Sim = Rollout;

    fn base(&self) -> Option<Rollout> {
        self.physics.clone().map(Rollout::new)
    }

    fn fork(&self) -> Option<Rollout> {
        Movement::fork(self)
    }

    fn on_ground(&self) -> bool {
        Movement::on_ground(self)
    }

    fn teleports(&self) -> u32 {
        self.teleports
    }

    fn setback_age(&self) -> Option<u64> {
        Movement::setback_age(self)
    }

    fn controls(&mut self) -> &mut Controls {
        &mut self.controls
    }
}
