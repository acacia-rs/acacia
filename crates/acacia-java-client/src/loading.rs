//! When the player enters a level, as `LevelLoadTracker` decides: the server's `LEVEL_CHUNKS_LOAD_START`, then the
//! player's own chunk, then the loading screen's 500 ms close delay. Only then does the client send `PlayerLoaded`,
//! and only then does `LocalPlayer` tick and send movement; a server simulates the player from that packet on.

/// `LevelLoadTracker.LEVEL_LOAD_CLOSE_DELAY_MS` in ticks.
const CLOSE_DELAY_TICKS: u32 = 10;
/// `CLIENT_WAIT_TIMEOUT_MS`: the chunk wait gives up after 30 s from the start.
const TIMEOUT_TICKS: u32 = 600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Loading {
    WaitingForServer { waited: u32 },
    WaitingForChunk { waited: u32 },
    Ready { shown: u32 },
    Loaded,
}

impl Default for Loading {
    fn default() -> Self {
        Loading::WaitingForServer { waited: 0 }
    }
}

impl Loading {
    /// `LEVEL_CHUNKS_LOAD_START` arrived.
    pub(crate) fn chunks_started(&mut self) {
        if let Loading::WaitingForServer { waited } = *self {
            *self = Loading::WaitingForChunk { waited };
        }
    }

    /// One client tick; `true` when `PlayerLoaded` goes out now. `chunk_ready`: the player's chunk is in, or it
    /// needs none (dead, or outside the build height).
    // Vanilla waits for the server forever; this gives up with the chunk timeout, as a server that never sends the
    // event would otherwise hold the bot in the loading screen.
    pub(crate) fn tick(&mut self, chunk_ready: bool) -> bool {
        *self = match *self {
            Loading::WaitingForServer { waited } | Loading::WaitingForChunk { waited } if waited >= TIMEOUT_TICKS => Loading::Ready { shown: 0 },
            Loading::WaitingForServer { waited } => Loading::WaitingForServer { waited: waited + 1 },
            Loading::WaitingForChunk { .. } if chunk_ready => Loading::Ready { shown: 0 },
            Loading::WaitingForChunk { waited } => Loading::WaitingForChunk { waited: waited + 1 },
            Loading::Ready { shown } if shown + 1 >= CLOSE_DELAY_TICKS => {
                *self = Loading::Loaded;
                return true;
            }
            Loading::Ready { shown } => Loading::Ready { shown: shown + 1 },
            Loading::Loaded => Loading::Loaded,
        };
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_half_a_second_after_the_chunk() {
        let mut l = Loading::default();
        assert!(!l.tick(true), "the server hasn't started sending chunks");
        l.chunks_started();
        assert!(!l.tick(false));
        assert!(!l.tick(true));
        let sent: Vec<bool> = (0..CLOSE_DELAY_TICKS).map(|_| l.tick(true)).collect();
        assert_eq!(sent.iter().filter(|s| **s).count(), 1);
        assert_eq!(sent.last(), Some(&true));
        assert_eq!(l, Loading::Loaded);
    }
}
