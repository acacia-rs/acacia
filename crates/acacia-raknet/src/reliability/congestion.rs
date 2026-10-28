//! Round-trip estimate, resend timeout and congestion window of the send side.
//!
//! The window is RakNet's sliding window (`CCRakNetSlidingWindow`) counted in datagrams: slow
//! start until the first loss, then one datagram of growth per window of ACKs. A NACK ends slow
//! start, as RakNet's `OnNAK` does, but halves the window only once [`BURST_LOSS`] datagrams of one
//! flight are NACKed: halving per NACK held a link with 2% random loss to about ten datagrams a
//! round trip, where BDS kept sending five times as fast, and never halving overran a narrow
//! link's queue (measured with acacia's `tools/netem`). A resend timeout restarts slow start and doubles the timeout until an
//! ACK comes back; an ACK for the datagram that timed out undoes both, since only its ACK was late
//! or lost. Losses among datagrams sent before the last reaction are the same event and change
//! nothing. The window grows only while the sender fills it, so a quiet connection cannot bank a
//! large window for its next burst.

use std::ops::RangeInclusive;
use std::time::Duration;

const INITIAL_RTO: Duration = Duration::from_millis(500);
const MIN_RTO: Duration = Duration::from_millis(200);
const MAX_RTO: Duration = Duration::from_secs(3);
const MAX_BACKOFF: u32 = 4;
const INITIAL_WINDOW: u32 = 10;
const MIN_WINDOW: u32 = 2;
const MAX_WINDOW: u32 = 2048;
/// NACKed datagrams in one flight that read as a full queue rather than random loss.
const BURST_LOSS: u32 = 3;

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

/// The window before the last resend timeout, and the datagram whose silence caused it.
struct Undo {
    seq: u64,
    window: u32,
    threshold: u32,
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
    /// Datagrams NACKed since the last reaction to a NACK.
    nacked: u32,
    undo: Option<Undo>,
}

impl Congestion {
    pub fn new(windowed: bool) -> Self {
        let window = windowed.then_some(INITIAL_WINDOW);
        Self { rtt: Rtt::default(), window, threshold: u32::MAX, acked: 0, recover: 0, backoff: 0, nacked: 0, undo: None }
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
            self.nacked = 0;
            if let Some(window) = self.window {
                self.threshold = (window / 2).max(MIN_WINDOW);
                self.acked = 0;
            }
        }
        self.nacked += 1;
        if self.nacked == BURST_LOSS
            && let Some(window) = &mut self.window
        {
            *window = (*window).min(self.threshold);
        }
    }

    /// Datagram `seq` went unanswered for a whole [`Congestion::rto`].
    pub fn on_resend_timeout(&mut self, seq: u64, next: u64) {
        if seq >= self.recover {
            self.recover = next;
            self.backoff = (self.backoff + 1).min(MAX_BACKOFF);
            if let Some(window) = &mut self.window {
                self.undo = Some(Undo { seq, window: *window, threshold: self.threshold });
                self.threshold = (*window / 2).max(MIN_WINDOW);
                *window = MIN_WINDOW;
                self.acked = 0;
            }
        }
    }

    /// The peer ACKed datagrams `seqs`, whether or not they were still awaited.
    pub fn on_received(&mut self, seqs: RangeInclusive<u64>) {
        if let Some(undo) = self.undo.take_if(|u| seqs.contains(&u.seq)) {
            self.backoff = 0;
            if let Some(window) = &mut self.window {
                *window = (*window).max(undo.window);
                self.threshold = undo.threshold;
            }
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
    fn slow_start_doubles_until_a_nack() {
        let mut c = Congestion::new(true);
        assert_eq!(c.window(), 10);
        acks(&mut c, 10);
        assert_eq!(c.window(), 20);
        c.on_nack(5, 30);
        assert_eq!(c.window(), 20, "a NACK keeps the window");
        acks(&mut c, 20);
        assert_eq!(c.window(), 21, "after slow start, one more per window of ACKs");
        acks(&mut c, 20);
        assert_eq!(c.window(), 21);
    }

    #[test]
    fn a_burst_of_nacks_in_one_flight_halves_once() {
        let mut c = Congestion::new(true);
        acks(&mut c, 10);
        c.on_nack(5, 30);
        c.on_nack(6, 30);
        assert_eq!(c.window(), 20);
        c.on_nack(7, 30);
        assert_eq!(c.window(), 10);
        c.on_nack(8, 30);
        assert_eq!(c.window(), 10, "one flight halves once");
    }

    #[test]
    fn an_ack_for_the_timed_out_datagram_undoes_the_timeout() {
        let mut c = Congestion::new(true);
        acks(&mut c, 10);
        c.on_resend_timeout(4, 30);
        assert_eq!((c.window(), c.rto()), (2, MIN_RTO * 2));
        c.on_received(0..=3);
        assert_eq!(c.window(), 2, "older datagrams say nothing of the one that timed out");
        c.on_received(4..=6);
        assert_eq!((c.window(), c.rto()), (20, MIN_RTO));
        c.on_received(4..=6);
        assert_eq!(c.window(), 20, "undone once");
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
