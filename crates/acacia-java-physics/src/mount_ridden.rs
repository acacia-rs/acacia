//! Each mount's `getRiddenInput`, `tickRidden`, `getRiddenSpeed` and rider jumps.

use crate::collide::Blocks;
use crate::mount::{Boost, Mount, MountKind, Rider};
use crate::mth::{cos, look_vector, normalize, sin, wrap_degrees, DEG_TO_RAD};

/// `PlayerRideableJumping.getPlayerJumpPendingScale`.
fn jump_scale(amount: i32) -> f32 {
    if amount >= 90 { 1.0 } else { 0.4 + 0.4 * amount as f32 / 90.0 }
}

/// The happy ghast's and nautilus' input: forward and up follow the rider's pitch; backwards is half as strong.
fn look_input(rider: Rider) -> [f32; 3] {
    if rider.zza == 0.0 {
        return [rider.xxa, 0.0, 0.0];
    }
    let pitch = f64::from(rider.pitch * DEG_TO_RAD);
    let (mut forward, mut up) = (cos(pitch), -sin(pitch));
    if rider.zza < 0.0 {
        forward *= -0.5;
        up *= -0.5;
    }
    [rider.xxa, up, forward]
}

impl Boost {
    /// `ItemBasedSteering.tickBoost`; false once the boost ran out.
    fn tick(&mut self) -> bool {
        let running = self.time <= self.total;
        self.time += 1;
        running
    }

    fn factor(&self) -> f32 {
        1.0 + 1.15 * sin(f64::from(self.time as f32 / self.total as f32 * std::f32::consts::PI))
    }
}

impl Mount {
    /// A camel sitting down or getting up (`refuseToMove`), by the client's game time.
    pub fn refuses_to_move(&self) -> bool {
        if self.kind != MountKind::Camel {
            return false;
        }
        let sitting = self.last_pose_change < 0;
        let pose_time = self.game_time - self.last_pose_change.abs();
        sitting || pose_time < if sitting { 40 } else { 52 }
    }

    /// Whether the rider's jump key charges a jump (`jumpableVehicle`).
    pub fn can_jump(&self) -> bool {
        match self.kind {
            MountKind::Horse | MountKind::SkeletonHorse | MountKind::Nautilus => true,
            MountKind::Camel => !self.refuses_to_move(),
            _ => false,
        }
    }

    pub fn jump_cooldown(&self) -> i32 {
        if matches!(self.kind, MountKind::Camel | MountKind::Nautilus) { self.dash_cooldown } else { 0 }
    }

    /// `onPlayerJump` with the released charge, out of 100.
    pub fn on_player_jump(&mut self, amount: i32) {
        match self.kind {
            MountKind::Nautilus if self.dash_cooldown <= 0 => self.jump_pending = jump_scale(amount),
            MountKind::Camel if self.dash_cooldown > 0 || !self.body.on_ground => {}
            MountKind::Horse | MountKind::SkeletonHorse | MountKind::Camel => {
                let amount = if amount < 0 {
                    0
                } else {
                    self.allow_stand_sliding = true;
                    (self.standing, self.stand_counter) = (true, 20);
                    amount
                };
                self.jump_pending = jump_scale(amount);
            }
            _ => {}
        }
    }

    pub(crate) fn ridden_input(&self, rider: Rider) -> [f64; 3] {
        match self.kind {
            MountKind::Pig | MountKind::Strider => [0.0, 0.0, 1.0],
            MountKind::HappyGhast => {
                let [x, mut y, z] = look_input(rider);
                if rider.jumping {
                    y += 0.5;
                }
                [x, y, z].map(|v| v as f64 * (3.9_f32 as f64 * self.flying_speed))
            }
            MountKind::Nautilus => look_input(rider).map(f64::from),
            _ if self.refuses_to_move() => [0.0; 3],
            _ => {
                if self.body.on_ground && self.jump_pending == 0.0 && self.standing && !self.allow_stand_sliding {
                    return [0.0; 3];
                }
                let forward = if rider.zza <= 0.0 { rider.zza * 0.25 } else { rider.zza };
                [(rider.xxa * 0.5) as f64, 0.0, forward as f64]
            }
        }
    }

    /// `setRot`: yaw wraps below 360, pitch also clamps to ±90.
    fn set_rot(&mut self, yaw: f32, pitch: f32) {
        self.body.yaw = yaw % 360.0;
        self.body.pitch = (pitch % 360.0 % 360.0).clamp(-90.0, 90.0);
    }

    pub(crate) fn tick_ridden(&mut self, world: &impl Blocks, rider: Rider, input: [f64; 3]) {
        let pitch = rider.pitch * 0.5;
        match self.kind {
            MountKind::Pig | MountKind::Strider => {
                self.set_rot(rider.yaw, pitch);
                if let Some(boost) = self.boost.as_mut() {
                    if !boost.tick() {
                        self.boost = None;
                    }
                }
            }
            MountKind::HappyGhast | MountKind::Nautilus => {
                let ease = if self.kind == MountKind::HappyGhast { 0.08 } else { 0.5 };
                let yaw = self.body.yaw + wrap_degrees(rider.yaw - self.body.yaw) * ease;
                self.set_rot(yaw, pitch);
                if self.kind == MountKind::Nautilus && self.jump_pending > 0.0 {
                    self.nautilus_dash(world, rider, self.jump_pending);
                }
                self.jump_pending = 0.0;
            }
            _ => {
                if !self.refuses_to_move() {
                    self.set_rot(rider.yaw, pitch);
                }
                if self.body.on_ground {
                    if self.jump_pending > 0.0 {
                        self.riders_jump(world, self.jump_pending, input);
                    }
                    self.jump_pending = 0.0;
                }
                let settled = self.game_time - self.last_pose_change.abs() >= 40;
                if self.kind == MountKind::Camel && rider.zza > 0.0 && self.last_pose_change < 0 && settled {
                    self.last_pose_change = self.game_time;
                }
            }
        }
    }

    pub(crate) fn ridden_speed(&self, rider: Rider) -> f32 {
        let boost = self.boost.map_or(1.0, |b| b.factor()) as f64;
        match self.kind {
            MountKind::Camel => {
                let bonus = if rider.sprinting && self.dash_cooldown == 0 { 0.1 } else { 0.0 };
                self.movement_speed as f32 + bonus
            }
            MountKind::Pig => (self.movement_speed * 0.225 * boost) as f32,
            MountKind::Strider => {
                let (speed, factor) = if self.suffocating {
                    (self.movement_speed + self.movement_speed * -0.34_f32 as f64, 0.35_f32)
                } else {
                    (self.movement_speed, 0.55)
                };
                (speed * factor as f64 * boost) as f32
            }
            MountKind::Nautilus => {
                let factor = if self.body.in_water { 0.0325 } else { 0.02 };
                factor * self.movement_speed as f32
            }
            _ => self.movement_speed as f32,
        }
    }

    /// `executeRidersJump`: a horse leaps, a camel dashes.
    fn riders_jump(&mut self, world: &impl Blocks, amount: f32, input: [f64; 3]) {
        let body = &mut self.body;
        if self.kind == MountKind::Camel {
            let momentum = body.jump_power(world, self.jump_strength as f32, 1.0) as f64;
            let factor = body.support.speed_factor(world, body.pos) as f64;
            let look = look_vector(body.pitch, body.yaw);
            let flat = normalize([look[0], 0.0, look[2]]);
            let scale = (22.2222_f32 * amount) as f64 * self.movement_speed * factor;
            let up = (1.4285_f32 * amount) as f64 * momentum;
            let dash = [flat[0] * scale + 0.0, flat[1] * scale + up, flat[2] * scale + 0.0];
            body.delta = [0, 1, 2].map(|a| body.delta[a] + dash[a]);
            (self.dash_cooldown, self.dashing) = (55, true);
            return;
        }
        body.delta[1] = body.jump_power(world, self.jump_strength as f32, amount) as f64;
        if input[2] > 0.0 {
            let radians = f64::from(body.yaw * DEG_TO_RAD);
            let (s, c) = (sin(radians), cos(radians));
            body.delta = [body.delta[0] + (-0.4 * s * amount) as f64, body.delta[1] + 0.0, body.delta[2] + (0.4 * c * amount) as f64];
        }
    }

    /// The nautilus' `executeRidersJump`: a dash along the rider's look.
    fn nautilus_dash(&mut self, world: &impl Blocks, rider: Rider, amount: f32) {
        let body = &mut self.body;
        let factor = body.support.speed_factor(world, body.pos) as f64;
        let strength = if body.in_water { 1.2_f32 } else { 0.5 };
        let scale = (strength * amount) as f64 * self.movement_speed * factor;
        let look = look_vector(rider.pitch, rider.yaw);
        for axis in 0..3 {
            body.delta[axis] += look[axis] * scale;
        }
        (self.dash_cooldown, self.dashing) = (40, true);
    }
}
