use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use bytes::{Bytes, BytesMut};

use super::CHANNELS;
use crate::wire::datagram::{encode_acks, Frame, Split, FLAG_ACK, FLAG_NACK};
use crate::wire::{unwrap24, U24_MASK};

const MAX_SPLIT_COUNT: u32 = 8192;
const MAX_CONCURRENT_SPLITS: usize = 64;
const MAX_ORDER_BACKLOG: usize = 8192;
const MAX_RELIABLE_WINDOW: usize = 1 << 16;
const MAX_NACK_GAP: u64 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecvError {
    BadSplit,
    TooManySplits,
    OrderBacklogFull,
    ReliableWindowFull,
}

impl std::fmt::Display for RecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for RecvError {}

#[derive(Default)]
struct OrderChannel {
    expected: u64,
    highest_sequence: Option<u64>,
    backlog: BTreeMap<u64, Bytes>,
}

struct SplitBuffer {
    parts: Vec<Option<Bytes>>,
    received: u32,
    len: usize,
}

#[derive(Default)]
pub(crate) struct RecvState {
    highest_datagram: Option<u64>,
    acks: Vec<u32>,
    nacks: Vec<u32>,
    reliable_base: u64,
    reliable_seen: HashSet<u64>,
    channels: [OrderChannel; CHANNELS],
    splits: HashMap<u16, SplitBuffer>,
    pub ready: VecDeque<Bytes>,
}

impl RecvState {
    pub fn on_datagram(&mut self, seq24: u32) {
        self.acks.push(seq24);
        let seq = unwrap24(self.highest_datagram.unwrap_or(0), seq24);
        let next = self.highest_datagram.map_or(0, |h| h + 1);
        if seq < next {
            // Late, not lost: arrived before the tick that would have NACKed it.
            self.nacks.retain(|&n| n != seq24);
            return;
        }
        for missing in next.max(seq.saturating_sub(MAX_NACK_GAP))..seq {
            self.nacks.push(missing as u32 & U24_MASK);
        }
        self.highest_datagram = Some(seq);
    }

    pub fn on_frame(&mut self, frame: Frame) -> Result<(), RecvError> {
        if frame.reliability.is_reliable() && !self.mark_reliable(frame.reliable_index)? {
            return Ok(());
        }
        let frame = match frame.split {
            Some(split) => match self.reassemble(frame, split)? {
                Some(whole) => whole,
                None => return Ok(()),
            },
            None => frame,
        };
        self.deliver(frame)
    }

    /// Returns false if this reliable index was already seen.
    fn mark_reliable(&mut self, idx24: u32) -> Result<bool, RecvError> {
        let idx = unwrap24(self.reliable_base, idx24);
        if idx < self.reliable_base || self.reliable_seen.contains(&idx) {
            return Ok(false);
        }
        if idx == self.reliable_base {
            self.reliable_base += 1;
            while self.reliable_seen.remove(&self.reliable_base) {
                self.reliable_base += 1;
            }
        } else {
            if self.reliable_seen.len() >= MAX_RELIABLE_WINDOW {
                return Err(RecvError::ReliableWindowFull);
            }
            self.reliable_seen.insert(idx);
        }
        Ok(true)
    }

    fn reassemble(&mut self, frame: Frame, split: Split) -> Result<Option<Frame>, RecvError> {
        if split.count == 0 || split.count > MAX_SPLIT_COUNT || split.index >= split.count {
            return Err(RecvError::BadSplit);
        }
        if !self.splits.contains_key(&split.id) && self.splits.len() >= MAX_CONCURRENT_SPLITS {
            return Err(RecvError::TooManySplits);
        }
        let buf = self.splits.entry(split.id).or_insert_with(|| SplitBuffer {
            parts: vec![None; split.count as usize],
            received: 0,
            len: 0,
        });
        if buf.parts.len() != split.count as usize {
            return Err(RecvError::BadSplit);
        }
        let slot = &mut buf.parts[split.index as usize];
        if slot.is_none() {
            buf.received += 1;
            buf.len += frame.body.len();
            *slot = Some(frame.body.clone());
        }
        if buf.received < split.count {
            return Ok(None);
        }
        let buf = self.splits.remove(&split.id).expect("entry exists");
        let mut whole = BytesMut::with_capacity(buf.len);
        for part in buf.parts.into_iter().flatten() {
            whole.extend_from_slice(&part);
        }
        Ok(Some(Frame { split: None, body: whole.freeze(), ..frame }))
    }

    fn deliver(&mut self, frame: Frame) -> Result<(), RecvError> {
        let ch = &mut self.channels[usize::from(frame.order_channel) % CHANNELS];
        if frame.reliability.is_ordered() {
            let idx = unwrap24(ch.expected, frame.order_index);
            if idx < ch.expected {
                return Ok(());
            }
            if idx > ch.expected {
                if ch.backlog.len() >= MAX_ORDER_BACKLOG {
                    return Err(RecvError::OrderBacklogFull);
                }
                ch.backlog.insert(idx, frame.body);
                return Ok(());
            }
            self.ready.push_back(frame.body);
            ch.expected += 1;
            while let Some(body) = ch.backlog.remove(&ch.expected) {
                self.ready.push_back(body);
                ch.expected += 1;
            }
        } else if frame.reliability.is_sequenced() {
            let seq = unwrap24(ch.highest_sequence.unwrap_or(0), frame.sequence_index);
            if ch.highest_sequence.is_some_and(|h| seq < h) {
                return Ok(());
            }
            ch.highest_sequence = Some(seq);
            self.ready.push_back(frame.body);
        } else {
            self.ready.push_back(frame.body);
        }
        Ok(())
    }

    /// Returns one ACK (preferred) or NACK datagram if any are pending.
    pub fn poll_ack(&mut self, max_payload: usize) -> Option<Bytes> {
        let (list, flag) = if !self.acks.is_empty() {
            (&mut self.acks, FLAG_ACK)
        } else if !self.nacks.is_empty() {
            (&mut self.nacks, FLAG_NACK)
        } else {
            return None;
        };
        list.sort_unstable();
        list.dedup();
        let mut buf = BytesMut::with_capacity(max_payload.min(3 + list.len() * 7));
        let used = encode_acks(&mut buf, flag, list, max_payload);
        list.drain(..used);
        Some(buf.freeze())
    }
}
