//! The one task that owns the daemon's state: connections, bot ownership and open calls. Connection tasks hand it
//! frames; it calls the host and splits the host's batches into each connection's frames.

mod requests;
mod stream;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use acacia_api::{BotId, BotParams, CallId, CancelCallParams, CancelParams, Command, Method};
use acacia_host::{Host, Outbound};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;

use crate::mirror::Mirror;
use crate::rpc;

pub(crate) type ConnId = u64;

pub(crate) enum ToRouter {
    Open { conn: ConnId, tx: mpsc::Sender<String> },
    Text { conn: ConnId, text: String },
    Closed { conn: ConnId },
}

struct Conn {
    /// Frames to the socket; dropping it closes the connection once the queued frames are written.
    tx: mpsc::Sender<String>,
    hello: bool,
    mirror: bool,
    /// The bots this connection created or attached to, with what it was last sent of their state.
    subs: HashMap<BotId, Mirror>,
    /// Numeric request ids still open, to the host's call ids (`cancel` names the request id).
    calls: HashMap<u64, CallId>,
    /// Records for the next `out` notification.
    pending: Vec<String>,
}

struct BotEntry {
    username: String,
    persist: bool,
    subs: HashSet<ConnId>,
}

enum Kind {
    Plain,
    CreateBot { username: String, persist: bool },
    Attach { bot: BotId, username: String, persist: bool },
}

struct Call {
    conn: ConnId,
    /// `None` for a client notification: nothing is answered.
    id: Option<Value>,
    bot: Option<BotId>,
    kind: Kind,
}

pub(crate) struct Router {
    host: Arc<Host>,
    token: Option<String>,
    conns: HashMap<ConnId, Conn>,
    bots: BTreeMap<BotId, BotEntry>,
    calls: HashMap<CallId, Call>,
    next_call: u64,
    /// Counts batches and mirror ticks, so each looks at a bot's state once.
    pass: u64,
    /// Connections to drop once the current step is done (too slow, or refused).
    doomed: Vec<ConnId>,
}

impl Router {
    pub(crate) fn new(host: Arc<Host>, token: Option<String>) -> Self {
        let (conns, bots, calls) = (HashMap::new(), BTreeMap::new(), HashMap::new());
        Self { host, token, conns, bots, calls, next_call: 0, pass: 0, doomed: Vec::new() }
    }

    /// Runs until the host's stream ends (it has shut down); the connections close with it.
    pub(crate) async fn run(mut self, mut outbound: Outbound, mut rx: mpsc::UnboundedReceiver<ToRouter>, mirror_every: Duration) {
        let mut ticker = tokio::time::interval(mirror_every);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                batch = outbound.recv() => match batch {
                    Some(batch) => self.route(&batch),
                    None => break,
                },
                Some(msg) = rx.recv() => self.on_msg(msg),
                _ = ticker.tick() => self.mirror_all(),
            }
            while let Some(conn) = self.doomed.pop() {
                self.drop_conn(conn);
            }
        }
    }

    fn on_msg(&mut self, msg: ToRouter) {
        match msg {
            ToRouter::Open { conn, tx } => {
                let state = Conn { tx, hello: false, mirror: false, subs: HashMap::new(), calls: HashMap::new(), pending: Vec::new() };
                self.conns.insert(conn, state);
            }
            ToRouter::Text { conn, text } => self.on_text(conn, &text),
            ToRouter::Closed { conn } => self.drop_conn(conn),
        }
    }

    /// Queues a frame; a connection whose queue is full is too slow to keep and is dropped.
    fn send(&mut self, conn: ConnId, frame: String) {
        let Some(c) = self.conns.get(&conn) else { return };
        if c.tx.try_send(frame).is_err() {
            self.doomed.push(conn);
        }
    }

    /// Sends the connection's pending records as one `out` notification.
    fn flush(&mut self, conn: ConnId) {
        let Some(c) = self.conns.get_mut(&conn) else { return };
        if c.pending.is_empty() {
            return;
        }
        let frame = rpc::out(&std::mem::take(&mut c.pending));
        self.send(conn, frame);
    }

    /// Queues a `mirror` record if the bot's state changed since the connection last saw it (once per pass).
    fn look(&mut self, conn: ConnId, bot: &BotId) {
        let Some(c) = self.conns.get_mut(&conn) else { return };
        let Some(mirror) = c.subs.get_mut(bot).filter(|m| c.mirror && m.looked != self.pass) else { return };
        mirror.looked = self.pass;
        if let Some(record) = mirror.update(&self.host, bot) {
            c.pending.push(record);
        }
    }

    fn mirror_all(&mut self) {
        self.pass += 1;
        let pairs: Vec<(ConnId, BotId)> =
            self.conns.iter().filter(|(_, c)| c.mirror).flat_map(|(id, c)| c.subs.keys().map(|b| (*id, b.clone()))).collect();
        for (conn, bot) in &pairs {
            self.look(*conn, bot);
        }
        let conns: Vec<ConnId> = self.conns.keys().copied().collect();
        for conn in conns {
            self.flush(conn);
        }
    }

    fn subscribed(&self, conn: ConnId, bot: &BotId) -> bool {
        self.conns.get(&conn).is_some_and(|c| c.subs.contains_key(bot))
    }

    fn subscribe(&mut self, conn: ConnId, bot: &BotId) {
        if let Some(entry) = self.bots.get_mut(bot) {
            entry.subs.insert(conn);
        }
        if let Some(c) = self.conns.get_mut(&conn) {
            c.subs.entry(bot.clone()).or_default();
        }
    }

    fn fresh_call(&mut self) -> CallId {
        self.next_call += 1;
        CallId(self.next_call)
    }

    /// A call of the daemon's own; its `done` matches no open call and is ignored.
    fn internal(&mut self, method: Method) {
        let call = self.fresh_call();
        let _ = self.host.submit(Command { call, method });
    }

    /// The connection is gone: its bots close unless persisted, and its calls on persisted bots are cancelled.
    fn drop_conn(&mut self, conn: ConnId) {
        let Some(c) = self.conns.remove(&conn) else { return };
        for bot in c.subs.keys() {
            let Some(entry) = self.bots.get_mut(bot) else { continue };
            entry.subs.remove(&conn);
            if !entry.persist {
                self.internal(Method::Close(BotParams { bot: bot.clone() }));
            }
        }
        let open: Vec<(CallId, BotId)> =
            self.calls.iter().filter(|(_, call)| call.conn == conn).filter_map(|(id, call)| Some((*id, call.bot.clone()?))).collect();
        for (call, bot) in open {
            if self.bots.get(&bot).is_some_and(|e| e.persist) {
                self.internal(Method::Cancel(CancelParams { bot, call }));
            }
        }
        let connecting: Vec<CallId> = self
            .calls
            .iter()
            .filter(|(_, c)| c.conn == conn && matches!(c.kind, Kind::CreateBot { persist: false, .. }))
            .map(|(id, _)| *id)
            .collect();
        for call in connecting {
            self.internal(Method::CancelCall(CancelCallParams { call }));
        }
    }
}
