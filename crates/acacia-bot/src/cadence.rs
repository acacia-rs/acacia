//! When client ticks fire. Vanilla's ticks follow its frames: ~51 ms apart on average with
//! p10 31 / p50 47 / p90 64 ms gaps and an occasional two inputs in one batch
//! (vanilla mitm capture). Tick n never fires before n × 51 ms, so the bot is
//! never ahead of 20/s.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::time::Duration;

use tokio::time::Instant;

const PERIOD: Duration = Duration::from_millis(51);
/// Each tick fires up to this long after its slot; gaps then spread triangularly over ±JITTER.
const JITTER_MS: u64 = 36;
/// One slot in this many is skipped and its tick sent with the next (vanilla: ~1 in 150).
const BURST_ONE_IN: u64 = 150;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fire {
    /// Since the cadence started.
    pub at: Duration,
    /// Ticks to run back to back.
    pub ticks: u32,
}

pub(crate) struct Cadence {
    slot: Duration,
    rng: u64,
}

impl Cadence {
    pub fn new(seed: u64) -> Self {
        Self { slot: Duration::ZERO, rng: seed }
    }

    /// The next firing after `now`; a late caller slides the schedule rather than catching up.
    pub fn next(&mut self, now: Duration) -> Fire {
        self.slot = (self.slot + PERIOD).max(now);
        let mut ticks = 1;
        if self.random().is_multiple_of(BURST_ONE_IN) {
            self.slot += PERIOD;
            ticks = 2;
        }
        Fire { at: self.slot + Duration::from_millis(self.random() % (JITTER_MS + 1)), ticks }
    }

    fn random(&mut self) -> u64 {
        splitmix(&mut self.rng)
    }
}

/// A per-process random seed (no rand dependency needed for timing noise).
pub(crate) fn seed() -> u64 {
    RandomState::new().hash_one(0u8)
}

/// splitmix64
pub(crate) fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A [`Cadence`] on the tokio clock, seeded per bot.
pub(crate) struct Ticker {
    start: Instant,
    cadence: Cadence,
    next: Fire,
}

impl Ticker {
    pub fn new() -> Self {
        let mut cadence = Cadence::new(seed());
        let next = cadence.next(Duration::ZERO);
        Self { start: Instant::now(), cadence, next }
    }

    /// Waits for the next firing and returns how many ticks to run. Cancel-safe.
    pub async fn wait(&mut self) -> u32 {
        tokio::time::sleep_until(self.start + self.next.at).await;
        let ticks = self.next.ticks;
        self.next = self.cadence.next(self.start.elapsed());
        ticks
    }

    /// A frame hitch: the next firing comes `pause` later and carries two ticks, as vanilla's do.
    pub fn stall(&mut self, pause: Duration) {
        self.next.at += pause;
        self.next.ticks = 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(n: usize) -> Vec<Fire> {
        let mut cadence = Cadence::new(7);
        let mut fires: Vec<Fire> = Vec::with_capacity(n);
        for _ in 0..n {
            let now = fires.last().map_or(Duration::ZERO, |f| f.at);
            fires.push(cadence.next(now));
        }
        fires
    }

    #[test]
    fn never_ahead_of_twenty_per_second() {
        let mut sent = 0u64;
        for fire in run(20_000) {
            sent += u64::from(fire.ticks);
            assert!(fire.at >= Duration::from_millis(50) * sent as u32, "{sent} ticks by {:?}", fire.at);
        }
    }

    #[test]
    fn gaps_look_like_vanilla() {
        let fires = run(20_000);
        let mut gaps: Vec<u128> = fires.windows(2).map(|w| (w[1].at - w[0].at).as_millis()).collect();
        let ticks: u32 = fires.iter().map(|f| f.ticks).sum();
        let mean = fires.last().unwrap().at.as_millis() as f64 / f64::from(ticks);
        assert!((50.5..52.0).contains(&mean), "mean {mean}");
        gaps.sort_unstable();
        let pct = |p: usize| gaps[gaps.len() * p / 100];
        assert!((25..=40).contains(&pct(10)) && (45..=56).contains(&pct(50)) && (60..=80).contains(&pct(90)));
        let bursts = fires.iter().filter(|f| f.ticks == 2).count();
        assert!((60..=220).contains(&bursts), "{bursts} bursts");
    }

    #[test]
    fn late_caller_does_not_catch_up() {
        let mut cadence = Cadence::new(1);
        cadence.next(Duration::ZERO);
        let late = Duration::from_secs(5);
        let fire = cadence.next(late);
        assert!(fire.at >= late && fire.ticks <= 2);
        assert!(cadence.next(fire.at).at >= fire.at + Duration::from_millis(15));
    }
}
