//! The client's side of a dimension change (docs/DESIGN.md "Dimension travel").

use acacia_client::proto::packets::{ChangeDimension, PlayerAction, ServerboundLoadingScreen, ShowCredits};
use acacia_client::proto::types::Action;
use acacia_client::proto::{Packet, RawPacket};
use acacia_world::ChunkView;

use crate::spawn::{LOADING_SCREEN_END, LOADING_SCREEN_START};
use crate::Bot;

pub(crate) const PACKETS: &[u32] = &[ChangeDimension::ID, PlayerAction::ID, ShowCredits::ID];

/// `ShowCredits` statuses.
const CREDITS_START: i32 = 0;
const CREDITS_END: i32 = 1;

/// Ticks after which the bot arrives whatever is missing: a server that sends no `DimensionChangeAck`, or
/// terrain that never comes (10 s).
const ARRIVAL_WAIT_TICKS: u32 = 200;
/// Ticks from leaving the loading screen to the first input: BDS ends the screen on its next tick and
/// drops the inputs that come before, so an early start leaves it one tick behind (one correction).
const SETTLE_TICKS: u32 = 10;

/// A dimension change the bot has not arrived from yet.
#[derive(Debug, Default)]
pub(crate) struct Travel {
    /// `ChangeDimension::loading_screen_id`, echoed in both loading screen packets.
    loading_screen_id: Option<u32>,
    /// The server's own `PlayerAction(DimensionChangeAck)` came: its side of the change is done.
    server_ready: bool,
    waited: u32,
    /// Ticks since the loading screen was left.
    settled: Option<u32>,
    /// `ChangeDimension::respawn`: the way out of the End.
    respawn: bool,
    /// `PlayerState::teleports` when the change came.
    teleports: u32,
}

#[derive(Debug, PartialEq)]
enum Step {
    Wait,
    /// Acknowledge the change and leave the loading screen.
    Leave,
    Arrived,
}

impl Travel {
    /// A loading screen packet, if the server announced a screen: BDS kicks (UnexpectedPacket) for one it
    /// does not expect, and the change after a death in another dimension carries no id.
    fn loading_screen(&self, r#type: i32) -> Option<ServerboundLoadingScreen> {
        self.loading_screen_id.map(|id| ServerboundLoadingScreen { r#type, loading_screen_id: Some(id) })
    }

    /// One client tick. `placed`: the terrain at the player's position has arrived. `moved`: the server
    /// has sent a position since the change.
    fn tick(&mut self, placed: bool, moved: bool) -> Step {
        self.waited += 1;
        // After the credits the change names no position (y 32767): it follows the acknowledgement.
        let placed = placed && (moved || !self.respawn);
        match &mut self.settled {
            None if self.server_ready && (placed || self.respawn) || self.waited >= ARRIVAL_WAIT_TICKS => {
                self.settled = Some(0);
                Step::Leave
            }
            None => Step::Wait,
            Some(ticks) => {
                *ticks += 1;
                if *ticks > SETTLE_TICKS && (placed || *ticks > ARRIVAL_WAIT_TICKS) { Step::Arrived } else { Step::Wait }
            }
        }
    }
}

/// Whether the sections around a player standing at `feet` have arrived, from the block below to the head:
/// BDS names the real position in `ChangeDimension`, so unlike at spawn there is no ground to wait for.
pub(crate) fn terrain_arrived(view: &ChunkView, [x, y, z]: [f32; 3]) -> bool {
    let column_arrived = |x: f32, z: f32| {
        let Some(chunk) = view.chunk((x.floor() as i32) >> 4, (z.floor() as i32) >> 4) else { return false };
        let chunk = chunk.read();
        let dim = chunk.dimension();
        let sections = (dim.height >> 4) as i32;
        let section = |y: f32| (y.floor() as i32 - dim.min_y) >> 4;
        // Outside the dimension's height there is nothing to arrive.
        (section(y - 1.0)..=section(y + 2.0)).filter(|s| (0..sections).contains(s)).all(|s| chunk.section_known(s as usize))
    };
    [(-0.3, -0.3), (0.3, -0.3), (-0.3, 0.3), (0.3, 0.3)].iter().all(|&(dx, dz)| column_arrived(x + dx, z + dz))
}

impl Bot {
    /// Opens the loading screen on `ChangeDimension` and notes the server's acknowledgement.
    pub(crate) fn on_travel_packet(&mut self, packet: &RawPacket) {
        match packet.id {
            ChangeDimension::ID => {
                let Ok(change) = packet.decode::<ChangeDimension>() else { return };
                tracing::debug!(?change, "dimension change");
                let teleports = self.state.player.teleports;
                let travel = Travel { loading_screen_id: change.loading_screen_id, respawn: change.respawn, teleports, ..Travel::default() };
                if let Some(screen) = travel.loading_screen(LOADING_SCREEN_START) {
                    self.client.send(&screen);
                }
                self.travel = Some(travel);
            }
            PlayerAction::ID => {
                if let (Some(travel), Ok(action)) = (&mut self.travel, packet.decode::<PlayerAction>())
                    && action.action == Action::DimensionChangeAck
                {
                    tracing::debug!(waited = travel.waited, "server dimension change ack");
                    travel.server_ready = true;
                }
            }
            // Leaving the End: BDS brings the player back only once the client reports the credits over.
            ShowCredits::ID => {
                let me = self.state.player.runtime_entity_id;
                if packet.decode::<ShowCredits>().is_ok_and(|c| c.runtime_entity_id == me && c.status == CREDITS_START) {
                    tracing::debug!("credits skipped");
                    self.client.send(&ShowCredits { runtime_entity_id: me, status: CREDITS_END });
                }
            }
            _ => {}
        }
    }

    /// One tick of a pending dimension change. `placed`: the terrain at the new position has arrived.
    /// True when movement may run: the change is over, or (with none pending) `placed`.
    pub(crate) fn travel_tick(&mut self, placed: bool) -> bool {
        let Some(travel) = &mut self.travel else { return placed };
        match travel.tick(placed, self.state.player.teleports != travel.teleports) {
            Step::Wait => {}
            Step::Leave => {
                tracing::debug!(?travel, "leaving the dimension loading screen");
                let me = self.state.player.runtime_entity_id;
                self.client.send(&crate::sleep::player_action(me, Action::DimensionChangeAck));
                if let Some(screen) = travel.loading_screen(LOADING_SCREEN_END) {
                    self.client.send(&screen);
                }
            }
            Step::Arrived => {
                self.travel = None;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps(travel: &mut Travel, ticks: u32, placed: bool) -> Vec<Step> {
        (0..ticks).map(|_| travel.tick(placed, false)).collect()
    }

    #[test]
    fn leaves_the_screen_once_both_sides_are_ready_then_settles() {
        let mut travel = Travel { loading_screen_id: Some(7), ..Travel::default() };
        assert!(steps(&mut travel, 5, true).iter().all(|s| *s == Step::Wait), "no server acknowledgement yet");
        travel.server_ready = true;
        assert!(steps(&mut travel, 5, false).iter().all(|s| *s == Step::Wait), "no terrain yet");
        assert_eq!(travel.tick(true, false), Step::Leave);
        assert!(steps(&mut travel, SETTLE_TICKS, true).iter().all(|s| *s == Step::Wait));
        assert_eq!(travel.tick(true, false), Step::Arrived);
    }

    #[test]
    fn out_of_the_end_waits_for_the_position_after_leaving_the_screen() {
        let mut travel = Travel { loading_screen_id: Some(7), respawn: true, server_ready: true, ..Travel::default() };
        assert_eq!(travel.tick(false, false), Step::Leave, "the placeholder position has no terrain to wait for");
        assert!(steps(&mut travel, SETTLE_TICKS + 20, true).iter().all(|s| *s == Step::Wait), "no position yet");
        assert_eq!(travel.tick(false, true), Step::Wait, "terrain at the real position");
        assert_eq!(travel.tick(true, true), Step::Arrived);
    }

    #[test]
    fn arrives_without_an_acknowledgement_after_the_wait() {
        let mut travel = Travel::default();
        let waited = steps(&mut travel, ARRIVAL_WAIT_TICKS, false);
        assert_eq!(waited.iter().filter(|s| **s == Step::Leave).count(), 1);
        assert_eq!(waited.last(), Some(&Step::Leave));
    }

    #[test]
    fn loading_screen_packets_only_for_an_announced_screen() {
        let announced = Travel { loading_screen_id: Some(0), ..Travel::default() };
        let screen = announced.loading_screen(LOADING_SCREEN_START).expect("id 0 is an id");
        assert_eq!((screen.r#type, screen.loading_screen_id), (LOADING_SCREEN_START, Some(0)));
        assert!(Travel::default().loading_screen(LOADING_SCREEN_END).is_none());
    }
}
