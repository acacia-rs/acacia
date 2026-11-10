//! Everything a bot's task does without the bot itself, so it can run beside an action holding `&mut Bot`.

use std::collections::VecDeque;
use std::sync::Arc;

use acacia_api::{CallId, ErrorCode, Event, Lane, Method, Reply};
use acacia_bot::client::ClientSender;
use acacia_bot::events::EventReceiver;
use acacia_bot::snapshot::{BotSnapshot, SnapshotReader};
use acacia_bot::{BotEvent, Immediate, Inbox};
use tokio::sync::{mpsc, oneshot};

use crate::convert;
use crate::registry::{BotOut, Msg};

const NO_SNAPSHOTS: &str = "the host publishes no snapshots (HostConfig::snapshots)";

pub(super) struct Lanes {
    pub out: Arc<BotOut>,
    pub rx: mpsc::UnboundedReceiver<Msg>,
    pub events: EventReceiver,
    /// Bedrock's sender, which needs no bot; Java chat goes through the inbox.
    pub sender: Option<ClientSender>,
    pub inbox: Inbox,
    /// The bot's published state, for reads that need no bot (`None` with snapshots off).
    pub reader: Option<SnapshotReader>,
    /// Queued calls not started yet, in order.
    pub queue: VecDeque<(CallId, Method)>,
    /// Snapshot calls the bot has yet to build, in the order the inbox hands them over.
    pub snapshots: VecDeque<(CallId, oneshot::Receiver<BotSnapshot>)>,
    pub closing: bool,
    /// `bot.close` calls, answered once the bot has ended.
    pub closers: Vec<CallId>,
    /// The `end` event went out.
    pub ended: bool,
}

pub(super) enum Wake {
    Msg(Option<Msg>),
    Event(BotEvent),
    Snapshot(CallId, Option<BotSnapshot>),
}

impl Lanes {
    /// The next thing that needs the task: a call, an event, a built snapshot.
    pub(super) async fn wake(&mut self) -> Wake {
        let snapshots = &mut self.snapshots;
        tokio::select! {
            msg = self.rx.recv() => Wake::Msg(msg),
            Some(event) = self.events.recv() => Wake::Event(event),
            built = async { (&mut snapshots.front_mut().expect("a snapshot is pending").1).await }, if !snapshots.is_empty() => {
                let (call, _) = snapshots.pop_front().expect("a snapshot is pending");
                Wake::Snapshot(call, built.ok())
            }
        }
    }

    /// Handles a wake-up; true if the running call (`running`) must be cancelled.
    pub(super) fn on_wake(&mut self, wake: Wake, running: Option<CallId>) -> bool {
        match wake {
            Wake::Msg(Some(Msg::Call(call, method))) => self.take(call, method, running),
            Wake::Msg(Some(Msg::Shutdown) | None) => {
                self.closing = true;
                true
            }
            Wake::Event(event) => {
                self.forward(event);
                false
            }
            Wake::Snapshot(call, built) => {
                // Events the bot raised before it built the snapshot go first.
                self.drain_events();
                match built {
                    Some(s) => self.out.done(call, Ok(Reply::Snapshot(convert::snapshot(&s)))),
                    None => self.out.fail(call, ErrorCode::Disconnected, "the bot ended before the snapshot"),
                }
                false
            }
        }
    }

    fn take(&mut self, call: CallId, method: Method, running: Option<CallId>) -> bool {
        if method.lane() == Lane::Queued {
            self.queue.push_back((call, method));
            return false;
        }
        match method {
            Method::Chat(p) => {
                let sent = match &self.sender {
                    Some(sender) => sender.chat(p.message),
                    None => self.inbox.send(Immediate::Chat(p.message)),
                };
                self.sent(call, sent, Reply::Chat(()));
            }
            Method::Command(p) => {
                let sent = match &self.sender {
                    Some(sender) => sender.command(&p.command),
                    None => self.inbox.send(Immediate::Command(p.command)),
                };
                self.sent(call, sent, Reply::Command(()));
            }
            Method::SetEvents(p) => {
                let sent = self.inbox.send(Immediate::SetEvents(convert::events(&p.events)));
                self.sent(call, sent, Reply::SetEvents(()));
            }
            Method::SetControlState(p) => {
                let sent = self.inbox.send(Immediate::Key(convert::key(p.control), p.state));
                self.sent(call, sent, Reply::SetControlState(()));
            }
            Method::ClearControlStates(_) => {
                let sent = self.inbox.send(Immediate::ReleaseKeys);
                self.sent(call, sent, Reply::ClearControlStates(()));
            }
            Method::FindBlocks(p) => match &self.reader {
                Some(reader) => self.out.done(call, convert::find_blocks(reader, &p).map(Reply::FindBlocks)),
                None => self.out.fail(call, ErrorCode::NotPossible, NO_SNAPSHOTS),
            },
            Method::BlockAtCursor(p) => match &self.reader {
                Some(reader) => self.out.done(call, convert::block_at_cursor(reader, &p).map(Reply::BlockAtCursor)),
                None => self.out.fail(call, ErrorCode::NotPossible, NO_SNAPSHOTS),
            },
            Method::Snapshot(_) => {
                let (tx, rx) = oneshot::channel();
                if self.inbox.send(Immediate::Snapshot(tx)) {
                    self.snapshots.push_back((call, rx));
                } else {
                    self.out.fail(call, ErrorCode::Disconnected, "the bot has ended");
                }
            }
            Method::Cancel(p) => {
                self.out.done(call, Ok(Reply::Cancel(())));
                if running == Some(p.call) {
                    return true;
                }
                if let Some(i) = self.queue.iter().position(|(c, _)| *c == p.call) {
                    self.queue.remove(i);
                    self.out.fail(p.call, ErrorCode::Cancelled, "cancelled before it ran");
                }
            }
            Method::Close(_) => {
                self.closing = true;
                self.closers.push(call);
                return true;
            }
            other => self.out.fail(call, ErrorCode::Internal, &format!("{} is not a bot method", other.name())),
        }
        false
    }

    fn sent(&self, call: CallId, sent: bool, reply: Reply) {
        match sent {
            true => self.out.done(call, Ok(reply)),
            false => self.out.fail(call, ErrorCode::Disconnected, "the connection has ended"),
        }
    }

    pub(super) fn forward(&mut self, event: BotEvent) {
        if let Some(event) = convert::event(event) {
            self.ended |= matches!(event, Event::End { .. });
            self.out.event(event);
        }
    }

    pub(super) fn drain_events(&mut self) {
        while let Some(event) = self.events.try_recv() {
            self.forward(event);
        }
    }

    /// The bot has ended: fails every call still owed but `bot.close` ones.
    pub(super) fn fail_open(&mut self) {
        for (call, _) in self.queue.drain(..) {
            self.out.fail(call, ErrorCode::Disconnected, "the bot disconnected before the call ran");
        }
        for (call, _) in self.snapshots.drain(..) {
            self.out.fail(call, ErrorCode::Disconnected, "the bot ended before the snapshot");
        }
    }
}
