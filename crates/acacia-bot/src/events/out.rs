use std::collections::VecDeque;
use std::sync::Arc;

use super::BotEvent;

/// Takes each event the moment the bot produces it ([`crate::BotConfig::event_sink`]; see [`crate::events`]).
pub type EventSink = Arc<dyn Fn(BotEvent) + Send + Sync>;

/// Where the bot's events go: queued for [`crate::Bot::next`], or straight to a sink.
pub(crate) enum EventOut {
    Queue(VecDeque<BotEvent>),
    Sink(EventSink),
}

impl EventOut {
    pub(crate) fn new(sink: Option<EventSink>) -> Self {
        sink.map_or_else(|| Self::Queue(VecDeque::new()), Self::Sink)
    }

    pub(crate) fn push(&mut self, event: BotEvent) {
        match self {
            Self::Queue(queue) => queue.push_back(event),
            Self::Sink(sink) => sink(event),
        }
    }

    /// The next queued event (never one in sink mode).
    pub(crate) fn pop(&mut self) -> Option<BotEvent> {
        match self {
            Self::Queue(queue) => queue.pop_front(),
            Self::Sink(_) => None,
        }
    }

    /// `event` for `next` to return now in queue mode; in sink mode it goes to the sink and `next` returns nothing.
    pub(crate) fn hand_over(&mut self, event: BotEvent) -> Option<BotEvent> {
        match self {
            Self::Queue(_) => Some(event),
            Self::Sink(sink) => {
                sink(event);
                None
            }
        }
    }

    pub(crate) fn is_sink(&self) -> bool {
        matches!(self, Self::Sink(_))
    }

    /// Drops queued events failing `keep`; a sink's events have already left.
    pub(crate) fn retain(&mut self, keep: impl FnMut(&BotEvent) -> bool) {
        if let Self::Queue(queue) = self {
            queue.retain(keep);
        }
    }
}

impl Extend<BotEvent> for EventOut {
    fn extend<I: IntoIterator<Item = BotEvent>>(&mut self, events: I) {
        events.into_iter().for_each(|e| self.push(e));
    }
}
