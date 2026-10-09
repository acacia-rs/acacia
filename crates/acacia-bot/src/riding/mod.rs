//! Riding: mounting by right-clicking an entity, dismounting, and the seated player's `PlayerAuthInput`.
//! The seat itself is tracked in `state::Riding`. Physics bots steer horses (`horse.rs`); other
//! vehicles are not simulated yet (docs/research/riding-fishing-elytra.md, with the vanilla packet sequences).

mod boat;
mod exit;
mod horse;
mod input;
mod keys;
mod tick;

use std::time::Duration;

use acacia_client::proto::packets::{ClientCameraAimAssist, ClientCameraAimAssistAction, Interact, InteractActionId};
use acacia_client::proto::types::Vec3f;
use acacia_client::proto::RawPacket;

use crate::movement::Idle;
use crate::state::Vehicle;
use crate::{ActionError, Bot};

/// How long the server has to answer a mount with `SetEntityLink`.
const LINK_TIMEOUT: Duration = Duration::from_secs(1);
/// Ticks a dismount may wait for an input to carry it (none go out while dead or loading).
const DISMOUNT_TICKS: u32 = 20;

/// Per-bot riding state for the tick loop.
#[derive(Default)]
pub(crate) struct Ride {
    /// Physics bots: stands in for movement while seated, continuing its tick count.
    idle: Option<Idle>,
    /// Seated inputs sent for the current vehicle; 0 means the next one is the first.
    seated: u32,
    /// Leave the seat on the next tick.
    dismount: bool,
    /// Feet position for a physics bot leaving the current seat (the server's exit spot when known).
    leaving_feet: Option<[f32; 3]>,
    /// Physics bots: the horse being driven.
    horse: Option<horse::HorseSim>,
    /// Physics bots: the boat being paddled.
    boat: Option<boat::BoatSim>,
    /// Added once to the next reported vehicle position ([`Bot::offset_vehicle_report`]).
    report_offset: Option<[f32; 3]>,
}

impl Bot {
    /// Gets on a tracked entity (needs `Trackers::entities`) by right-clicking it (`UseItemOnEntity`
    /// Interact, empty hand in the capture) and waits for the server's link. Pigs and striders need a
    /// saddle (the same click holding one); boats and minecarts don't. Servers refuse mounts while sneaking.
    pub async fn mount(&mut self, runtime_id: u64) -> Result<Vehicle, ActionError> {
        if let Some(v) = self.state.riding.vehicle.as_ref().filter(|v| v.runtime_id == Some(runtime_id)) {
            return Ok(v.clone());
        }
        if self.movement.as_ref().is_some_and(|m| m.controls.sneak) {
            return Err(ActionError::NotPossible("sneaking".into()));
        }
        self.interact_entity(runtime_id).await?;
        let seated = |bot: &Bot, _: &RawPacket| bot.state.riding.vehicle.clone().filter(|v| v.runtime_id == Some(runtime_id));
        match self.wait_until(LINK_TIMEOUT, seated).await {
            Err(ActionError::Timeout) => Err(ActionError::Rejected(format!("not seated on entity {runtime_id}"))),
            other => other,
        }
    }

    /// Leaves the vehicle like vanilla: on the next tick the client gives up its seat itself, sends
    /// `Interact` LeaveVehicle with the point it steps off at (that tick's input already stands there,
    /// no sneak flags) and clears aim assist. A server that disagrees links the bot again, which shows
    /// in [`Bot::vehicle`].
    pub async fn dismount(&mut self) -> Result<(), ActionError> {
        if !self.state.riding.is_riding() {
            return Err(ActionError::NotPossible("not riding".into()));
        }
        self.ride.dismount = true;
        for _ in 0..DISMOUNT_TICKS {
            self.next_tick(|_, _| false).await?;
            if !self.ride.dismount || !self.state.riding.is_riding() {
                self.ride.dismount = false;
                return Ok(());
            }
        }
        self.ride.dismount = false;
        Err(ActionError::Timeout)
    }

    /// Testing aid: reports the driven vehicle `offset` away from its simulated position on the next
    /// tick, so the server answers with a correction carrying its own vehicle state.
    #[doc(hidden)]
    pub fn offset_vehicle_report(&mut self, offset: [f32; 3]) {
        self.ride.report_offset = Some(offset);
    }

    /// The vehicle the bot sits on, from the server's links (also when the server seated it).
    pub fn vehicle(&self) -> Option<&Vehicle> {
        self.state.riding.vehicle.as_ref()
    }

    /// The seated player's eye: the seat (`RiderSeatPosition` turned by the vehicle's yaw) on the vehicle's
    /// latest pose. `None` when not riding or before the server sent the seat.
    pub fn seat_eye(&self) -> Option<[f32; 3]> {
        self.state.riding.seat_offset.as_ref()?;
        exit::Mount::of(&self.state).map(|m| m.seat())
    }

    /// `eye`: the dismount tick's input position, which vanilla repeats in LeaveVehicle.
    fn leave_vehicle(&mut self, eye: Vec3f) {
        let target = self.state.riding.vehicle.as_ref().and_then(|v| v.runtime_id).unwrap_or(0);
        self.client.send(&Interact {
            action_id: InteractActionId::LeaveVehicle,
            target_entity_id: target,
            has_position: true,
            position: Some(eye),
        });
        self.clear_aim_assist();
        self.state.riding.leave();
        self.ride.seated = 0;
    }

    /// `ClientCameraAimAssist` Clear, which vanilla sends on every mount and dismount.
    fn clear_aim_assist(&self) {
        self.client.send(&ClientCameraAimAssist {
            preset_id: String::new(),
            action: ClientCameraAimAssistAction::Clear,
            allow_aim_assist: false,
        });
    }
}
