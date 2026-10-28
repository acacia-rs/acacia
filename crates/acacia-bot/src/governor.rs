//! One place every module asks before sending a world action (break, place, use): an echo round-trip
//! estimate that scales confirmation timeouts, a cap on actions awaiting their echo, a minimum gap
//! between actions, and a hold after a server setback. Network-free; the bot ticks it.
//! Design: acacia-pathfind/docs/confirmed-actions.md.

use std::time::Duration;

use crate::cadence::{scaled, unscaled};

/// These wall durations are all per-tick quantities: read them through [`scaled`] (see `cadence`).
const TICK: Duration = Duration::from_millis(50);
/// Before any echo was measured.
const DEFAULT_RTT: Duration = Duration::from_millis(150);
/// Confirmation timeout bounds: below the floor a busy server tick reads as a lost action.
const MIN_TIMEOUT: Duration = Duration::from_millis(500);
const MAX_TIMEOUT: Duration = Duration::from_secs(5);
/// Ticks without a new setback before actions resume.
pub const SETTLE_TICKS: u64 = 10;
/// Above this round-trip [`Governor::lagging`] is true.
const LAGGING: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Actions sent and not yet confirmed or given up on.
    pub max_in_flight: u32,
    /// Ticks between two actions: each gap is drawn from `min..=max`, so clicks never keep a tick-locked
    /// rhythm (anticheats flag low click-interval variance).
    pub min_gap_ticks: u64,
    pub max_gap_ticks: u64,
}

impl Limits {
    /// BDS kept 6 placements a tick from one spot (place_rate probe, 2026-10-19); 4 leaves room.
    pub const DEFAULT_IN_FLIGHT: u32 = 4;
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_in_flight: Self::DEFAULT_IN_FLIGHT, min_gap_ticks: 1, max_gap_ticks: 3 }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Governor {
    pub limits: Limits,
    tick: u64,
    /// Smoothed echo round-trip and its mean deviation (RFC 6298), in seconds.
    srtt: Option<f32>,
    rttvar: f32,
    samples: u32,
    in_flight: u32,
    last_sent: Option<u64>,
    /// The gap drawn after the last action.
    gap: u64,
    rng: u64,
    sent_this_tick: u32,
    /// Ticks since the movement's last setback, as of this tick.
    setback_age: Option<u64>,
    sent: u64,
    retries: u64,
}

impl Governor {
    pub fn new(limits: Limits) -> Self {
        Self { limits, ..Self::default() }
    }

    /// Advances one client tick; `setback_age`: ticks since the server last set the bot back.
    pub fn tick(&mut self, setback_age: Option<u64>) {
        self.tick += 1;
        self.setback_age = setback_age;
        self.sent_this_tick = 0;
    }

    pub fn now(&self) -> u64 {
        self.tick
    }

    /// An echo arrived `rtt` after its action (first tries only: a retried action's echo is ambiguous).
    pub fn sample(&mut self, rtt: Duration) {
        let r = rtt.as_secs_f32();
        match self.srtt {
            None => (self.srtt, self.rttvar) = (Some(r), r / 2.0),
            Some(s) => {
                self.rttvar = 0.75 * self.rttvar + 0.25 * (s - r).abs();
                self.srtt = Some(0.875 * s + 0.125 * r);
            }
        }
        self.samples += 1;
    }

    /// Smoothed echo round-trip; `None` before the first echo.
    pub fn rtt(&self) -> Option<Duration> {
        self.srtt.map(Duration::from_secs_f32)
    }

    pub fn samples(&self) -> u32 {
        self.samples
    }

    pub fn lagging(&self) -> bool {
        self.rtt().is_some_and(|r| r > scaled(LAGGING))
    }

    /// How long to wait for an action's echo before re-reading the cell.
    pub fn timeout(&self) -> Duration {
        let wait = match self.srtt {
            None => scaled(DEFAULT_RTT * 4),
            Some(s) => Duration::from_secs_f32(s + 4.0 * self.rttvar) + 2 * scaled(TICK),
        };
        wait.clamp(scaled(MIN_TIMEOUT), scaled(MAX_TIMEOUT))
    }

    /// [`Governor::timeout`] in client ticks.
    pub fn timeout_ticks(&self) -> u64 {
        unscaled(self.timeout()).as_millis().div_ceil(TICK.as_millis()) as u64
    }

    /// The server set the bot back within the last [`SETTLE_TICKS`].
    pub fn settling(&self) -> bool {
        self.setback_age.is_some_and(|a| a < SETTLE_TICKS)
    }

    /// Whether an action may go out this tick.
    pub fn may_send(&self) -> bool {
        self.in_flight < self.limits.max_in_flight
            && !self.settling()
            && self.last_sent.is_none_or(|t| self.tick >= t + self.gap.max(self.limits.min_gap_ticks))
    }

    /// An action went out (a retry of one counts again).
    pub fn sent(&mut self, retry: bool) {
        self.last_sent = Some(self.tick);
        let Limits { min_gap_ticks: lo, max_gap_ticks: hi, .. } = self.limits;
        self.gap = lo + crate::cadence::splitmix(&mut self.rng) % (hi.max(lo) - lo + 1);
        self.sent_this_tick += 1;
        self.sent += 1;
        self.retries += u64::from(retry);
        if !retry {
            self.in_flight += 1;
        }
    }

    /// An action was confirmed or given up on.
    pub fn settled(&mut self) {
        self.in_flight = self.in_flight.saturating_sub(1);
    }

    /// Actions sent in the current tick.
    pub fn sent_this_tick(&self) -> u32 {
        self.sent_this_tick
    }

    pub fn in_flight(&self) -> u32 {
        self.in_flight
    }

    /// Actions sent, retries included.
    pub fn actions_sent(&self) -> u64 {
        self.sent
    }

    pub fn retries(&self) -> u64 {
        self.retries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_follows_the_measured_round_trip() {
        let mut g = Governor::default();
        assert_eq!(g.timeout(), Duration::from_millis(600));
        for _ in 0..20 {
            g.sample(Duration::from_millis(400));
        }
        let t = g.timeout().as_millis();
        assert!((500..=600).contains(&t), "steady 400 ms echo: timeout {t} ms");
        g.sample(Duration::from_millis(1400));
        assert!(g.timeout().as_millis() > 1200, "a spike widens it: {:?}", g.timeout());
        assert!(g.lagging());
        assert_eq!(Governor::default().timeout_ticks(), 12);
    }

    #[test]
    fn caps_actions_and_holds_after_setbacks() {
        let mut g = Governor::new(Limits { max_in_flight: 1, min_gap_ticks: 2, max_gap_ticks: 2 });
        assert!(g.may_send());
        g.sent(false);
        assert!(!g.may_send(), "one in flight");
        g.settled();
        g.tick(None);
        assert!(!g.may_send(), "gap of 2 ticks");
        g.tick(None);
        assert!(g.may_send());
        g.tick(Some(0));
        assert!(!g.may_send() && g.settling(), "setback this tick");
        g.tick(Some(SETTLE_TICKS));
        assert!(g.may_send());
    }

    #[test]
    fn gaps_vary_within_the_limits() {
        let mut g = Governor::new(Limits { max_in_flight: 9, min_gap_ticks: 1, max_gap_ticks: 3 });
        let mut gaps = std::collections::BTreeSet::new();
        for _ in 0..200 {
            g.sent(false);
            g.settled();
            let mut waited = 0;
            while { g.tick(None); waited += 1; !g.may_send() } {}
            gaps.insert(waited);
        }
        assert_eq!(gaps.into_iter().collect::<Vec<_>>(), vec![1, 2, 3]);
    }
}
