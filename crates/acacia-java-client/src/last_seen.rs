//! What the client acknowledges of other players' signed chat: `LastSeenMessagesTracker` and
//! `MessageSignatureCache`, ported.

use std::collections::VecDeque;

use acacia_java_proto::chat::{LAST_SEEN, LastSeenUpdate, MessageSignature, PackedSignature};

/// `MessageSignatureCache.DEFAULT_CAPACITY`.
const CACHE_CAPACITY: usize = 128;

#[derive(Debug, Clone)]
struct Tracked {
    signature: MessageSignature,
    pending: bool,
}

/// `LastSeenMessagesTracker(20)`: a ring of the last 20 messages, `None` for ones not shown.
#[derive(Debug, Default)]
pub(crate) struct Tracker {
    slots: [Option<Tracked>; LAST_SEEN],
    tail: usize,
    offset: i32,
    last: Option<MessageSignature>,
}

impl Tracker {
    /// `addPending`: false for a repeat of the last message.
    pub(crate) fn add_pending(&mut self, signature: MessageSignature, shown: bool) -> bool {
        if self.last.as_ref() == Some(&signature) {
            return false;
        }
        self.slots[self.tail] = shown.then(|| Tracked { signature: signature.clone(), pending: true });
        self.last = Some(signature);
        self.tail = (self.tail + 1) % LAST_SEEN;
        self.offset += 1;
        true
    }

    /// `ignorePending`: the server deleted a message we have not acknowledged yet.
    pub(crate) fn ignore_pending(&mut self, signature: &MessageSignature) {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.as_ref().is_some_and(|t| t.pending && &t.signature == signature)) {
            *slot = None;
        }
    }

    pub(crate) fn offset(&self) -> i32 {
        self.offset
    }

    pub(crate) fn take_offset(&mut self) -> i32 {
        std::mem::take(&mut self.offset)
    }

    /// `generateAndApplyUpdate`: the signatures a message signs as seen, oldest first, and the update telling the
    /// server which they are.
    pub(crate) fn update(&mut self) -> (Vec<MessageSignature>, LastSeenUpdate) {
        let offset = self.take_offset();
        let mut acknowledged = 0;
        let mut seen = Vec::with_capacity(LAST_SEEN);
        for i in 0..LAST_SEEN {
            if let Some(tracked) = &mut self.slots[(self.tail + i) % LAST_SEEN] {
                acknowledged |= 1 << i;
                seen.push(tracked.signature.clone());
                tracked.pending = false;
            }
        }
        let checksum = checksum(&seen);
        (seen, LastSeenUpdate { offset, acknowledged, checksum })
    }
}

/// `LastSeenMessages.computeChecksum`: Java's list-style hash of each signature's `Arrays.hashCode`, as a byte
/// that is never 0 (0 means "don't check").
pub(crate) fn checksum(seen: &[MessageSignature]) -> u8 {
    let hash = |bytes: &[u8]| bytes.iter().fold(1i32, |h, &b| h.wrapping_mul(31).wrapping_add(i32::from(b as i8)));
    let total = seen.iter().fold(1i32, |h, s| h.wrapping_mul(31).wrapping_add(hash(&s.0)));
    match total as u8 {
        0 => 1,
        byte => byte,
    }
}

/// `MessageSignatureCache`: what packed signatures refer to.
#[derive(Debug)]
pub(crate) struct SignatureCache(Vec<Option<MessageSignature>>);

impl Default for SignatureCache {
    fn default() -> Self {
        Self(vec![None; CACHE_CAPACITY])
    }
}

impl SignatureCache {
    pub(crate) fn unpack(&self, packed: &PackedSignature) -> Option<MessageSignature> {
        match packed {
            PackedSignature::Full(signature) => Some(signature.clone()),
            PackedSignature::Cached(id) => self.0.get(*id).cloned().flatten(),
        }
    }

    /// `push(body, signature)`: the message's last-seen list then its own signature go to the front; displaced
    /// entries move back unless they are among the new ones.
    pub(crate) fn push(&mut self, last_seen: Vec<MessageSignature>, signature: Option<MessageSignature>) {
        let fresh: Vec<_> = last_seen.iter().cloned().chain(signature.clone()).collect();
        let mut queue: VecDeque<_> = last_seen.into_iter().chain(signature).collect();
        for slot in &mut self.0 {
            let Some(next) = queue.pop_back() else { break };
            if let Some(old) = slot.replace(next).filter(|old| !fresh.contains(old)) {
                queue.push_front(old);
            }
        }
    }
}
