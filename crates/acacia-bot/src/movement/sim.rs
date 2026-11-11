//! What planning and following need of an edition's player simulation (docs/java-edition.md "backend split"):
//! step a copy one tick with held [`Controls`], read its [`Body`], restart it at a planned node, and the
//! edition's movement constants ([`Envelope`]). Bedrock's is [`Rollout`](super::Rollout).

use acacia_physics::{Aabb, BlockPos, Vec3};

use super::Controls;

/// One edition's player simulation, as planning and following step it.
pub trait Sim: Clone + PartialEq + Send + Sync + 'static {
    /// The edition's movement constants.
    fn envelope() -> &'static Envelope;
    /// Standing on the ground with feet at `feet`, nothing else set.
    fn standing(feet: Vec3) -> Self;
    fn body(&self) -> Body;
    /// What a planned node keeps of this state.
    fn motion(&self) -> Motion;
    /// A fresh state moving as `m`; attributes, effects, equipment and flight from `self` (the live state), keys
    /// released and everything transient from rest.
    fn synth(&self, m: &Motion) -> Self;
    fn set_motion(&mut self, pos: Vec3, vel: Vec3, yaw: f32);
    fn traits(&self) -> Traits;
}

/// Stepping a [`Sim`] in world `W`.
pub trait StepIn<W: ?Sized>: Sim {
    /// One tick with `c` held, key latches included, exactly as the live bot runs it.
    fn step(&mut self, c: &Controls, w: &W);
}

/// What the follower reads of the world it steps in, besides stepping.
pub trait SimWorld {
    /// Lava or a cobweb: what a live re-solve or shortcut must not touch.
    fn hazard(&self, p: BlockPos) -> bool;
    /// Some collision box intersects `area` (world space).
    fn collides(&self, area: &Aabb) -> bool;
    fn friction(&self, p: BlockPos) -> f32;
}

/// What a controller sees of the player each tick, in rollouts and live alike.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    /// Feet.
    pub pos: Vec3,
    pub vel: Vec3,
    /// Degrees, as last sent.
    pub yaw: f32,
    pub on_ground: bool,
    pub h_collision: bool,
    /// Hit a ceiling this tick.
    pub bonk: bool,
    /// Blocks fallen since last on the ground.
    pub fall: f32,
}

impl Body {
    pub fn h_speed(&self) -> f32 {
        self.vel[0].hypot(self.vel[2])
    }

    pub fn cell(&self) -> [i32; 2] {
        [self.pos[0].floor() as i32, self.pos[2].floor() as i32]
    }

    /// Largest per-axis position difference.
    pub fn error(&self, other: &Body) -> f32 {
        (0..3).map(|i| (self.pos[i] - other.pos[i]).abs()).fold(0.0, f32::max)
    }
}

/// The motion a planned node keeps: enough to start a simulation there again.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Motion {
    /// Feet.
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub on_ground: bool,
    pub sprinting: bool,
    pub sneaking: bool,
    pub pressing_jump: bool,
    /// Ticks before the next jump, at most the edition's jump delay.
    pub jump_delay: u8,
}

/// The state's traits that change how far and how it moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Traits {
    /// The movement speed attribute ([`Envelope::movement_speed`] unmodified).
    pub speed: f32,
    pub jump_boost: bool,
    /// Levitation or slow falling.
    pub altered_fall: bool,
}

/// An edition's movement constants, as the planner's costs and bounds use them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Envelope {
    /// Blocks per second (Baritone's `ActionCosts` speeds).
    pub walk_speed: f32,
    pub sprint_speed: f32,
    pub swim_speed: f32,
    pub sneak_speed: f32,
    /// Blocks per tick on a ladder or vine.
    pub climb_speed: f32,
    /// A fall from rest: each tick `v = (v + gravity) * air_drag`.
    pub gravity: f32,
    pub air_drag: f32,
    /// Horizontal speed kept per tick on default ground.
    pub ground_drag: f32,
    /// Default block friction.
    pub friction: f32,
    pub step_height: f32,
    /// Highest feet rise a standing jump clears.
    pub jump_clear: f32,
    /// Least height a jump rises (no effects; jump boost only adds).
    pub jump_rise: f32,
    /// Eye above the feet, standing.
    pub eye_height: f32,
    /// The default movement speed attribute.
    pub movement_speed: f32,
    /// Fastest horizontal travel in one tick any leap reaches without effects.
    pub leap_vmax: f32,
    /// Steady sprint-jumping on flat ground: blocks per tick, and ticks per hop.
    pub hop_speed: f32,
    pub hop_ticks: f32,
    /// Sprint cost per block: turns heuristic distance into ticks (Baritone `costHeuristic`).
    pub cost_heuristic: f32,
}

/// The live movement a pilot drives: its simulation and controls.
pub trait Live {
    type Sim: Sim;
    /// The live state with no keys held: what plans start from (`None` before movement starts).
    fn base(&self) -> Option<Self::Sim>;
    /// The live simulation with its key latches, to step ahead of it (`None` before movement starts).
    fn fork(&self) -> Option<Self::Sim>;
    fn on_ground(&self) -> bool;
    /// Server teleports so far.
    fn teleports(&self) -> u32;
    /// Ticks since the last server setback.
    fn setback_age(&self) -> Option<u64>;
    fn controls(&mut self) -> &mut Controls;
}
