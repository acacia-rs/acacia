//! Elytra state the bot reads and sets on the simulation; the glide physics is acacia-physics'.

use super::Movement;

impl Movement {
    /// The simulation is gliding.
    pub fn gliding(&self) -> bool {
        self.physics.as_ref().is_some_and(|st| st.gliding)
    }

    /// Predicts a firework boost from the use tick, as vanilla does, until the server's
    /// `MovementEffect` gives the rocket's real length.
    pub(crate) fn predict_glide_boost(&mut self) {
        if let Some(st) = self.physics.as_mut().filter(|st| st.gliding) {
            st.glide_boost_ticks = PREDICTED_BOOST_TICKS;
        }
    }

    /// The server's GLIDE_BOOST, stamped with input tick `tick`. The boost lasts the rocket's random
    /// lifetime, `10 * (flight + 1) + rand(6) + rand(7)` ticks, so only the packet knows it; `duration`
    /// is twice that (BDS log, 2026-10-03: duration 44 boosted 22 inputs; captures 42-60 for flight 1).
    pub(super) fn server_glide_boost(&mut self, duration: u32, tick: u64) {
        let elapsed = self.tick.saturating_sub(tick);
        if let Some(st) = self.physics.as_mut() {
            st.glide_boost_ticks = (i64::from(duration / 2) - elapsed as i64).max(0);
        }
    }
}

/// The client's own boost prediction before the server's effect arrives.
const PREDICTED_BOOST_TICKS: i64 = 20;
