//! Where a vehicle seats the local player (`positionRider`): the vehicle's passenger attachment for the player's place
//! among its riders (`getPassengerAttachmentPoint`; a camel's rises and sinks with its sit and stand animations, a
//! horse's moves back as it rears, a strider's bobs as it walks) less the player's own vehicle attachment, which
//! scales with the player. Also the driven mount's size as its pose changes.

use acacia_java_physics::{Mount, MountKind, feet_at, ride_height};
use acacia_java_proto::entity_types::ENTITY_SHAPES;

use crate::entity::Entity;
use crate::entity_size::POSE_STANDING;
use crate::join::Client;
use crate::mount_data::short_name;
use crate::riding::is_boat;

const SIT_DOWN_TICKS: i64 = 40;
const STAND_UP_TICKS: i64 = 52;
/// `EntityTypes.HAPPY_GHAST`'s passenger attachments (doubles: `Vec3`, not floats).
const HAPPY_GHAST_SEATS: [[f64; 3]; 4] = [[0.0, 4.0, 1.7], [-1.7, 4.0, 0.0], [0.0, 4.0, -1.7], [1.7, 4.0, 0.0]];

/// A rider's index among the vehicle's riders the client knows, and how many there are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Place {
    pub index: usize,
    pub count: usize,
}

impl Place {
    pub(crate) const DRIVER: Place = Place { index: 0, count: 1 };
}

fn is_camel(name: &str) -> bool {
    matches!(name, "camel" | "camel_husk")
}

/// The seats whose animation moves them: `AbstractHorse`'s rear (not llamas' or camels') and the strider's bob.
fn anim_kind(name: &str) -> Option<MountKind> {
    match name {
        "horse" | "donkey" | "mule" | "zombie_horse" => Some(MountKind::Horse),
        "skeleton_horse" => Some(MountKind::SkeletonHorse),
        "strider" => Some(MountKind::Strider),
        _ => None,
    }
}

/// `getScale() * getAgeScale()`, as `LivingEntity.getPassengerRidingPosition` passes it.
fn seat_scale(e: &Entity) -> f32 {
    let age = if !e.baby { 1.0 } else if is_camel(short_name(e.kind)) { 0.6 } else { 0.5 };
    e.scale() * age
}

/// `AbstractBoat.getPassengerAttachmentPoint`'s forward offset (players aren't `Animal`s).
pub(crate) fn boat_seat_z(name: &str, place: Place) -> f32 {
    match place {
        Place { count: 2.., index: 0 } => 0.2,
        Place { count: 2.., .. } => -0.6,
        _ if name.ends_with("_chest_boat") || name.ends_with("_chest_raft") => 0.15,
        _ => 0.0,
    }
}

/// The passenger attachment point for `place`, unturned and before animations.
pub(crate) fn seat(e: &Entity, place: Place, game_time: i64) -> Option<[f64; 3]> {
    let name = short_name(e.kind);
    let scale = e.scale();
    if is_camel(name) {
        let (_, height, _) = e.dimensions()?;
        let front = place.index == 0;
        let scale = seat_scale(e);
        let y = camel_anchor(height, e.baby, scale, front, e.mount.last_pose_change, game_time) as f32;
        let offset = if place.count > 1 && !front { -0.7 } else { 0.5 };
        return Some([0.0, f64::from(y), f64::from(offset * scale)]);
    }
    if is_boat(name) {
        return Some([0.0, ride_height(name.ends_with("_raft")), f64::from(boat_seat_z(name, place))]);
    }
    let seat = if name == "happy_ghast" {
        HAPPY_GHAST_SEATS[place.index.min(HAPPY_GHAST_SEATS.len() - 1)]
    } else {
        ENTITY_SHAPES.get(usize::try_from(e.kind).ok()?)?.seat
    };
    Some(seat.map(|v| if scale == 1.0 { v } else { v * f64::from(scale) }))
}

/// `Camel.getBodyAnchorAnimationYOffset`: the standing anchor, moved along the sit-down or stand-up curve while the
/// pose changes (the back seat's curve differs), and down to the bottom once sat.
fn camel_anchor(height: f32, baby: bool, scale: f32, front: bool, last_pose_change: i64, game_time: i64) -> f64 {
    let mut offset = f64::from(height) - if baby { 0.09375 } else { 0.375 };
    let sitting_drop = scale * 1.43;
    let vertical_drop = sitting_drop - scale * 0.2;
    let bottom = sitting_drop - vertical_drop;
    let sitting = last_pose_change < 0;
    let pose_time = game_time - last_pose_change.abs();
    let duration = if sitting { SIT_DOWN_TICKS } else { STAND_UP_TICKS };
    let changing = pose_time < duration;
    if changing {
        let (half, flex_offset) = match (sitting, front) {
            (true, true) => (28.0, 0.5),
            (true, false) => (28.0, 0.1),
            (false, true) => (24.0, 0.6),
            (false, false) => (32.0, 0.35),
        };
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

impl Entity {
    /// The client's own tick of what moves a seat: a horse's rear from its synced flag, a strider's walk from its
    /// interpolated move since `from`.
    pub(crate) fn tick_seat_anim(&mut self, from: [f64; 3]) {
        match anim_kind(short_name(self.kind)) {
            Some(MountKind::Strider) => self.mount.anim.tick_walk(from, self.position),
            Some(_) => self.mount.anim.tick_stand(self.mount.standing),
            None => {}
        }
    }
}

/// Before the driven mount's tick: its size from its pose and scale (`refreshDimensions`), the size a rider's
/// stand-up switches it to, and the scale its seat moves by.
pub(crate) fn resize_mount(mount: &mut Mount, e: &Entity) {
    if let (Some((w, h, _)), Some((sw, sh, _))) = (e.dimensions(), e.dimensions_in(POSE_STANDING)) {
        mount.set_size((w, h));
        mount.standing_size = (sw, sh);
    }
    mount.seat_scale = seat_scale(e);
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
    if let Some(seat) = seat(e, Place::DRIVER, mount.game_time) {
        mount.seat = seat;
    }
}

impl Client {
    /// The local player's place among `vehicle`'s riders.
    pub(crate) fn place_on(&self, vehicle: i32) -> Place {
        let me = self.world.player_id;
        let Some(e) = self.world.entities.get(&vehicle) else { return Place::DRIVER };
        let known: Vec<i32> = e.passengers.iter().copied().filter(|p| *p == me || self.world.entities.contains_key(p)).collect();
        Place { index: known.iter().position(|&p| p == me).unwrap_or(0), count: known.len().max(1) }
    }

    /// Where the player sits on a vehicle the client doesn't drive.
    pub(crate) fn passenger_feet(&self, vehicle: i32) -> Option<[f64; 3]> {
        let e = self.world.entities.get(&vehicle)?;
        let attachment = self.movement.player.vehicle_attachment();
        let unturned = seat(e, self.place_on(vehicle), self.world.game_time)?;
        let point = e.mount.anim.point(anim_kind(short_name(e.kind)), unturned, seat_scale(e), e.y_rot);
        Some(feet_at(e.position, point, attachment))
    }
}

#[cfg(test)]
#[path = "seat_tests.rs"]
mod tests;
