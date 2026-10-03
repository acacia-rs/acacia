//! Movement constants (bedsim `constants.go`).

pub const DEFAULT_JUMP_HEIGHT: f32 = 0.42;
pub const DEFAULT_AIR_FRICTION: f32 = 0.91;
pub const DEFAULT_BLOCK_FRICTION: f32 = 0.6;
pub const NORMAL_GRAVITY_MULTIPLIER: f32 = 0.98;
pub const LEVITATION_GRAVITY_MULTIPLIER: f32 = 0.05;
pub const NORMAL_GRAVITY: f32 = 0.08;
pub const SLOW_FALLING_GRAVITY: f32 = 0.01;
pub const STEP_HEIGHT: f32 = 0.5625;
pub const SLIME_BOUNCE_MULTIPLIER: f32 = -1.0;
pub const SLIME_MIN_BOUNCE_SPEED: f32 = 0.08000012;
pub const BED_BOUNCE_MULTIPLIER: f32 = -0.75;
pub const CLIMB_SPEED: f32 = 0.2;
pub const MAX_CONSUMING_IMPULSE: f32 = 0.1225;
pub const MAX_SNEAK_IMPULSE: f32 = 0.3;
pub const DEFAULT_UNDERWATER_MOVEMENT_SPEED: f32 = 0.02;
pub const DEFAULT_LAVA_MOVEMENT_SPEED: f32 = 0.02;
pub const DEFAULT_SWIM_SPEED_MULTIPLIER: f32 = 1.0;
pub(crate) const GLIDE_FALL_DISTANCE_VELOCITY_THRESHOLD: f32 = -0.5;
/// Air acceleration picked by the sprint flag alone; it never scales with the movement attribute.
pub const WALK_AIR_SPEED: f32 = 0.02;
pub const SPRINT_AIR_SPEED: f32 = 0.026;
pub const WATER_DRAG: f32 = 0.8;
pub const DEFAULT_MOVEMENT_SPEED: f32 = 0.1;
pub const SPRINT_SPEED_MULTIPLIER: f32 = 1.3;

pub const DEFAULT_PLAYER_WIDTH: f32 = 0.6;
pub const DEFAULT_PLAYER_HEIGHT: f32 = 1.8;
pub const SNEAKING_PLAYER_HEIGHT: f32 = 1.49;
pub const CRAWLING_PLAYER_HEIGHT: f32 = 0.6;

/// Eye offsets above the feet; `PlayerAuthInput.position` is feet + this.
pub const DEFAULT_PLAYER_HEIGHT_OFFSET: f32 = 1.62;
pub const SNEAKING_PLAYER_HEIGHT_OFFSET: f32 = 1.27;
pub const COMPACT_PLAYER_HEIGHT_OFFSET: f32 = 0.4;

/// Emergent (never clamped): `(v - 0.08) * 0.98 = v`.
pub const TERMINAL_VELOCITY: f32 = -3.92;

pub const JUMP_DELAY_TICKS: u64 = 10;
pub const GLIDE_BOOST_TICKS: i64 = 20;
pub const DEFAULT_SWIM_WATER_GRACE_TICKS: i64 = 10;

/// Grounded acceleration adjustment on soul sand; drag keeps the plain friction.
pub const SOUL_SAND_ACCELERATION_FRICTION_MULTIPLIER: f32 = 1.225;
