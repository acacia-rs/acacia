//! One direction of an impaired link: whether a datagram gets through, and when it arrives.
//! Loss is drawn first, then the datagram waits its turn on a link of `rate` (dropped where the
//! queue is full), then the delay and jitter are added; arrivals never overtake each other.

use std::time::{Duration, Instant};

/// Bits in a byte times nanoseconds in a second: a byte at `rate` bits a second takes this over
/// `rate` nanoseconds.
const NANOS_PER_BYTE_SECOND: u128 = 8 * 1_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impairment {
    pub delay: Duration,
    /// Up to this much more, drawn per datagram.
    pub jitter: Duration,
    /// The chance, 0 to 1, that a datagram is lost.
    pub loss: f64,
    /// Bits a second; `None` is no cap.
    pub rate: Option<u64>,
    /// Bytes that may wait for the link before more are dropped.
    pub queue: usize,
}

impl Default for Impairment {
    fn default() -> Self {
        Self { delay: Duration::ZERO, jitter: Duration::ZERO, loss: 0.0, rate: None, queue: 256 * 1024 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fate {
    Lost,
    /// The queue for the link was full.
    Overflow,
    Arrives(Instant),
}

pub struct Link {
    impairment: Impairment,
    /// When the link has sent everything queued so far.
    free_at: Instant,
    last_arrival: Instant,
    rng: u64,
}

impl Link {
    pub fn new(impairment: Impairment, now: Instant, seed: u64) -> Self {
        Self { impairment, free_at: now, last_arrival: now, rng: seed | 1 }
    }

    /// xorshift64*, as a fraction in [0, 1).
    fn draw(&mut self) -> f64 {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Bytes waiting for the link at `now`.
    pub fn backlog(&self, now: Instant) -> usize {
        self.impairment.rate.map_or(0, |rate| (self.free_at.saturating_duration_since(now).as_nanos() * u128::from(rate) / NANOS_PER_BYTE_SECOND) as usize)
    }

    pub fn admit(&mut self, now: Instant, size: usize) -> Fate {
        if self.draw() < self.impairment.loss {
            return Fate::Lost;
        }
        let start = self.free_at.max(now);
        self.free_at = match self.impairment.rate {
            Some(rate) => {
                if self.backlog(now) + size > self.impairment.queue {
                    return Fate::Overflow;
                }
                start + Duration::from_nanos((size as u128 * NANOS_PER_BYTE_SECOND / u128::from(rate)) as u64)
            }
            None => now,
        };
        let jitter = self.impairment.jitter.mul_f64(self.draw());
        let arrival = (self.free_at + self.impairment.delay + jitter).max(self.last_arrival);
        self.last_arrival = arrival;
        Fate::Arrives(arrival)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arrivals(link: &mut Link, now: Instant, sizes: &[usize]) -> Vec<Fate> {
        sizes.iter().map(|size| link.admit(now, *size)).collect()
    }

    #[test]
    fn a_clean_link_delays_each_datagram_by_its_delay() {
        let now = Instant::now();
        let mut link = Link::new(Impairment { delay: Duration::from_millis(150), ..Impairment::default() }, now, 7);
        assert_eq!(arrivals(&mut link, now, &[100, 1400]), [Fate::Arrives(now + Duration::from_millis(150)); 2]);
    }

    #[test]
    fn a_rate_spaces_datagrams_out_and_a_full_queue_drops() {
        let now = Instant::now();
        // 80 kbit/s sends 1000 bytes in 100 ms; the queue holds two such datagrams.
        let mut link = Link::new(Impairment { rate: Some(80_000), queue: 2000, ..Impairment::default() }, now, 7);
        let fates = arrivals(&mut link, now, &[1000, 1000, 1000]);
        assert_eq!(fates[..2], [Fate::Arrives(now + Duration::from_millis(100)), Fate::Arrives(now + Duration::from_millis(200))]);
        assert_eq!(fates[2], Fate::Overflow);
        assert_eq!(link.backlog(now), 2000);
        assert_eq!(link.backlog(now + Duration::from_millis(150)), 500);
    }

    #[test]
    fn loss_drops_about_its_share_and_jitter_never_reorders() {
        let now = Instant::now();
        let impairment = Impairment { loss: 0.1, delay: Duration::from_millis(50), jitter: Duration::from_millis(40), ..Impairment::default() };
        let mut link = Link::new(impairment, now, 42);
        let fates: Vec<Fate> = (0..10_000).map(|n| link.admit(now + Duration::from_micros(n * 100), 500)).collect();
        let lost = fates.iter().filter(|fate| **fate == Fate::Lost).count();
        assert!((900..1100).contains(&lost), "{lost}");
        let times: Vec<Instant> = fates.iter().filter_map(|fate| if let Fate::Arrives(at) = fate { Some(*at) } else { None }).collect();
        assert!(times.windows(2).all(|pair| pair[0] <= pair[1]));
    }
}
