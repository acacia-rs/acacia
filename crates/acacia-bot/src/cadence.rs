//! When client ticks fire. Vanilla's ticks follow its frames: ~51 ms apart on average with
//! p10 31 / p50 47 / p90 64 ms gaps and an occasional two inputs in one batch
//! (vanilla mitm capture). Tick n never fires before n × 51 ms, so the bot is
//! never ahead of 20/s.
//!
//! `ACACIA_TICK_SCALE` (default 1) runs every tick-derived wall duration that many times faster, for a test
//! BDS patched to tick faster (testbox work/bds-pathfind4 `tps.sh`); per-tick behaviour stays the same.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::pin::Pin;
use std::sync::OnceLock;
use std::time::Duration;

use tokio::time::{Instant, Sleep};

/// `ACACIA_TICK_SCALE`, read once; anything but a finite positive number is 1.
pub(crate) fn tick_scale() -> f64 {
    static SCALE: OnceLock<f64> = OnceLock::new();
    *SCALE.get_or_init(|| {
        std::env::var("ACACIA_TICK_SCALE").ok().and_then(|v| v.parse().ok()).filter(|s: &f64| s.is_finite() && *s > 0.0).unwrap_or(1.0)
    })
}

/// `d` at [`tick_scale`]; exactly `d` at scale 1.
pub(crate) fn scaled(d: Duration) -> Duration {
    scaled_by(d, tick_scale())
}

/// Inverse of [`scaled`]: a wall duration in vanilla-tick time.
pub(crate) fn unscaled(d: Duration) -> Duration {
    scaled_by(d, 1.0 / tick_scale())
}

fn scaled_by(d: Duration, scale: f64) -> Duration {
    if scale == 1.0 {
        d
    } else {
        d.div_f64(scale)
    }
}

const PERIOD: Duration = Duration::from_millis(51);
/// Each tick fires up to this long after its slot; gaps then spread triangularly over ±JITTER.
const JITTER_MS: u64 = 36;
/// Lateness the schedule absorbs: a few timer wake-ups, well under a frame hitch.
const SLACK: Duration = Duration::from_millis(100);
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
    scale: f64,
}

impl Cadence {
    #[cfg(test)]
    pub fn new(seed: u64) -> Self {
        Self::with_scale(seed, 1.0)
    }

    pub fn with_scale(seed: u64, scale: f64) -> Self {
        Self { slot: Duration::ZERO, rng: seed, scale }
    }

    /// The next firing after `now`. A caller late by more than [`SLACK`] slides the schedule
    /// rather than catching up; one late by less keeps it, or a coarse timer (Windows wakes
    /// sleepers up to 16 ms late) would stretch every period and run the client at 16 ticks a
    /// second (measured 2026-10-11).
    pub fn next(&mut self, now: Duration) -> Fire {
        let period = scaled_by(PERIOD, self.scale);
        let due = self.slot + period;
        self.slot = if now > due + scaled_by(SLACK, self.scale) { now } else { due };
        let mut ticks = 1;
        if self.random().is_multiple_of(BURST_ONE_IN) {
            self.slot += period;
            ticks = 2;
        }
        let jitter = Duration::from_millis(self.random() % (JITTER_MS + 1));
        Fire { at: self.slot + scaled_by(jitter, self.scale), ticks }
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
    /// `ACACIA_TICK_LAG_MS`: each wake-up comes up to this late, to try a coarse or busy
    /// machine's timing where the timer is exact.
    lag: Option<u64>,
    lag_rng: u64,
    /// One timer for the bot's life: [`Self::wait`] runs in a `select!` per packet, and a fresh sleep each
    /// time registered and cleared a timer-wheel entry.
    sleep: Pin<Box<Sleep>>,
}

impl Ticker {
    pub fn new() -> Self {
        let mut cadence = Cadence::with_scale(seed(), tick_scale());
        let next = cadence.next(Duration::ZERO);
        let lag = std::env::var("ACACIA_TICK_LAG_MS").ok().and_then(|ms| ms.parse().ok());
        let start = Instant::now();
        Self { start, cadence, next, lag, lag_rng: seed(), sleep: Box::pin(tokio::time::sleep_until(start + next.at)) }
    }

    /// Waits for the next firing and returns how many ticks to run. Cancel-safe.
    pub async fn wait(&mut self) -> u32 {
        let at = self.start + self.next.at;
        if self.sleep.deadline() != at {
            self.sleep.as_mut().reset(at);
        }
        self.sleep.as_mut().await;
        if let Some(lag) = self.lag {
            // One wake-up in fifty hangs for twenty times the usual lateness.
            let late = splitmix(&mut self.lag_rng) % (lag + 1) * if splitmix(&mut self.lag_rng).is_multiple_of(50) { 20 } else { 1 };
            tokio::time::sleep(Duration::from_millis(late)).await;
        }
        let ticks = self.next.ticks;
        self.next = self.cadence.next(self.start.elapsed());
        ticks
    }

    /// Whether the next firing's time has come ([`Self::wait`] would not sleep).
    pub fn due(&self) -> bool {
        Instant::now() >= self.start + self.next.at
    }

    /// A frame hitch: the next firing comes `pause` later and carries two ticks, as vanilla's do.
    pub fn stall(&mut self, pause: Duration) {
        self.next.at += scaled(pause);
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
    fn a_coarse_timer_does_not_slow_the_ticks() {
        // Every wake-up comes 16 ms after its firing was due.
        let mut cadence = Cadence::new(3);
        let (mut now, mut ticks) = (Duration::ZERO, 0);
        for _ in 0..2_000 {
            let fire = cadence.next(now);
            now = fire.at.max(now) + Duration::from_millis(16);
            ticks += fire.ticks;
        }
        let mean = now.as_millis() as f64 / f64::from(ticks);
        assert!((50.5..52.5).contains(&mean), "mean {mean}");
    }

    #[test]
    fn a_tick_scale_compresses_the_same_schedule() {
        let (mut one, mut two) = (Cadence::new(9), Cadence::with_scale(9, 2.0));
        let (mut a, mut b) = (Fire { at: Duration::ZERO, ticks: 0 }, Fire { at: Duration::ZERO, ticks: 0 });
        for _ in 0..1_000 {
            (a, b) = (one.next(a.at), two.next(b.at));
            assert_eq!(a.ticks, b.ticks);
            assert!(a.at.abs_diff(b.at * 2) <= Duration::from_micros(1), "{a:?} vs {b:?}");
        }
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
