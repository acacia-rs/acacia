//! The outbound stream: each shard buffers its bots' records and flushes them as one JSON batch per wake-up.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use acacia_api::{batch_json, Out};
use parking_lot::Mutex;
use tokio::sync::mpsc::{self, error::TrySendError};
use tokio::sync::Notify;
use tokio::time::Instant;

/// The consuming end of the host's stream: each item is a JSON array of [`Out`] records.
pub struct Outbound(mpsc::Receiver<String>);

impl Outbound {
    /// The next batch; `None` once the host has shut down and every batch was taken.
    pub async fn recv(&mut self) -> Option<String> {
        self.0.recv().await
    }

    /// [`Outbound::recv`] for threads outside any tokio runtime (a napi delivery thread).
    pub fn blocking_recv(&mut self) -> Option<String> {
        self.0.blocking_recv()
    }

    pub fn try_recv(&mut self) -> Option<String> {
        self.0.try_recv().ok()
    }
}

pub(crate) fn channel(batches: usize) -> (mpsc::Sender<String>, Outbound) {
    let (tx, rx) = mpsc::channel(batches);
    (tx, Outbound(rx))
}

/// One shard's records not yet flushed. Pushed from the shard's thread (and from a panic report).
pub(crate) struct ShardBuf {
    records: Mutex<Vec<Out>>,
    notify: Notify,
    stopped: AtomicBool,
}

impl ShardBuf {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self { records: Mutex::new(Vec::new()), notify: Notify::new(), stopped: AtomicBool::new(false) })
    }

    pub(crate) fn push(&self, record: Out) {
        self.records.lock().push(record);
        self.notify.notify_one();
    }

    /// The flusher sends what is left and ends.
    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        self.notify.notify_one();
    }
}

pub(crate) struct Flusher {
    pub buf: Arc<ShardBuf>,
    pub tx: mpsc::Sender<String>,
    /// At most one batch per this long, so bots ticking together share one.
    pub every: Duration,
    /// Past this many held records, lossy events are dropped and counted (`lagged`).
    pub max_held: usize,
}

impl Flusher {
    pub(crate) async fn run(self) {
        let mut held = Vec::new();
        let mut last = Instant::now();
        loop {
            self.buf.notify.notified().await;
            tokio::time::sleep_until(last + self.every).await;
            last = Instant::now();
            held.append(&mut self.buf.records.lock());
            held = self.flush(held);
            if self.buf.stopped.load(Ordering::Acquire) {
                held.append(&mut self.buf.records.lock());
                self.flush(held);
                return;
            }
            if !held.is_empty() {
                // Retry next round even if no bot pushes again.
                self.buf.notify.notify_one();
            }
        }
    }

    /// Sends `held` as one batch; what could not go out is returned to merge into the next one.
    fn flush(&self, mut held: Vec<Out>) -> Vec<Out> {
        if held.is_empty() {
            return held;
        }
        match self.tx.try_send(batch_json(&held)) {
            Ok(()) | Err(TrySendError::Closed(_)) => Vec::new(),
            Err(TrySendError::Full(_)) => {
                if held.len() > self.max_held {
                    let before = held.len();
                    held.retain(|r| !matches!(r, Out::Event { event, .. } if event.is_lossy()));
                    let dropped = (before - held.len()) as u64;
                    if dropped > 0 {
                        held.push(Out::Lagged { dropped });
                    }
                }
                held
            }
        }
    }
}
