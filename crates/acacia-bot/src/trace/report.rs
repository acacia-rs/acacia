//! What a [`super::replay`] found: ticks off the recording and server corrections compared with ours.

use acacia_physics::Vec3;

/// A tick whose replayed output differs from the recorded `PlayerAuthInput`.
#[derive(Debug, Clone)]
pub struct TickDiff {
    pub tick: u64,
    pub mark: Option<String>,
    /// Eye positions (a driven vehicle's own position on its ticks).
    pub ours: Vec3,
    pub recorded: Vec3,
    pub delta_err: f32,
    /// Blocks around the recorded position, as in [`CorrectionDiff::blocks`].
    pub blocks: String,
    /// Within a couple of ticks of a teleport or knockback arriving or taking effect.
    pub near_event: bool,
}

/// A server correction compared with the replayed position at the tick it names.
#[derive(Debug, Clone)]
pub struct CorrectionDiff {
    pub tick: u64,
    pub mark: Option<String>,
    /// Eye positions (a driven vehicle's own position when `vehicle`); `ours` is `None` when the tick was not
    /// replayed.
    pub ours: Option<Vec3>,
    pub server: Vec3,
    /// Distance between our velocity and the server's at that tick.
    pub delta_err: Option<f32>,
    /// Our velocity and the server's at that tick.
    pub deltas: (Option<Vec3>, Vec3),
    /// The eye position and velocity the recorded client sent for that tick (a real client's own physics).
    pub sent: Option<(Vec3, Vec3)>,
    /// Explained by what the client had not been sent yet rather than a disagreement: a correction to a
    /// teleport's target, or one for a tick the server moved us on before we heard of it, froze us by an
    /// amount it reported later, or simulated in a chunk we had not received; for a vehicle, its state as
    /// boarded or a boat's height alone.
    pub explained: bool,
    /// Non-air blocks around the server's position (both layers), for triage.
    pub blocks: String,
    /// A correction of the driven vehicle, not the player.
    pub vehicle: bool,
}

impl CorrectionDiff {
    pub fn error(&self) -> Option<f32> {
        self.ours.map(|o| dist(o, self.server))
    }

    /// A disagreement in position or velocity beyond `tolerance` (a correction for a tick we did not
    /// simulate counts).
    pub fn mismatch(&self, tolerance: f32) -> bool {
        !self.explained && (self.error().is_none_or(|e| e > tolerance) || self.delta_err.is_some_and(|e| e > tolerance))
    }
}

/// Server corrections within this of our position are periodic resyncs, not mismatches.
pub const CORRECTION_TOLERANCE: f32 = 0.001;
/// Farther than any tick of movement: BDS applied a `/tp` it has not sent us yet (it can lag many ticks).
const TELEPORT_LAG_DISTANCE: f32 = 4.0;

#[derive(Debug, Default)]
pub struct Report {
    pub ticks: usize,
    /// Ticks off the recording by more than the tolerance.
    pub diverged: Vec<TickDiff>,
    pub corrections: Vec<CorrectionDiff>,
}

impl Report {
    /// Corrections we disagree with beyond `tolerance`, and those skipped as teleport lags.
    pub fn correction_mismatches(&self, tolerance: f32) -> (Vec<&CorrectionDiff>, Vec<&CorrectionDiff>) {
        let (lagged, mismatches) = self
            .corrections
            .iter()
            .filter(|c| c.mismatch(tolerance))
            .partition(|c| c.error().is_some_and(|e| e > TELEPORT_LAG_DISTANCE));
        (mismatches, lagged)
    }
}

pub(super) fn dist(a: Vec3, b: Vec3) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
