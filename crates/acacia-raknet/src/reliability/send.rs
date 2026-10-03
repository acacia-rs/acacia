use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use bytes::{Bytes, BytesMut};

use super::CHANNELS;
use crate::wire::datagram::{
    put_datagram_header, Frame, Reliability, Split, DATAGRAM_HEADER_LEN, MAX_FRAME_HEADER_LEN,
};
use crate::wire::{unwrap24, U24_MASK};

const INITIAL_RTO: Duration = Duration::from_millis(500);
const MIN_RTO: Duration = Duration::from_millis(200);
const MAX_RTO: Duration = Duration::from_secs(3);
/// Largest frame header when the message is not split.
const UNSPLIT_HEADER_LEN: usize = MAX_FRAME_HEADER_LEN - 10;

struct InFlight {
    sent_at: Instant,
    frames: Vec<Frame>,
}

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

pub(crate) struct SendQueue {
    next_datagram: u64,
    next_reliable: u32,
    next_split_id: u16,
    order_index: [u32; CHANNELS],
    sequence_index: [u32; CHANNELS],
    pending: VecDeque<Frame>,
    in_flight: BTreeMap<u64, InFlight>,
    rtt: Rtt,
}

impl SendQueue {
    pub fn new() -> Self {
        Self {
            next_datagram: 0,
            next_reliable: 0,
            next_split_id: 0,
            order_index: [0; CHANNELS],
            sequence_index: [0; CHANNELS],
            pending: VecDeque::new(),
            in_flight: BTreeMap::new(),
            rtt: Rtt::default(),
        }
    }

    pub fn rtt(&self) -> Option<Duration> {
        self.rtt.srtt
    }

    fn take_reliable_index(&mut self) -> u32 {
        let idx = self.next_reliable;
        self.next_reliable = (idx + 1) & U24_MASK;
        idx
    }

    /// Queues a message, splitting it into fragments if it cannot fit in one datagram of `max_payload` bytes.
    pub fn push(&mut self, body: Bytes, reliability: Reliability, channel: u8, max_payload: usize) {
        let ch = usize::from(channel) % CHANNELS;
        let mut order_index = 0;
        let mut sequence_index = 0;
        if reliability.is_ordered() {
            order_index = self.order_index[ch];
            self.order_index[ch] = (order_index + 1) & U24_MASK;
        } else if reliability.is_sequenced() {
            order_index = self.order_index[ch];
            sequence_index = self.sequence_index[ch];
            self.sequence_index[ch] = (sequence_index + 1) & U24_MASK;
        }
        let template = |reliability, reliable_index, split, body| Frame {
            reliability,
            reliable_index,
            sequence_index,
            order_index,
            order_channel: ch as u8,
            split,
            body,
        };

        let room = max_payload - DATAGRAM_HEADER_LEN;
        if body.len() <= room - UNSPLIT_HEADER_LEN {
            let reliable_index = if reliability.is_reliable() { self.take_reliable_index() } else { 0 };
            self.pending.push_back(template(reliability, reliable_index, None, body));
            return;
        }

        let reliability = reliability.for_split();
        let chunk = room - MAX_FRAME_HEADER_LEN;
        let count = body.len().div_ceil(chunk) as u32;
        let id = self.next_split_id;
        self.next_split_id = id.wrapping_add(1);
        for (index, start) in (0..body.len()).step_by(chunk).enumerate() {
            let part = body.slice(start..(start + chunk).min(body.len()));
            let split = Split { count, id, index: index as u32 };
            let reliable_index = self.take_reliable_index();
            self.pending.push_back(template(reliability, reliable_index, Some(split), part));
        }
    }

    /// Packs queued frames into one datagram, tracking reliable frames for resend.
    pub fn pack(&mut self, now: Instant, max_payload: usize) -> Option<Bytes> {
        if self.pending.is_empty() {
            return None;
        }
        let seq = self.next_datagram;
        self.next_datagram += 1;
        let mut buf = BytesMut::with_capacity(max_payload);
        put_datagram_header(&mut buf, seq as u32);
        let mut reliable = Vec::new();
        while let Some(f) = self.pending.front() {
            if buf.len() > DATAGRAM_HEADER_LEN && buf.len() + f.encoded_len() > max_payload {
                break;
            }
            let f = self.pending.pop_front().expect("front exists");
            f.encode(&mut buf);
            if f.reliability.is_reliable() {
                reliable.push(f);
            }
        }
        if !reliable.is_empty() {
            self.in_flight.insert(seq, InFlight { sent_at: now, frames: reliable });
        }
        Some(buf.freeze())
    }

    fn resolve_range(&self, start: u32, end: u32) -> (u64, u64) {
        let start = unwrap24(self.next_datagram.saturating_sub(1), start);
        (start, start + u64::from(end.wrapping_sub(start as u32) & U24_MASK))
    }

    pub fn on_ack(&mut self, now: Instant, start: u32, end: u32) {
        let (start, end) = self.resolve_range(start, end);
        while let Some(&seq) = self.in_flight.range(start..=end).next().map(|(k, _)| k) {
            let acked = self.in_flight.remove(&seq).expect("key from range");
            self.rtt.sample(now.saturating_duration_since(acked.sent_at));
        }
    }

    pub fn on_nack(&mut self, start: u32, end: u32) {
        let (start, end) = self.resolve_range(start, end);
        while let Some(&seq) = self.in_flight.range(start..=end).next().map(|(k, _)| k) {
            let lost = self.in_flight.remove(&seq).expect("key from range");
            for f in lost.frames.into_iter().rev() {
                self.pending.push_front(f);
            }
        }
    }

    pub fn on_timeout(&mut self, now: Instant) {
        let rto = self.rtt.rto();
        while let Some(entry) = self.in_flight.first_entry() {
            if entry.get().sent_at + rto > now {
                break;
            }
            for f in entry.remove().frames.into_iter().rev() {
                self.pending.push_front(f);
            }
        }
    }

    pub fn next_resend(&self) -> Option<Instant> {
        self.in_flight.first_key_value().map(|(_, f)| f.sent_at + self.rtt.rto())
    }
}
