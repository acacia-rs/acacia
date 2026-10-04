//! Round-trip estimate, resend timeout and congestion window of the send side.
//!
//! The window is RakNet's sliding window (`CCRakNetSlidingWindow`) counted in datagrams: slow
//! start until the first loss, then one datagram of growth per window of ACKs. A NACK halves it;
//! a resend timeout restarts slow start and doubles the timeout until an ACK comes back. Losses
//! among datagrams sent before the last reaction are the same event and change nothing. The
//! window grows only while the sender fills it, so a quiet connection cannot bank a large
//! window for its next burst.

use std::time::Duration;

const INITIAL_RTO: Duration = Duration::from_millis(500);
const MIN_RTO: Duration = Duration::from_millis(200);
const MAX_RTO: Duration = Duration::from_secs(3);
const MAX_BACKOFF: u32 = 4;
const INITIAL_WINDOW: u32 = 10;
const MIN_WINDOW: u32 = 2;
const MAX_WINDOW: u32 = 2048;

/// RFC 6298 round-trip estimator.
#[derive(Default)]
struct Rtt {
    srtt: Option<Duration>,
    rttvar: Duration,
}

impl Rtt {
    fn sample(&mut self, rtt: Duration) {
        match self.srtt {
            None => {
                self.srtt = Some(rtt);
                self.rttvar = rtt / 2;
            }
            Some(srtt) => {
                self.rttvar = (self.rttvar * 3 + srtt.abs_diff(rtt)) / 4;
                self.srtt = Some((srtt * 7 + rtt) / 8);
            }
        }
    }

    fn rto(&self) -> Duration {
        self.srtt.map_or(INITIAL_RTO, |s| (s + self.rttvar * 4).clamp(MIN_RTO, MAX_RTO))
    }
}

pub(crate) struct Congestion {
    rtt: Rtt,
    /// `None` sends without a window.
    window: Option<u32>,
    /// Slow start ends once the window reaches this.
    threshold: u32,
    /// ACKs towards the next step of growth after slow start.
    acked: u32,
    /// Datagrams numbered below this were sent before the last reaction to a loss.
    recover: u64,
    backoff: u32,
}

impl Congestion {
    pub fn new(windowed: bool) -> Self {
        Self { rtt: Rtt::default(), window: windowed.then_some(INITIAL_WINDOW), threshold: u32::MAX, acked: 0, recover: 0, backoff: 0 }
    }

    pub fn srtt(&self) -> Option<Duration> {
        self.rtt.srtt
    }

    pub fn rto(&self) -> Duration {
        (self.rtt.rto() * (1 << self.backoff)).min(MAX_RTO)
    }

    /// How many datagrams may await their ACK at once.
    pub fn window(&self) -> usize {
        self.window.map_or(usize::MAX, |w| w as usize)
    }

    /// `limited` says the window, not a lack of data, is what last held the sender back.
    pub fn on_ack(&mut self, rtt: Duration, limited: bool) {
        self.rtt.sample(rtt);
        self.backoff = 0;
        let Some(window) = &mut self.window else { return };
        if !limited {
            return;
        }
        if *window < self.threshold {
            *window += 1;
        } else {
            self.acked += 1;
            if self.acked >= *window {
                self.acked = 0;
                *window += 1;
            }
        }
        *window = (*window).min(MAX_WINDOW);
    }

    /// Datagram `seq` was NACKed; the next datagram sent gets number `next`.
    pub fn on_nack(&mut self, seq: u64, next: u64) {
        if seq >= self.recover {
            self.recover = next;
            self.shrink(|half| half);
        }
    }

    /// Datagram `seq` went unanswered for a whole [`Congestion::rto`].
    pub fn on_resend_timeout(&mut self, seq: u64, next: u64) {
        if seq >= self.recover {
            self.recover = next;
            self.backoff = (self.backoff + 1).min(MAX_BACKOFF);
            self.shrink(|_| MIN_WINDOW);
        }
    }

    fn shrink(&mut self, to: impl Fn(u32) -> u32) {
        if let Some(window) = &mut self.window {
            self.threshold = (*window / 2).max(MIN_WINDOW);
            *window = to(self.threshold);
            self.acked = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RTT: Duration = Duration::from_millis(50);

    /// `n` ACKs for a sender the window is holding back.
    fn acks(c: &mut Congestion, n: usize) {
        for _ in 0..n {
            c.on_ack(RTT, true);
        }
    }

    #[test]
    fn slow_start_doubles_and_a_nack_halves() {
        let mut c = Congestion::new(true);
        assert_eq!(c.window(), 10);
        acks(&mut c, 10);
        assert_eq!(c.window(), 20);
        c.on_nack(5, 30);
        assert_eq!(c.window(), 10);
        c.on_nack(6, 31);
        assert_eq!(c.window(), 10, "a second loss in the same flight is the same event");
        acks(&mut c, 10);
        assert_eq!(c.window(), 11, "after slow start, one more per window of ACKs");
        acks(&mut c, 10);
        assert_eq!(c.window(), 11);
        c.on_nack(31, 50);
        assert_eq!(c.window(), 5);
    }

    #[test]
    fn a_resend_timeout_restarts_slow_start_and_backs_off() {
        let mut c = Congestion::new(true);
        assert_eq!(c.rto(), INITIAL_RTO);
        c.on_resend_timeout(0, 10);
        assert_eq!((c.window(), c.rto()), (2, INITIAL_RTO * 2));
        c.on_resend_timeout(3, 12);
        assert_eq!(c.rto(), INITIAL_RTO * 2, "sent before the first timeout");
        c.on_resend_timeout(10, 14);
        c.on_resend_timeout(14, 16);
        assert_eq!(c.rto(), MAX_RTO);
        acks(&mut c, 1);
        assert_eq!(c.rto(), MIN_RTO, "an ACK ends the backoff");
        acks(&mut c, 2);
        assert_eq!(c.window(), 3);
    }

    #[test]
    fn a_window_that_is_not_filled_does_not_grow() {
        let mut c = Congestion::new(true);
        for _ in 0..100 {
            c.on_ack(RTT, false);
        }
        assert_eq!(c.window(), 10);
    }

    #[test]
    fn without_a_window_only_the_timeout_backs_off() {
        let mut c = Congestion::new(false);
        c.on_resend_timeout(0, 10);
        c.on_nack(10, 20);
        assert_eq!((c.window(), c.rto()), (usize::MAX, INITIAL_RTO * 2));
    }
}
