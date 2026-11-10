//! The immediate lane: requests another thread or task hands a bot while an action may be running. The bot handles
//! them between packets, inside whatever it is awaiting (an action or [`Bot::next`]), in the order they were sent.
//! Only requests that cannot disturb an action belong here; everything else waits for the bot itself.
//! [`acacia_client::ClientSender`] ([`Bot::sender`]) sends chat without even that hop.

use acacia_client::ClientSender;
use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};

use crate::movement::Key;
use crate::snapshot::BotSnapshot;
use crate::{Bot, Events};

/// One request for [`Inbox::send`].
#[derive(Debug)]
#[non_exhaustive]
pub enum Immediate {
    Chat(String),
    /// The leading `/` is optional.
    Command(String),
    /// An already-encoded packet (header + body).
    Send(Bytes),
    /// [`Bot::set_events`].
    SetEvents(Events),
    /// [`crate::CancelHandle::cancel`].
    Cancel,
    /// [`Bot::snapshot`], built when the request is handled.
    Snapshot(oneshot::Sender<BotSnapshot>),
    /// [`crate::movement::Controls::set_key`] on a physics bot. An action that drives the bot overrides the keys
    /// while it runs, and [`Bot::abandon_action`] releases them.
    Key(Key, bool),
    /// Releases every movement key.
    ReleaseKeys,
}

/// Sends a bot [`Immediate`] requests from any thread (see [`crate::inbox`]). Cheap to clone.
#[derive(Debug, Clone)]
pub struct Inbox(mpsc::UnboundedSender<Immediate>);

impl Inbox {
    /// False once the bot is gone.
    pub fn send(&self, request: Immediate) -> bool {
        self.0.send(request).is_ok()
    }
}

/// The bot's end; holding a sender too keeps `recv` from ever ending.
pub(crate) struct InboxState {
    tx: mpsc::UnboundedSender<Immediate>,
    pub(crate) rx: mpsc::UnboundedReceiver<Immediate>,
}

impl Default for InboxState {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self { tx, rx }
    }
}

impl Bot {
    /// A handle other threads use to reach this bot mid-action (see [`crate::inbox`]).
    pub fn inbox(&self) -> Inbox {
        Inbox(self.inbox.tx.clone())
    }

    /// Sends on this Bedrock bot's connection from any thread, without the bot; `None` on Java.
    pub fn sender(&self) -> Option<ClientSender> {
        Some(self.backend.bedrock()?.client.sender())
    }

    /// The event groups the bot emits now.
    pub fn events(&self) -> Events {
        self.events.events()
    }

    /// Changes the event groups the bot emits, from the next packet on; the connection's packet filter follows.
    pub fn set_events(&mut self, events: Events) {
        self.events.set(events);
        if let Some(b) = self.backend.bedrock() {
            b.client.set_filter(self.events.filter());
        }
    }

    /// Handles every queued request.
    pub(crate) fn drain_inbox(&mut self) {
        while let Ok(request) = self.inbox.rx.try_recv() {
            self.on_immediate(request);
        }
    }

    pub(crate) fn on_immediate(&mut self, request: Immediate) {
        match request {
            Immediate::Chat(message) => _ = self.chat(&message),
            Immediate::Command(command) => _ = self.command(&command),
            Immediate::Send(packet) => _ = self.backend.bedrock().map(|b| b.client.send_raw(packet)),
            Immediate::SetEvents(events) => self.set_events(events),
            Immediate::Cancel => self.cancel_handle().cancel(),
            Immediate::Snapshot(reply) => _ = reply.send(self.snapshot()),
            Immediate::Key(key, down) => {
                if let Some(c) = self.controls() {
                    c.set_key(key, down);
                }
            }
            Immediate::ReleaseKeys => self.release_keys(),
        }
    }
}
