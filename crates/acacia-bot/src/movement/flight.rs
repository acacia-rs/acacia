//! Flight abilities the bot sets on the simulation; the flight physics is acacia-physics'.

use acacia_client::proto::types::GameMode;
use acacia_physics::Flight;

use super::{Controls, Movement};
use crate::state::PlayerState;

/// The jump key, replaced while `c.fly` differs from the flight by the double tap that toggles it: release,
/// press, release, press (BDS `FlyTriggerIntentSystem` ignores the input's StartFlying).
pub(super) fn fly_jump(taps: &mut u8, c: &Controls, flight: &Flight) -> bool {
    if *taps == 0 && c.fly != flight.flying && (flight.flying || flight.may_fly) {
        *taps = 4;
    }
    if *taps == 0 {
        return c.jump;
    }
    *taps -= 1;
    *taps % 2 == 0
}

impl Movement {
    /// The simulation is flying.
    pub fn flying(&self) -> bool {
        self.physics.as_ref().is_some_and(|st| st.flight.flying)
    }

    /// Takes the server's abilities and game mode for the next tick. Its flying flag is followed when first seen
    /// and when it turns off: BDS reported our own flight start 7 s late (drill `flight`), so a late
    /// "flying" may predate our stop.
    pub(crate) fn set_abilities(&mut self, player: &PlayerState) {
        let Some(st) = self.physics.as_mut() else { return };
        let (abilities, flight) = (&player.abilities, &mut st.flight);
        flight.may_fly = player.may_fly();
        flight.fly_speed = abilities.fly_speed;
        flight.vertical_fly_speed = abilities.vertical_fly_speed;
        flight.creative = player.game_mode == GameMode::Creative;
        let last = self.server_flying.replace(abilities.flying);
        if last != Some(abilities.flying) && (last.is_none() || !abilities.flying) {
            (flight.flying, flight.travel) = (abilities.flying, abilities.flying);
            self.controls.fly = abilities.flying;
        }
        if !flight.may_fly {
            (flight.flying, flight.travel) = (false, false);
        }
    }
}
