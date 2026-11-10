//! Driving a mount: whether the player controls it (`getControllingPassenger`), the mount built from what the client
//! tracks, synced data handed to it as `onSyncedDataUpdated` does, and the rider's jump bar (`LocalPlayer.aiStep`).

use acacia_java_physics::{Boost, Mount, MountKind, seat_feet};
use acacia_java_proto::entity_types::ENTITY_SHAPES;
use acacia_registry::Item;

use crate::entity::Entity;
use crate::join::Client;
use crate::mount_data::{MountData, Synced, short_name};

/// The rider's jump charge: `jumpRidingTicks` and `jumpRidingScale`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JumpBar {
    ticks: i32,
    scale: f32,
}

impl JumpBar {
    /// One `aiStep` of the bar for a mount that takes jumps now; the charge out of 100 when the key is let go.
    fn step(&mut self, was_jumping: bool, jumping: bool) -> Option<i32> {
        if self.ticks < 0 {
            self.ticks += 1;
            if self.ticks == 0 {
                self.scale = 0.0;
            }
        }
        if was_jumping && !jumping {
            self.ticks = -10;
            return Some((self.scale * 100.0).floor() as i32);
        }
        if !was_jumping && jumping {
            (self.ticks, self.scale) = (0, 0.0);
        } else if was_jumping {
            self.ticks += 1;
            self.scale = if self.ticks < 10 { self.ticks as f32 * 0.1 } else { 0.8 + 2.0 / (self.ticks - 9) as f32 * 0.1 };
        }
        None
    }
}

/// Synced data the driven mount reacts to.
pub(crate) fn apply_synced(mount: &mut Mount, synced: Synced) {
    if let Some(standing) = synced.standing {
        mount.standing = standing;
    }
    if let Some(tick) = synced.last_pose_change {
        mount.last_pose_change = tick;
    }
    if let Some(dashing) = synced.dashing {
        if dashing && mount.dash_cooldown == 0 {
            mount.dash_cooldown = if mount.kind == MountKind::Camel { 55 } else { 40 };
        }
        mount.dashing = dashing;
    }
    if let Some(total) = synced.boost {
        mount.boost = Some(Boost { time: 0, total });
    }
}

/// Synced attributes, defaults where the server sent none.
pub(crate) fn apply_attributes(mount: &mut Mount, data: &MountData) {
    mount.movement_speed = data.movement_speed.unwrap_or(mount.movement_speed);
    mount.jump_strength = data.jump_strength.unwrap_or(mount.jump_strength);
    mount.flying_speed = data.flying_speed.unwrap_or(mount.flying_speed);
}

fn new_mount(e: &Entity, game_time: i64) -> Option<Mount> {
    let (width, height, _) = e.dimensions()?;
    let seat = ENTITY_SHAPES.get(usize::try_from(e.kind).ok()?)?.seat;
    let mut mount = Mount::new(short_name(e.kind), (width, height), seat, e.position, e.y_rot)?;
    (mount.body.pitch, mount.body.delta, mount.game_time) = (e.x_rot, e.motion, game_time);
    apply_attributes(&mut mount, &e.mount);
    let synced = Synced { standing: Some(e.mount.standing), dashing: None, last_pose_change: Some(e.mount.last_pose_change), boost: e.mount.boost };
    apply_synced(&mut mount, synced);
    mount.dashing = e.mount.dashing;
    Some(mount)
}

impl Client {
    fn holding(&self, item: Item) -> bool {
        let held = [self.inventory.held(), self.inventory.offhand()];
        held.into_iter().flatten().any(|s| u16::try_from(s.item).ok().and_then(Item::from_java_id) == Some(item))
    }

    /// Whether the player in the driver's seat controls `e`, so the client simulates it.
    fn controls(&self, e: &Entity) -> bool {
        match short_name(e.kind) {
            "pig" => e.mount.saddled && self.holding(Item::CarrotOnAStick),
            "strider" => e.mount.saddled && self.holding(Item::WarpedFungusOnAStick),
            "happy_ghast" => e.mount.harnessed && !e.stays_still,
            _ => e.mount.saddled,
        }
    }

    /// The mount the player drives now: kept while control lasts, built when it starts, dropped when it ends.
    pub(crate) fn controlled_mount(&self, vehicle: i32, kept: Option<Mount>) -> Option<Mount> {
        let e = self.world.entities.get(&vehicle).filter(|e| self.controls(e))?;
        kept.or_else(|| new_mount(e, self.world.game_time))
    }

    /// The rider's `aiStep` part for a mount: the jump bar, returning the charge to send on release.
    pub(crate) fn charge_jump(&mut self, was_jumping: bool, jumping: bool) -> Option<i32> {
        let riding = self.movement.riding.as_mut()?;
        let mount = riding.driven.as_mut().and_then(|d| d.mount_mut()).filter(|m| m.can_jump() && m.jump_cooldown() == 0);
        let Some(mount) = mount else {
            riding.jump_bar.scale = 0.0;
            return None;
        };
        let charge = riding.jump_bar.step(was_jumping, jumping)?;
        mount.on_player_jump(charge);
        Some(charge)
    }

    /// Where the player sits on a vehicle the client doesn't drive.
    // TODO: only the first seat; a second rider (camel, boat) sits elsewhere, and minecarts' seat moves on rails.
    pub(crate) fn passenger_feet(&self, vehicle: i32) -> Option<[f64; 3]> {
        let e = self.world.entities.get(&vehicle)?;
        let seat = ENTITY_SHAPES.get(usize::try_from(e.kind).ok()?)?.seat.map(f64::from);
        Some(seat_feet(e.position, seat, e.y_rot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_after(held: u32) -> Option<i32> {
        let mut bar = JumpBar::default();
        let mut was = false;
        for _ in 0..held {
            assert_eq!(bar.step(was, true), None);
            was = true;
        }
        bar.step(was, false)
    }

    #[test]
    fn the_jump_bar_peaks_on_its_tenth_tick() {
        assert_eq!(release_after(1), Some(0));
        assert_eq!(release_after(5), Some(40));
        assert_eq!(release_after(11), Some(100));
        assert_eq!(release_after(13), Some(86));
    }
}
