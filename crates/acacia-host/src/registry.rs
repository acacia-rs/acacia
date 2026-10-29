//! The host's bots: where each runs, its call channel, and what it still owes the stream.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use acacia_api::{ApiError, BotId, CallId, CallResult, EndReason, ErrorCode, Event, Method, Out, Reply};
use acacia_bot::snapshot::SnapshotReader;
use parking_lot::Mutex;
use tokio::sync::{mpsc, Notify};

use crate::outbound::ShardBuf;

/// What a bot's task receives.
pub(crate) enum Msg {
    Call(CallId, Method),
    /// The host is shutting down: close like `bot.close`, with no call to answer.
    Shutdown,
}

/// The stream end of one bot, shared by its task and the host (which answers for it if the task panics).
pub(crate) struct BotOut {
    pub id: BotId,
    seq: AtomicU64,
    /// Calls accepted and not yet answered.
    open: Mutex<HashSet<CallId>>,
    buf: Arc<ShardBuf>,
}

impl BotOut {
    pub(crate) fn new(id: BotId, buf: Arc<ShardBuf>) -> Arc<Self> {
        Arc::new(Self { id, seq: AtomicU64::new(0), open: Mutex::new(HashSet::new()), buf })
    }

    pub(crate) fn event(&self, event: Event) {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        self.buf.push(Out::Event { bot: self.id.clone(), seq, event });
    }

    pub(crate) fn done(&self, call: CallId, result: Result<Reply, ApiError>) {
        self.open.lock().remove(&call);
        let result = result.map_or_else(CallResult::Err, |r| CallResult::ok(&r));
        self.buf.push(Out::Done { bot: self.id.clone(), call, result });
    }

    pub(crate) fn fail(&self, call: CallId, code: ErrorCode, message: &str) {
        self.done(call, Err(ApiError::new(code, message)));
    }

    fn open(&self) -> usize {
        self.open.lock().len()
    }

    /// The task died: answer everything it owed and end the bot.
    fn abandon(&self, message: &str) {
        let calls: Vec<_> = self.open.lock().drain().collect();
        self.event(Event::Error { message: message.to_owned() });
        for call in calls {
            self.fail(call, ErrorCode::Internal, message);
        }
        self.event(Event::End { reason: EndReason::Lost, message: message.to_owned() });
    }
}

struct Entry {
    tx: mpsc::UnboundedSender<Msg>,
    out: Arc<BotOut>,
    shard: usize,
    /// Set once the bot has spawned, if the host publishes snapshots.
    reader: Option<SnapshotReader>,
}

pub(crate) struct Registry {
    bots: HashMap<BotId, Entry>,
    /// Live bots per shard.
    loads: Vec<usize>,
    next: u64,
    closing: bool,
    max_open: usize,
    emptied: Arc<Notify>,
}

impl Registry {
    pub(crate) fn new(shards: usize, max_open: usize) -> Self {
        Self { bots: HashMap::new(), loads: vec![0; shards], next: 0, closing: false, max_open, emptied: Arc::new(Notify::new()) }
    }

    /// Names a new bot on the least loaded shard and registers it with its first call open.
    pub(crate) fn add(&mut self, call: CallId, bufs: &[Arc<ShardBuf>]) -> Result<(usize, Arc<BotOut>, mpsc::UnboundedReceiver<Msg>), ApiError> {
        if self.closing {
            return Err(ApiError::new(ErrorCode::Closed, "the host is shutting down"));
        }
        let shard = (0..self.loads.len()).min_by_key(|&i| self.loads[i]).expect("at least one shard");
        self.next += 1;
        let id = BotId(format!("bot-{}", self.next));
        let out = BotOut::new(id.clone(), bufs[shard].clone());
        out.open.lock().insert(call);
        let (tx, rx) = mpsc::unbounded_channel();
        self.loads[shard] += 1;
        self.bots.insert(id, Entry { tx, out: out.clone(), shard, reader: None });
        Ok((shard, out, rx))
    }

    /// Hands a call to its bot's task.
    pub(crate) fn send(&self, bot: &BotId, call: CallId, method: Method) -> Result<(), ApiError> {
        if self.closing {
            return Err(ApiError::new(ErrorCode::Closed, "the host is shutting down"));
        }
        let entry = self.bots.get(bot).ok_or_else(|| ApiError::new(ErrorCode::UnknownBot, format!("no bot {bot}")))?;
        if entry.out.open() >= self.max_open {
            return Err(ApiError::new(ErrorCode::Busy, format!("{bot} has {} calls open", self.max_open)));
        }
        entry.out.open.lock().insert(call);
        if entry.tx.send(Msg::Call(call, method)).is_err() {
            entry.out.open.lock().remove(&call);
            return Err(ApiError::new(ErrorCode::UnknownBot, format!("{bot} has ended")));
        }
        Ok(())
    }

    pub(crate) fn set_reader(&mut self, bot: &BotId, reader: Option<SnapshotReader>) {
        if let Some(entry) = self.bots.get_mut(bot) {
            entry.reader = reader;
        }
    }

    pub(crate) fn reader(&self, bot: &BotId) -> Option<SnapshotReader> {
        self.bots.get(bot)?.reader.clone()
    }

    /// Stops taking calls and tells every bot to close; returns what fires once the last one is gone.
    pub(crate) fn close_all(&mut self) -> Option<Arc<Notify>> {
        self.closing = true;
        for entry in self.bots.values() {
            let _ = entry.tx.send(Msg::Shutdown);
        }
        (!self.bots.is_empty()).then(|| self.emptied.clone())
    }

    /// The bot's task has ended; later calls to it fail with `UNKNOWN_BOT`.
    pub(crate) fn remove(&mut self, bot: &BotId) {
        if let Some(entry) = self.bots.remove(bot) {
            self.loads[entry.shard] -= 1;
        }
        if self.closing && self.bots.is_empty() {
            self.emptied.notify_one();
        }
    }

    /// The bot's task panicked.
    pub(crate) fn panicked(&mut self, bot: &BotId, message: &str) {
        if let Some(entry) = self.bots.get(bot) {
            entry.out.abandon(&format!("the bot's task panicked: {message}"));
        }
        self.remove(bot);
    }
}
