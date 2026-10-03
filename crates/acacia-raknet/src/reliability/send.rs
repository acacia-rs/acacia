use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use bytes::{Bytes, BytesMut};

use super::congestion::Congestion;
use super::CHANNELS;
use crate::wire::datagram::{
    put_datagram_header, Frame, Reliability, Split, DATAGRAM_HEADER_LEN, MAX_FRAME_HEADER_LEN,
};
use crate::wire::{unwrap24, U24_MASK};

/// Largest frame header when the message is not split.
const UNSPLIT_HEADER_LEN: usize = MAX_FRAME_HEADER_LEN - 10;

struct InFlight {
    sent_at: Instant,
    frames: Vec<Frame>,
}

pub(crate) struct SendQueue {
    next_datagram: u64,
    next_reliable: u32,
    next_split_id: u16,
    order_index: [u32; CHANNELS],
    sequence_index: [u32; CHANNELS],
    /// Pings and pongs: ahead of `pending` and outside the window, like RakNet's immediate priority.
    control: VecDeque<Frame>,
    pending: VecDeque<Frame>,
    /// Body bytes in `pending`.
    queued_bytes: usize,
    in_flight: BTreeMap<u64, InFlight>,
    congestion: Congestion,
    /// Whether the window refused a datagram since the queue last ran dry; see `congestion.rs`.
    window_limited: bool,
    resent: u64,
}

impl SendQueue {
    /// `windowed` applies the congestion window; without it everything queued is sent at once.
    pub fn new(windowed: bool) -> Self {
        Self {
            next_datagram: 0,
            next_reliable: 0,
            next_split_id: 0,
            order_index: [0; CHANNELS],
            sequence_index: [0; CHANNELS],
            control: VecDeque::new(),
            pending: VecDeque::new(),
            queued_bytes: 0,
            in_flight: BTreeMap::new(),
            congestion: Congestion::new(windowed),
            window_limited: false,
            resent: 0,
        }
    }

    pub fn rtt(&self) -> Option<Duration> {
        self.congestion.srtt()
    }

    /// Message bytes not yet sent, or waiting to be resent.
    pub fn queued_bytes(&self) -> usize {
        self.queued_bytes
    }

    /// Datagrams awaiting their ACK.
    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    pub fn window(&self) -> usize {
        self.congestion.window()
    }

    /// Datagrams whose frames had to be sent again.
    pub fn resent(&self) -> u64 {
        self.resent
    }

    fn take_reliable_index(&mut self) -> u32 {
        let idx = self.next_reliable;
        self.next_reliable = (idx + 1) & U24_MASK;
        idx
    }

    /// Queues a ping or pong: unreliable, unordered and never split.
    pub fn push_control(&mut self, body: Bytes) {
        let reliability = Reliability::Unreliable;
        self.control.push_back(Frame { reliability, reliable_index: 0, sequence_index: 0, order_index: 0, order_channel: 0, split: None, body });
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

        self.queued_bytes += body.len();
        let room = max_payload.saturating_sub(DATAGRAM_HEADER_LEN);
        if body.len() <= room.saturating_sub(UNSPLIT_HEADER_LEN) {
            let reliable_index = if reliability.is_reliable() { self.take_reliable_index() } else { 0 };
            self.pending.push_back(template(reliability, reliable_index, None, body));
            return;
        }

        let reliability = reliability.for_split();
        let chunk = room.saturating_sub(MAX_FRAME_HEADER_LEN).max(1);
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

    /// Packs one datagram: control frames, then queued frames if the congestion window has room.
    /// Reliable frames are tracked for resend.
    pub fn pack(&mut self, now: Instant, max_payload: usize) -> Option<Bytes> {
        let open = self.in_flight.len() < self.congestion.window();
        match (self.pending.is_empty(), open) {
            (true, true) => self.window_limited = false,
            (false, false) => self.window_limited = true,
            _ => {}
        }
        self.pack_from(now, max_payload, open)
    }

    /// Packs everything queued, window or not: the last words before a close.
    pub fn flush(&mut self, now: Instant, max_payload: usize) -> Vec<Bytes> {
        std::iter::from_fn(|| self.pack_from(now, max_payload, true)).collect()
    }

    fn pack_from(&mut self, now: Instant, max_payload: usize, window_open: bool) -> Option<Bytes> {
        if self.control.is_empty() && (self.pending.is_empty() || !window_open) {
            return None;
        }
        let seq = self.next_datagram;
        self.next_datagram += 1;
        let mut buf = BytesMut::with_capacity(max_payload);
        put_datagram_header(&mut buf, seq as u32);
        // The first frame always goes, so an oversized one cannot block the queue.
        let fits = |buf: &BytesMut, f: &Frame| buf.len() == DATAGRAM_HEADER_LEN || buf.len() + f.encoded_len() <= max_payload;
        while self.control.front().is_some_and(|f| fits(&buf, f)) {
            self.control.pop_front().expect("front exists").encode(&mut buf);
        }
        let mut reliable = Vec::new();
        while window_open && self.pending.front().is_some_and(|f| fits(&buf, f)) {
            let f = self.pending.pop_front().expect("front exists");
            f.encode(&mut buf);
            self.queued_bytes -= f.body.len();
            if f.reliability.is_reliable() {
                reliable.push(f);
            }
        }
        if !reliable.is_empty() {
            self.in_flight.insert(seq, InFlight { sent_at: now, frames: reliable });
        }
        Some(buf.freeze())
    }

    /// Puts a lost datagram's frames back at the head of the queue.
    fn requeue(&mut self, lost: InFlight) {
        self.resent += 1;
        for f in lost.frames.into_iter().rev() {
            self.queued_bytes += f.body.len();
            self.pending.push_front(f);
        }
    }

    fn resolve_range(&self, start: u32, end: u32) -> (u64, u64) {
        let start = unwrap24(self.next_datagram.saturating_sub(1), start);
        (start, start + u64::from(end.wrapping_sub(start as u32) & U24_MASK))
    }

    pub fn on_ack(&mut self, now: Instant, start: u32, end: u32) {
        let (start, end) = self.resolve_range(start, end);
        while let Some(&seq) = self.in_flight.range(start..=end).next().map(|(k, _)| k) {
            let acked = self.in_flight.remove(&seq).expect("key from range");
            self.congestion.on_ack(now.saturating_duration_since(acked.sent_at), self.window_limited);
        }
    }

    pub fn on_nack(&mut self, start: u32, end: u32) {
        let (start, end) = self.resolve_range(start, end);
        // Newest first, so the oldest datagram's frames end up at the head of the queue.
        while let Some(&seq) = self.in_flight.range(start..=end).next_back().map(|(k, _)| k) {
            let lost = self.in_flight.remove(&seq).expect("key from range");
            self.congestion.on_nack(seq, self.next_datagram);
            self.requeue(lost);
        }
    }

    pub fn on_timeout(&mut self, now: Instant) {
        let rto = self.congestion.rto();
        let mut expired = Vec::new();
        while let Some(entry) = self.in_flight.first_entry() {
            if entry.get().sent_at + rto > now {
                break;
            }
            self.congestion.on_resend_timeout(*entry.key(), self.next_datagram);
            expired.push(entry.remove());
        }
        for lost in expired.into_iter().rev() {
            self.requeue(lost);
        }
    }

    pub fn next_resend(&self) -> Option<Instant> {
        self.in_flight.first_key_value().map(|(_, f)| f.sent_at + self.congestion.rto())
    }
}
