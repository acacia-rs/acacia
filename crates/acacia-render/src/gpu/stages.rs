//! Where a slow frame's time went: each stage of `Renderer::render`, logged when the whole took long.

use std::time::{Duration, Instant};

const SLOW: Duration = Duration::from_millis(100);

pub(super) struct StageClock {
    start: Instant,
    last: Instant,
    stages: Vec<(&'static str, Duration)>,
}

impl StageClock {
    pub(super) fn start() -> StageClock {
        let now = Instant::now();
        StageClock { start: now, last: now, stages: Vec::with_capacity(10) }
    }

    pub(super) fn mark(&mut self, stage: &'static str) {
        let now = Instant::now();
        self.stages.push((stage, now - self.last));
        self.last = now;
    }

    pub(super) fn report(&self) {
        let total = self.last - self.start;
        if total >= SLOW {
            let stages: Vec<String> = self.stages.iter().map(|(s, d)| format!("{s}={:.1}ms", d.as_secs_f64() * 1e3)).collect();
            tracing::warn!(target: "frame", total_ms = total.as_millis() as u64, stages = %stages.join(" "), "slow render");
        }
    }
}
