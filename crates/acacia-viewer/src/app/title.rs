//! The title-bar overlay: frame rate and memory figures, logged every few updates.

use std::time::{Duration, Instant};

use acacia_render::FrameStats;

use super::App;

const TITLE_EVERY: Duration = Duration::from_millis(500);
/// Title updates between stats lines in the log (5 s).
const LOG_EVERY_TITLES: u32 = 10;

/// Frame-rate and memory figures shown in the title.
pub(super) struct Overlay {
    since: Instant,
    frames: u32,
    reports: u32,
}

impl Overlay {
    pub(super) fn new() -> Overlay {
        Overlay { since: Instant::now(), frames: 0, reports: 0 }
    }
}

impl App {
    /// Counts a drawn frame; every `TITLE_EVERY` sets the title (and now and then logs it).
    pub(super) fn count_frame(&mut self, stats: &FrameStats, now: Instant) {
        self.overlay.frames += 1;
        let elapsed = self.overlay.since.elapsed();
        if elapsed < TITLE_EVERY {
            return;
        }
        self.fps = self.overlay.frames as f32 / elapsed.as_secs_f32();
        let line = crate::overlay::title(self.fps, stats, &self.status);
        if let Some(w) = &self.window {
            w.set_title(&line);
        }
        self.overlay.reports += 1;
        if self.overlay.reports.is_multiple_of(LOG_EVERY_TITLES) {
            tracing::info!("{line}");
        }
        self.overlay = Overlay { since: now, frames: 0, reports: self.overlay.reports };
    }
}
