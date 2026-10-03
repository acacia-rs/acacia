//! Change notifications for consumers that cache derived data per section (renderer meshes).

use std::sync::mpsc::{Receiver, Sender, channel};

use parking_lot::Mutex;

/// What changed in a chunk column. Block coordinates are world coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkChange {
    /// The whole column was (re)decoded.
    Column { x: i32, z: i32 },
    /// One section, `section_y` = world y >> 4.
    Section { x: i32, section_y: i32, z: i32 },
    Block { x: i32, y: i32, z: i32 },
}

#[derive(Default)]
pub(crate) struct Notifier {
    subscribers: Mutex<Vec<Sender<ChunkChange>>>,
}

impl Notifier {
    pub(crate) fn subscribe(&self) -> Receiver<ChunkChange> {
        let (tx, rx) = channel();
        self.subscribers.lock().push(tx);
        rx
    }

    pub(crate) fn send(&self, change: ChunkChange) {
        let mut subs = self.subscribers.lock();
        if !subs.is_empty() {
            subs.retain(|s| s.send(change).is_ok());
        }
    }
}
