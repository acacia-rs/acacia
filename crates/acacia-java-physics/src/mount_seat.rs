//! Where a vehicle seats its rider (`positionRider`): the passenger attachment turned by the vehicle's yaw, plus the
//! client-side animations that move a seat: a rearing horse's `standAnimO` and a walking strider's bob.

use crate::mount::{Mount, MountKind};
use crate::mth::{cos, sin, DEG_TO_RAD};

/// `Vec3.yRot(-yaw * DEG_TO_RAD)`: an attachment point turned with a vehicle facing `yaw`.
pub fn turned(point: [f64; 3], yaw: f32) -> [f64; 3] {
    let radians = f64::from(-yaw * DEG_TO_RAD);
    let (s, c) = (f64::from(sin(radians)), f64::from(cos(radians)));
    let [x, y, z] = point;
    [x * c + z * s, y, z * c - x * s]
}

/// A player's feet on a vehicle at `pos` turned to `yaw`, from the vehicle's passenger attachment `seat` and the
/// player's own vehicle attachment height ([`crate::Player::vehicle_attachment`]).
pub fn seat_feet(pos: [f64; 3], seat: [f64; 3], yaw: f32, attachment: f64) -> [f64; 3] {
    feet_at(pos, turned(seat, yaw), attachment)
}

/// `positionRider` with an already turned attachment point: `getPassengerRidingPosition` less the rider's own.
pub fn feet_at(pos: [f64; 3], point: [f64; 3], attachment: f64) -> [f64; 3] {
    [pos[0] + point[0], pos[1] + point[1] - attachment, pos[2] + point[2]]
}

/// `Vec3.add` of two turned points.
fn plus(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// The animation state a seat follows, ticked as the client ticks the vehicle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SeatAnim {
    /// `AbstractHorse.standAnim` and `standAnimO`.
    pub stand: f32,
    pub stand_old: f32,
    /// `WalkAnimationState` speed and position.
    pub walk_speed: f32,
    pub walk_position: f32,
}

impl SeatAnim {
    /// `AbstractHorse.tick`'s rear animation, after the stand counter ran.
    pub fn tick_stand(&mut self, standing: bool) {
        self.stand_old = self.stand;
        let s = self.stand;
        if standing {
            self.stand = (s + ((1.0 - s) * 0.4 + 0.05)).min(1.0);
        } else {
            self.stand = (s + ((0.8 * s * s * s - s) * 0.6 - 0.05)).max(0.0);
        }
    }

    /// `calculateEntityAnimation(false)` for a mob that isn't a passenger: the horizontal distance it moved this
    /// tick drives the walk (`updateWalkAnimation`; position scale 1 for adults, so only `position` matters).
    pub fn tick_walk(&mut self, from: [f64; 3], to: [f64; 3]) {
        let (dx, dz) = (to[0] - from[0], to[2] - from[2]);
        let distance = (dx * dx + dz * dz).sqrt() as f32;
        let target = (distance * 4.0).min(1.0);
        self.walk_speed += (target - self.walk_speed) * 0.4;
        self.walk_position += self.walk_speed;
    }

    /// `AbstractHorse.getPassengerAttachmentPoint`'s addition, turned: back and up as the horse rears.
    pub fn horse_rear(&self, scale: f32, yaw: f32) -> [f64; 3] {
        let (o, scale) = (f64::from(self.stand_old), f64::from(scale));
        turned([0.0, 0.15 * o * scale, -0.7 * o * scale], yaw)
    }

    /// `Strider.getPassengerAttachmentPoint`'s client-side bob.
    pub fn strider_bob(&self, scale: f32) -> f64 {
        let speed = self.walk_speed.min(0.25);
        let offset = 0.12 * cos(f64::from(self.walk_position * 1.5)) * 2.0 * speed;
        f64::from(offset * scale)
    }

    /// The turned attachment point of a vehicle of `kind`, `seat` unturned, with its animation's share.
    pub fn point(&self, kind: Option<MountKind>, seat: [f64; 3], scale: f32, yaw: f32) -> [f64; 3] {
        let base = turned(seat, yaw);
        match kind {
            Some(MountKind::Horse | MountKind::SkeletonHorse) => plus(base, self.horse_rear(scale, yaw)),
            Some(MountKind::Strider) => [base[0] + 0.0, base[1] + self.strider_bob(scale), base[2] + 0.0],
            _ => base,
        }
    }
}

impl Mount {
    /// `positionRider`: the rider's feet on the turned seat, moved by a rear or a bob.
    pub fn rider_feet(&self, attachment: f64) -> [f64; 3] {
        let point = self.anim.point(Some(self.kind), self.seat, self.seat_scale, self.body.yaw);
        feet_at(self.body.pos, point, attachment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rear_eases_in_and_its_seat_lags_a_tick() {
        let mut anim = SeatAnim::default();
        anim.tick_stand(true);
        assert!(anim.stand_old == 0.0 && (anim.stand - 0.45).abs() < 1e-6, "{anim:?}");
        for _ in 0..10 {
            anim.tick_stand(true);
        }
        assert_eq!(anim.stand, 1.0);
        let back = anim.horse_rear(1.0, 0.0);
        assert!((back[2] + 0.7).abs() < 1e-9 && (back[1] - 0.15).abs() < 1e-9, "{back:?}");
        for _ in 0..10 {
            anim.tick_stand(false);
        }
        assert_eq!(anim.stand, 0.0);
    }

    #[test]
    fn a_still_strider_does_not_bob() {
        let mut anim = SeatAnim::default();
        anim.tick_walk([0.0; 3], [0.0; 3]);
        assert_eq!(anim.strider_bob(1.0), 0.0);
        anim.tick_walk([0.0; 3], [0.1, 0.0, 0.0]);
        assert!(anim.strider_bob(1.0) != 0.0);
    }

    #[test]
    fn turning_a_side_seat_keeps_vanillas_add_order() {
        let feet = seat_feet([10.0, 64.0, 20.0], [-1.7, 4.0, 0.0], 90.0, 0.6);
        assert!((feet[2] - 18.3).abs() < 1e-6 && (feet[0] - 10.0).abs() < 1e-6, "{feet:?}");
    }
}
