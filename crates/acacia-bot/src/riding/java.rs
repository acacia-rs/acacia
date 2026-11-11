//! Riding on Java: the client keeps the seat and drives the boat or mount (`acacia_java_client::Riding`); this mirrors the seat
//! into `state.riding`. A dismount holds shift until the server lets the player off.

use acacia_java_physics::{Boat, Mount};

use crate::state::{java_bedrock_kind, Vehicle};
use crate::Bot;

impl Bot {
    /// The boat the bot drives on Java, as simulated after its last tick.
    pub fn java_boat(&self) -> Option<&Boat> {
        self.java()?.client().movement.riding.as_ref()?.driven.as_ref()?.boat()
    }

    /// The mount the bot drives on Java, as simulated after its last tick.
    pub fn java_mount(&self) -> Option<&Mount> {
        self.java()?.client().movement.riding.as_ref()?.driven.as_ref()?.mount()
    }

    /// Mirrors the Java client's seat into `state.riding`; a dismount is over once the seat is gone.
    pub(crate) fn mirror_java_riding(&mut self) {
        let Some(client) = self.java().map(|j| j.client()) else { return };
        let vehicle = client.movement.riding.as_ref().map(|r| Vehicle {
            unique_id: i64::from(r.vehicle),
            runtime_id: u64::try_from(r.vehicle).ok(),
            kind: client.world.entities.get(&r.vehicle).map(|e| java_bedrock_kind(e.kind).to_owned()),
            driver: r.driver,
        });
        if vehicle.is_none() {
            self.ride.dismount = false;
        }
        self.state.riding.vehicle = vehicle;
    }

    /// The server lets a rider off when its input holds shift (`Player.rideTick`).
    pub(crate) fn java_dismounting(&self) -> bool {
        self.ride.dismount && self.state.riding.is_riding()
    }
}
