use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use bytes::{Bytes, BytesMut};

use super::CHANNELS;
use crate::wire::datagram::{encode_acks, Frame, Split, FLAG_ACK, FLAG_NACK};
use crate::wire::{unwrap24, U24_MASK};

const MAX_ORDER_BACKLOG: usize = 8192;
const MAX_RELIABLE_WINDOW: usize = 1 << 16;
const MAX_NACK_GAP: u64 = 1024;
/// Charged per buffered fragment or message on top of its bytes, for the map entry holding it.
const HELD_OVERHEAD: usize = 64;

/// What one peer may make us buffer. Going over any of them ends the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecvLimits {
    /// Most fragments in one split message.
    pub max_split_parts: u32,
    /// Most split messages in reassembly at once.
    pub max_splits: usize,
    /// Most bytes held in unfinished splits and in messages waiting for an earlier ordered one.
    pub max_buffered_bytes: usize,
}

impl RecvLimits {
    /// For a client: servers send multi-megabyte messages (resource packs, chunks) back to back.
    pub const CLIENT: Self = Self { max_split_parts: 8192, max_splits: 64, max_buffered_bytes: 64 << 20 };
    /// For a server's peers, whose largest message is the login (well under 1 MiB).
    pub const SERVER: Self = Self { max_split_parts: 512, max_splits: 16, max_buffered_bytes: 2 << 20 };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecvError {
    BadSplit,
    TooManySplits,
    BufferFull,
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

/// Fragments by index: memory follows what arrived, not the count the peer claims.
struct SplitBuffer {
    count: u32,
    parts: BTreeMap<u32, Bytes>,
    len: usize,
}

pub(crate) struct RecvState {
    limits: RecvLimits,
    highest_datagram: Option<u64>,
    acks: Vec<u32>,
    nacks: Vec<u32>,
    reliable_base: u64,
    reliable_seen: HashSet<u64>,
    channels: [OrderChannel; CHANNELS],
    splits: HashMap<u16, SplitBuffer>,
    /// Bytes charged against `limits.max_buffered_bytes`.
    buffered: usize,
    pub ready: VecDeque<Bytes>,
}

/// A frame body slices its datagram and keeps all of it alive, so a small body is copied out
/// before it is buffered: a peer cannot pin a full datagram per byte it is charged for.
fn detach(body: Bytes, datagram_len: usize) -> Bytes {
    if body.len() * 2 < datagram_len { Bytes::copy_from_slice(&body) } else { body }
}

impl RecvState {
    pub fn new(limits: RecvLimits) -> Self {
        Self {
            limits,
            highest_datagram: None,
            acks: Vec::new(),
            nacks: Vec::new(),
            reliable_base: 0,
            reliable_seen: HashSet::new(),
            channels: Default::default(),
            splits: HashMap::new(),
            buffered: 0,
            ready: VecDeque::new(),
        }
    }

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

    /// `datagram_len` is the size of the datagram the frame came in.
    pub fn on_frame(&mut self, frame: Frame, datagram_len: usize) -> Result<(), RecvError> {
        if frame.reliability.is_reliable() && !self.mark_reliable(frame.reliable_index)? {
            return Ok(());
        }
        match frame.split {
            Some(split) => match self.reassemble(frame, split, datagram_len)? {
                Some(whole) => self.deliver(whole, 0),
                None => Ok(()),
            },
            None => self.deliver(frame, datagram_len),
        }
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

    fn charge(&mut self, len: usize) -> Result<(), RecvError> {
        let total = self.buffered + len + HELD_OVERHEAD;
        if total > self.limits.max_buffered_bytes {
            return Err(RecvError::BufferFull);
        }
        self.buffered = total;
        Ok(())
    }

    fn reassemble(&mut self, frame: Frame, split: Split, datagram_len: usize) -> Result<Option<Frame>, RecvError> {
        if split.count == 0 || split.count > self.limits.max_split_parts || split.index >= split.count {
            return Err(RecvError::BadSplit);
        }
        match self.splits.get(&split.id) {
            Some(buf) if buf.count != split.count => return Err(RecvError::BadSplit),
            Some(buf) if buf.parts.contains_key(&split.index) => return Ok(None),
            Some(_) => {}
            None if self.splits.len() >= self.limits.max_splits => return Err(RecvError::TooManySplits),
            None => {}
        }
        self.charge(frame.body.len())?;
        let buf = self.splits.entry(split.id).or_insert_with(|| SplitBuffer { count: split.count, parts: BTreeMap::new(), len: 0 });
        buf.len += frame.body.len();
        buf.parts.insert(split.index, detach(frame.body.clone(), datagram_len));
        if buf.parts.len() < split.count as usize {
            return Ok(None);
        }
        let buf = self.splits.remove(&split.id).expect("entry exists");
        self.buffered -= buf.len + buf.parts.len() * HELD_OVERHEAD;
        let mut whole = BytesMut::with_capacity(buf.len);
        for part in buf.parts.values() {
            whole.extend_from_slice(part);
        }
        Ok(Some(Frame { split: None, body: whole.freeze(), ..frame }))
    }

    fn deliver(&mut self, frame: Frame, datagram_len: usize) -> Result<(), RecvError> {
        let ch = usize::from(frame.order_channel) % CHANNELS;
        if frame.reliability.is_ordered() {
            let expected = self.channels[ch].expected;
            let idx = unwrap24(expected, frame.order_index);
            if idx < expected || self.channels[ch].backlog.contains_key(&idx) {
                return Ok(());
            }
            if idx > expected {
                if self.channels[ch].backlog.len() >= MAX_ORDER_BACKLOG {
                    return Err(RecvError::OrderBacklogFull);
                }
                self.charge(frame.body.len())?;
                self.channels[ch].backlog.insert(idx, detach(frame.body, datagram_len));
                return Ok(());
            }
            self.ready.push_back(frame.body);
            let ch = &mut self.channels[ch];
            ch.expected += 1;
            while let Some(body) = ch.backlog.remove(&ch.expected) {
                self.buffered -= body.len() + HELD_OVERHEAD;
                self.ready.push_back(body);
                ch.expected += 1;
            }
        } else if frame.reliability.is_sequenced() {
            let ch = &mut self.channels[ch];
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

    #[cfg(test)]
    pub fn buffered(&self) -> usize {
        self.buffered
    }
}
