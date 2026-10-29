use acacia_client::{Client, DisconnectReason, PacketTraceSummary};

use crate::movement::{Controls, Movement};
use crate::state::GameState;
use crate::trace;
use crate::world::WorldTracker;
use crate::Bot;

impl Bot {
    /// Why the connection ended, once it has.
    pub fn disconnect_reason(&self) -> Option<&DisconnectReason> {
        self.closed.as_ref()
    }

    /// Labels the recorded trace from here on (no-op when not recording).
    pub fn trace_mark(&mut self, label: &str) {
        if let Some(r) = &mut self.recorder {
            r.write(&trace::Event::Mark(label.to_owned()));
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Recent inbound packets (filtered ones too) and the server's silence, for stall diagnostics.
    pub fn packet_trace(&self) -> PacketTraceSummary {
        self.client.packet_trace()
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn world(&self) -> Option<&WorldTracker> {
        self.world.as_ref()
    }

    /// Movement intent (physics bots only); applied from the next tick on.
    pub fn controls(&mut self) -> Option<&mut Controls> {
        self.movement.as_mut().map(|m| &mut m.controls)
    }

    pub fn movement(&self) -> Option<&Movement> {
        self.movement.as_ref()
    }
}
