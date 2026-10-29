use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::Notify;

use super::{BotEvent, EventSink};

/// A sink that never blocks and a receiver for another task or thread. Past `capacity` queued events, lossy ones
/// ([`BotEvent::is_lossy`]) are dropped and counted in a [`BotEvent::Lagged`] where they would have been; the
/// rest are always kept. The receiver ends once every clone of the sink is gone (the bot and its config).
pub fn channel(capacity: usize) -> (EventSink, EventReceiver) {
    let shared = Arc::new(Shared { queue: Mutex::new(Queue::default()), notify: Notify::new(), capacity });
    let end = SinkEnd(shared.clone());
    (Arc::new(move |event| end.push(event)), EventReceiver(shared))
}

/// The consuming end of [`channel`].
pub struct EventReceiver(Arc<Shared>);

struct Shared {
    queue: Mutex<Queue>,
    notify: Notify,
    capacity: usize,
}

#[derive(Default)]
struct Queue {
    events: VecDeque<BotEvent>,
    closed: bool,
}

struct SinkEnd(Arc<Shared>);

impl SinkEnd {
    fn push(&self, event: BotEvent) {
        let mut queue = self.0.queue.lock();
        if event.is_lossy() && queue.events.len() >= self.0.capacity {
            match queue.events.back_mut() {
                Some(BotEvent::Lagged { dropped }) => *dropped += 1,
                _ => queue.events.push_back(BotEvent::Lagged { dropped: 1 }),
            }
        } else {
            queue.events.push_back(event);
        }
        drop(queue);
        self.0.notify.notify_one();
    }
}

impl Drop for SinkEnd {
    fn drop(&mut self) {
        self.0.queue.lock().closed = true;
        self.0.notify.notify_one();
    }
}

impl EventReceiver {
    /// The next event; `None` once the queue is empty and the sink is gone.
    pub async fn recv(&mut self) -> Option<BotEvent> {
        loop {
            {
                let mut queue = self.0.queue.lock();
                if let Some(event) = queue.events.pop_front() {
                    return Some(event);
                }
                if queue.closed {
                    return None;
                }
            }
            self.0.notify.notified().await;
        }
    }

    /// The next event if one is queued.
    pub fn try_recv(&mut self) -> Option<BotEvent> {
        self.0.queue.lock().events.pop_front()
    }

    /// Events queued now (a `Lagged` counts as one).
    pub fn len(&self) -> usize {
        self.0.queue.lock().events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
