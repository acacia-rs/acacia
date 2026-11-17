//! Riptide launches the bot predicts; the spin itself is acacia-physics'.

use super::Movement;

/// BDS keeps a player's position at the eye, this far above the feet.
const EYE_OFFSET: f32 = 1.62001;

impl Movement {
    /// The one cell BDS `Actor::isInWaterOrRain` tests for rain: the eye position raised by the box height, so a
    /// cell above the head (docs/research/riptide-rain.md).
    pub(crate) fn rain_cell(&self) -> Option<[i32; 3]> {
        let st = self.physics.as_ref()?;
        let [x, y, z] = st.pos;
        Some([x.floor() as i32, (y + EYE_OFFSET + st.size[1]).floor() as i32, z.floor() as i32])
    }

    /// Launches on the next tick at Riptide `level` if the simulation touched water last tick or `in_rain`.
    pub(crate) fn predict_riptide(&mut self, level: u8, in_rain: bool) -> bool {
        let Some(st) = self.physics.as_mut().filter(|st| in_rain || st.touched_water()) else { return false };
        st.spin.launch = Some(level);
        true
    }
}
