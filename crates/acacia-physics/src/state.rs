use crate::aabb::Aabb;
use crate::constants::*;
use crate::math::{BlockPos, Vec3, add};

/// Active status effects that change movement. Speed/Slowness are not here: they are part of
/// the server's movement attribute (`PlayerState::movement_speed`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Effects {
    /// Amplifier (level - 1).
    pub jump_boost: Option<i32>,
    pub levitation: Option<i32>,
    pub slow_falling: bool,
    pub weaving: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Equipment {
    pub depth_strider: i32,
    pub soul_speed: i32,
    pub swift_sneak: i32,
    pub leather_boots: bool,
    pub elytra: bool,
}

/// Retained native box endpoints after a sweep (bedsim `collisionShape`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CollisionShape {
    pub bb: Aabb,
    pub pos: Vec3,
    pub dims: [f32; 2],
}

/// Authoritative movement state of the local player (bedsim `MovementState`).
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerState {
    /// Feet position (centre of the box bottom).
    pub pos: Vec3,
    pub last_pos: Vec3,
    /// Velocity carried into the next tick; after `tick` this is the `PlayerAuthInput.delta`.
    pub vel: Vec3,
    pub last_vel: Vec3,
    /// Displacement actually applied this tick.
    pub mov: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// The pitch before `pitch` (the previous rotation BDS keeps).
    pub prev_pitch: f32,
    pub impulse: [f32; 2],
    /// (width, height, scale).
    pub size: Vec3,
    pub stuck_speed_multiplier: Vec3,
    pub standing_height: f32,
    pub sneaking_height: f32,
    pub crawling_height: f32,
    pub supporting_block: Option<BlockPos>,

    pub gravity: f32,
    pub jump_height: f32,
    /// Base jump velocity; 0 uses 0.42.
    pub jump_strength: f32,
    pub fall_distance: f32,
    /// Effective movement attribute (incl. Speed/Slowness and sprint).
    pub movement_speed: f32,
    /// Effective movement attribute without the sprint modifier.
    pub default_movement_speed: f32,
    pub air_speed: f32,
    pub underwater_movement_speed: f32,
    pub lava_movement_speed: f32,
    pub swim_speed_multiplier: f32,
    pub dolphin_boost_ticks: i64,
    /// The server sent a movement attribute since the last sprint toggle.
    pub server_updated_speed: bool,

    pub knockback: Option<Vec3>,
    pub pending_teleport: Option<Vec3>,

    pub sprinting: bool,
    /// A sprint started and was cancelled on the last tick (both StartSprinting and StopSprinting are sent).
    pub sprint_start_cancelled: bool,
    pub pressing_sprint: bool,
    pub sneaking: bool,
    pub pressing_sneak: bool,
    pub pressing_ascend: bool,
    pub pressing_descend: bool,
    pub want_down: bool,
    pub jumping: bool,
    pub pressing_jump: bool,
    pub effective_jumping: bool,
    pub jump_delay: u64,
    /// Jump cooldown after a swim ends; unlike `jump_delay`, releasing jump does not clear it (vanilla
    /// client; BDS follows the client's StartJumping, so only captures show it).
    pub(crate) swim_exit_jump_delay: u64,

    pub swimming: bool,
    pub swim_amount: f32,
    pub stopped_swimming_this_tick: bool,
    pub(crate) swim_water_contact: bool,
    /// The box before this tick's pose change, which liquid contact uses (BDS changes the pose after moving).
    pub(crate) liquid_box: Option<Aabb>,
    pub swim_water_grace_ticks: i64,

    pub collide_x: bool,
    pub collide_y: bool,
    pub collide_z: bool,
    pub on_ground: bool,
    pub penetrated_last_frame: bool,
    pub stuck_in_collider: bool,
    /// Server-set immobile flag: movement is frozen.
    pub immobile: bool,

    pub gliding: bool,
    pub glide_boost_ticks: i64,
    pub has_gravity: bool,
    pub slow_falling: bool,
    pub crawling: bool,
    pub ticks_since_can_slowdown: i32,

    pub effects: Effects,
    pub equipment: Equipment,

    /// Sprint stalled against a wall during the last tick (stops sprinting next tick).
    pub sprint_movement_blocked: bool,
    pub(crate) jumped: bool,
    pub(crate) shape: Option<CollisionShape>,
}

impl PlayerState {
    pub fn new(pos: Vec3) -> Self {
        Self {
            pos,
            last_pos: pos,
            vel: [0.0; 3],
            last_vel: [0.0; 3],
            mov: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            prev_pitch: 0.0,
            impulse: [0.0; 2],
            size: [DEFAULT_PLAYER_WIDTH, DEFAULT_PLAYER_HEIGHT, 1.0],
            stuck_speed_multiplier: [0.0; 3],
            standing_height: 0.0,
            sneaking_height: 0.0,
            crawling_height: 0.0,
            supporting_block: None,
            gravity: 0.0,
            jump_height: 0.0,
            jump_strength: 0.0,
            fall_distance: 0.0,
            movement_speed: DEFAULT_MOVEMENT_SPEED,
            default_movement_speed: DEFAULT_MOVEMENT_SPEED,
            air_speed: WALK_AIR_SPEED,
            underwater_movement_speed: 0.0,
            lava_movement_speed: 0.0,
            swim_speed_multiplier: 0.0,
            dolphin_boost_ticks: 0,
            server_updated_speed: false,
            knockback: None,
            pending_teleport: None,
            sprinting: false,
            sprint_start_cancelled: false,
            pressing_sprint: false,
            sneaking: false,
            pressing_sneak: false,
            pressing_ascend: false,
            pressing_descend: false,
            want_down: false,
            jumping: false,
            pressing_jump: false,
            effective_jumping: false,
            jump_delay: 0,
            swim_exit_jump_delay: 0,
            swimming: false,
            swim_amount: 0.0,
            stopped_swimming_this_tick: false,
            swim_water_contact: false,
            liquid_box: None,
            swim_water_grace_ticks: 0,
            collide_x: false,
            collide_y: false,
            collide_z: false,
            on_ground: false,
            penetrated_last_frame: false,
            stuck_in_collider: false,
            immobile: false,
            gliding: false,
            glide_boost_ticks: 0,
            has_gravity: true,
            slow_falling: false,
            crawling: false,
            ticks_since_can_slowdown: 0,
            effects: Effects::default(),
            equipment: Equipment::default(),
            sprint_movement_blocked: false,
            jumped: false,
            shape: None,
        }
    }

    /// Swim hitbox: swimming with water contact this tick or within the grace window.
    pub fn swim_pose(&self) -> bool {
        self.swimming && (self.swim_water_contact || self.swim_water_grace_ticks > 0)
    }

    /// Eye position for `PlayerAuthInput.position`.
    pub fn eye_position(&self) -> Vec3 {
        let offset = if self.swim_pose() || self.crawling || self.gliding {
            COMPACT_PLAYER_HEIGHT_OFFSET
        } else if self.sneaking {
            SNEAKING_PLAYER_HEIGHT_OFFSET
        } else {
            DEFAULT_PLAYER_HEIGHT_OFFSET
        };
        let scale = if self.size[2] <= 0.0 { 1.0 } else { self.size[2] };
        add(self.pos, [0.0, offset * scale, 0.0])
    }

    /// Queues a server teleport (feet position); the next `tick` lands there with zero velocity.
    pub fn queue_teleport(&mut self, feet: Vec3) {
        self.pending_teleport = Some(feet);
    }

    /// Queues a velocity replacement (`SetEntityMotion`) applied at the start of the next tick.
    pub fn queue_knockback(&mut self, velocity: Vec3) {
        self.knockback = Some(velocity);
    }

    /// Applies a server movement attribute (`minecraft:movement`) value that excludes sprint.
    pub fn set_movement_attribute(&mut self, without_sprint: f32) {
        self.default_movement_speed = without_sprint;
        self.movement_speed =
            if self.sprinting { without_sprint * SPRINT_SPEED_MULTIPLIER } else { without_sprint };
        self.server_updated_speed = true;
    }

    /// Rewinds to a server-corrected state (`CorrectPlayerMovePrediction`).
    pub fn apply_correction(&mut self, feet: Vec3, delta: Vec3, on_ground: bool) {
        self.last_pos = self.pos;
        self.pos = feet;
        self.last_vel = self.vel;
        self.vel = delta;
        self.on_ground = on_ground;
        self.supporting_block = None;
    }

    pub(crate) fn set_vel(&mut self, v: Vec3) {
        self.last_vel = self.vel;
        self.vel = v;
    }

    pub(crate) fn set_pos(&mut self, p: Vec3) {
        self.last_pos = self.pos;
        self.pos = p;
    }

    pub(crate) fn ensure_pose_heights(&mut self) {
        if self.standing_height <= 0.0 {
            self.standing_height = if !self.sneaking && !self.crawling && self.size[1] > 0.0 {
                self.size[1]
            } else {
                DEFAULT_PLAYER_HEIGHT
            };
        }
        if self.sneaking_height <= 0.0 {
            self.sneaking_height = SNEAKING_PLAYER_HEIGHT;
        }
        if self.crawling_height <= 0.0 {
            self.crawling_height = CRAWLING_PLAYER_HEIGHT;
        }
    }
}
