//! Where a vehicle seats the local player (`positionRider`): the vehicle's first passenger attachment
//! (`getPassengerAttachmentPoint`; a camel's rises and sinks with its sit and stand animations) less the player's own
//! vehicle attachment, which scales with the player. Also the driven mount's size as its pose changes.

use acacia_java_physics::{Mount, seat_feet};
use acacia_java_proto::entity_types::ENTITY_SHAPES;

use crate::entity::Entity;
use crate::entity_size::POSE_STANDING;
use crate::join::Client;
use crate::mount_data::short_name;

const SIT_DOWN_TICKS: i64 = 40;
const STAND_UP_TICKS: i64 = 52;

/// The first passenger's attachment point, unturned: the type's, scaled with the entity, or a lone camel rider's
/// body anchor half a block (times the camel's scale) forward.
pub(crate) fn first_seat(e: &Entity, game_time: i64) -> Option<[f64; 3]> {
    let scale = e.scale();
    if matches!(short_name(e.kind), "camel" | "camel_husk") {
        let (_, height, _) = e.dimensions()?;
        let age = if e.baby { 0.6 } else { 1.0 };
        let y = camel_anchor(height, e.baby, scale * age, e.mount.last_pose_change, game_time) as f32;
        return Some([0.0, f64::from(y), f64::from(0.5 * (scale * age))]);
    }
    let seat = ENTITY_SHAPES.get(usize::try_from(e.kind).ok()?)?.seat;
    Some(seat.map(|v| if scale == 1.0 { f64::from(v) } else { f64::from(v) * f64::from(scale) }))
}

/// `Camel.getBodyAnchorAnimationYOffset` for the front seat: the standing anchor, moved along the sit-down or
/// stand-up curve while the pose changes, and down to the bottom once sat.
fn camel_anchor(height: f32, baby: bool, scale: f32, last_pose_change: i64, game_time: i64) -> f64 {
    let mut offset = f64::from(height) - if baby { 0.09375 } else { 0.375 };
    let sitting_drop = scale * 1.43;
    let vertical_drop = sitting_drop - scale * 0.2;
    let bottom = sitting_drop - vertical_drop;
    let sitting = last_pose_change < 0;
    let pose_time = game_time - last_pose_change.abs();
    let duration = if sitting { SIT_DOWN_TICKS } else { STAND_UP_TICKS };
    let changing = pose_time < duration;
    if changing {
        let (half, flex_offset) = if sitting { (28.0, 0.5) } else { (24.0, 0.6) };
        let duration = duration as f32;
        let t = (pose_time as f32).clamp(0.0, duration);
        let first = t < half;
        let part = if first { t / half } else { (t - half) / (duration - half) };
        let flex = sitting_drop - flex_offset * vertical_drop;
        let (from, to) = match (sitting, first) {
            (true, true) => (sitting_drop, flex),
            (true, false) => (flex, bottom),
            (false, true) => (bottom - sitting_drop, bottom - flex),
            (false, false) => (bottom - flex, 0.0),
        };
        offset += f64::from(from + part * (to - from));
    }
    if sitting && !changing {
        offset += f64::from(bottom);
    }
    offset
}

/// Before the driven mount's tick: its size from its pose and scale (`refreshDimensions`), and the size a rider's
/// stand-up switches it to.
pub(crate) fn resize_mount(mount: &mut Mount, e: &Entity) {
    if let (Some((w, h, _)), Some((sw, sh, _))) = (e.dimensions(), e.dimensions_in(POSE_STANDING)) {
        mount.set_size((w, h));
        mount.standing_size = (sw, sh);
    }
}

/// After the driven mount's tick: a stand-up it made (`Camel.standUp`, `standUpInstantly`) shows on the entity, and
/// the seat follows the pose.
pub(crate) fn reseat_mount(mount: &mut Mount, e: &mut Entity) {
    if mount.last_pose_change != e.mount.last_pose_change {
        e.mount.last_pose_change = mount.last_pose_change;
        if mount.last_pose_change >= 0 {
            e.pose = POSE_STANDING;
        }
    }
    if let Some(seat) = first_seat(e, mount.game_time) {
        mount.seat = seat;
    }
}

impl Client {
    /// Where the player sits on a vehicle the client doesn't drive.
    // TODO: only the first seat; a second rider (camel, boat) sits elsewhere, and minecarts' seat moves on rails.
    pub(crate) fn passenger_feet(&self, vehicle: i32) -> Option<[f64; 3]> {
        let e = self.world.entities.get(&vehicle)?;
        let attachment = self.movement.player.vehicle_attachment();
        Some(seat_feet(e.position, first_seat(e, self.world.game_time)?, e.y_rot, attachment))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEIGHT: f32 = 2.375;

    #[test]
    fn a_standing_camel_seats_at_its_anchor() {
        assert_eq!(camel_anchor(HEIGHT, false, 1.0, 100, 1000), f64::from(HEIGHT) - 0.375);
    }

    #[test]
    fn a_sitting_camel_lowers_the_seat_to_its_bottom() {
        let sitting = HEIGHT - 1.43;
        let sat = camel_anchor(sitting, false, 1.0, -100, 1000);
        assert_eq!(sat, f64::from(sitting) - 0.375 + f64::from(1.43_f32 - (1.43_f32 - 0.2)));
        // Just sat down: the anchor starts a full sitting drop up and passes the flex point at tick 28.
        let start = camel_anchor(sitting, false, 1.0, -1000, 1000);
        assert!((start - (f64::from(HEIGHT) - 0.375)).abs() < 1.0E-6, "{start}");
        let flex = camel_anchor(sitting, false, 1.0, -1000, 1028);
        assert!(start > flex && flex > sat, "{start} {flex} {sat}");
    }

    #[test]
    fn standing_up_climbs_back_to_the_standing_anchor() {
        let rising = camel_anchor(HEIGHT, false, 1.0, 1000, 1000);
        let low = f64::from(HEIGHT) - 0.375 + f64::from(0.2_f32 - 1.43);
        assert!((rising - low).abs() < 1.0E-6, "{rising} vs {low}");
        assert_eq!(camel_anchor(HEIGHT, false, 1.0, 1000, 1052), f64::from(HEIGHT) - 0.375);
    }
}
