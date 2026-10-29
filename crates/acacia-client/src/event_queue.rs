//! Events from the connection task to the [`crate::Client`]: one channel message per batch of events, with
//! at most `capacity` undelivered. A message per packet cost a send, a pop and a semaphore each.

use std::sync::Arc;

use tokio::sync::{mpsc, Semaphore};

use crate::Event;

/// The client is gone.
pub(crate) struct Closed;

pub(crate) fn channel(capacity: usize) -> (EventSender, EventReceiver) {
    let (tx, rx) = mpsc::unbounded_channel();
    let room = Arc::new(Semaphore::new(capacity));
    let receiver = EventReceiver { rx, room: room.clone(), batch: Vec::new().into_iter(), held: 0 };
    (EventSender { tx, room, capacity }, receiver)
}

pub(crate) struct EventSender {
    tx: mpsc::UnboundedSender<(Vec<Event>, usize)>,
    room: Arc<Semaphore>,
    capacity: usize,
}

impl EventSender {
    /// Sends the front of `pending` if there is room for it now.
    pub fn try_send(&self, pending: &mut Vec<Event>) -> Result<(), Closed> {
        let n = self.batch_len(pending);
        match self.room.try_acquire_many(n as u32) {
            Ok(permit) => {
                permit.forget();
                self.send_permitted(pending, n)
            }
            Err(tokio::sync::TryAcquireError::NoPermits) => Ok(()),
            Err(tokio::sync::TryAcquireError::Closed) => Err(Closed),
        }
    }

    /// Waits for room for the front of `pending`, then sends it. Cancel-safe: nothing is taken until room is.
    pub async fn send(&self, pending: &mut Vec<Event>) -> Result<(), Closed> {
        let n = self.batch_len(pending);
        self.room.acquire_many(n as u32).await.map_err(|_| Closed)?.forget();
        self.send_permitted(pending, n)
    }

    /// Sends `events` past the capacity: the last ones, after which the connection is gone.
    pub fn send_final(&self, events: Vec<Event>) -> Result<(), Closed> {
        self.tx.send((events, 0)).map_err(|_| Closed)
    }

    fn batch_len(&self, pending: &[Event]) -> usize {
        pending.len().min(self.capacity)
    }

    fn send_permitted(&self, pending: &mut Vec<Event>, n: usize) -> Result<(), Closed> {
        // Usually the whole queue goes: hand its buffer over rather than copy it out.
        // Capped: a join burst's size would ride along with every later batch.
        let batch = if n == pending.len() { std::mem::replace(pending, Vec::with_capacity(n.min(64))) } else { pending.drain(..n).collect() };
        self.tx.send((batch, n)).map_err(|_| Closed)
    }
}

pub(crate) struct EventReceiver {
    rx: mpsc::UnboundedReceiver<(Vec<Event>, usize)>,
    room: Arc<Semaphore>,
    batch: std::vec::IntoIter<Event>,
    /// Permits taken for the current batch, returned once it is used up.
    held: usize,
}

impl EventReceiver {
    /// Cancel-safe.
    pub async fn recv(&mut self) -> Option<Event> {
        loop {
            if let Some(event) = self.next_in_batch() {
                return Some(event);
            }
            let batch = self.rx.recv().await?;
            self.start(batch);
        }
    }

    pub fn try_recv(&mut self) -> Option<Event> {
        loop {
            if let Some(event) = self.next_in_batch() {
                return Some(event);
            }
            let batch = self.rx.try_recv().ok()?;
            self.start(batch);
        }
    }

    /// Returns a used-up batch's room before anyone waits: the sender may be waiting for it.
    fn next_in_batch(&mut self) -> Option<Event> {
        let event = self.batch.next();
        if event.is_none() {
            self.room.add_permits(std::mem::take(&mut self.held));
        }
        event
    }

    fn start(&mut self, (events, permits): (Vec<Event>, usize)) {
        self.held = permits;
        self.batch = events.into_iter();
    }
}

impl Drop for EventReceiver {
    /// Wakes a connection task waiting for room.
    fn drop(&mut self) {
        self.room.close();
    }
}

#[cfg(test)]
mod tests {
    use acacia_session::DisconnectReason;

    use super::*;

    fn events(n: usize) -> Vec<Event> {
        (0..n).map(|_| Event::Disconnected(DisconnectReason::LocalClose)).collect()
    }

    #[tokio::test]
    async fn holds_at_most_capacity_undelivered() {
        let (tx, mut rx) = channel(2);
        let mut pending = events(5);
        assert!(tx.try_send(&mut pending).is_ok());
        assert!(tx.try_send(&mut pending).is_ok());
        assert_eq!(pending.len(), 3, "the second send had no room");
        assert!(rx.try_recv().is_some() && rx.try_recv().is_some());
        // Finding the batch used up returns its room, before the receiver waits for more.
        assert!(rx.try_recv().is_none());
        let sent = tokio::time::timeout(std::time::Duration::from_secs(5), tx.send(&mut pending)).await;
        sent.expect("room came back").ok().expect("open");
        assert_eq!(pending.len(), 1);
        tx.send_final(std::mem::take(&mut pending)).ok().expect("open");
        assert_eq!(std::iter::from_fn(|| rx.try_recv()).count(), 3);
    }

    #[tokio::test]
    async fn a_dropped_receiver_wakes_the_sender() {
        let (tx, rx) = channel(1);
        let mut pending = events(2);
        assert!(tx.try_send(&mut pending).is_ok());
        let waiting = tokio::spawn(async move { tx.send(&mut pending).await.is_err() });
        drop(rx);
        assert!(waiting.await.unwrap());
    }
}
