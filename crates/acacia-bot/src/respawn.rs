//! Death and respawn: the vanilla handshake is server `Respawn(searching)` on death, client
//! `Respawn(client ready)` + `PlayerAction(Respawn)` (acacia-session `Session::respawn`), then server
//! `Respawn(ready)` with the position.

use acacia_client::proto::packets::Respawn;
use acacia_client::proto::{Packet, RawPacket};

use crate::Bot;

const SEARCHING_FOR_SPAWN: u8 = 0;
/// A player reads the death screen before pressing "respawn" (vanilla capture: ~2 s).
const DEATH_SCREEN_TICKS: u32 = 40;

impl Bot {
    /// Presses "respawn" on the death screen. Movement resumes at the position the server replies with.
    pub fn respawn(&self) {
        tracing::debug!("respawning");
        self.client.respawn();
    }

    /// Schedules one respawn per death. BDS shows the death screen with `Respawn(searching)`; Geyser
    /// never sends it (the client shows it at zero health), so dying is a trigger too, and on BDS both
    /// arrive for the same death. BDS also sends "searching" while a live player joins, which vanilla
    /// doesn't answer. `was_alive`: before `packet` was applied.
    pub(crate) fn auto_respawn(&mut self, packet: &RawPacket, was_alive: bool) {
        if self.state.player.alive {
            self.respawn_in = None;
            return;
        }
        let searching = packet.id == Respawn::ID && packet.decode::<Respawn>().is_ok_and(|r| r.state == SEARCHING_FOR_SPAWN);
        if self.auto_respawn && self.respawn_in.is_none() && (searching || was_alive) {
            self.respawn_in = Some(DEATH_SCREEN_TICKS);
        }
    }

    /// Counts down a scheduled respawn; called every tick.
    pub(crate) fn tick_respawn(&mut self) {
        match self.respawn_in {
            Some(0) => {
                self.respawn_in = None;
                self.respawn();
            }
            Some(n) => self.respawn_in = Some(n - 1),
            None => {}
        }
    }
}
