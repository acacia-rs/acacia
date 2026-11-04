//! Splits one host batch into each connection's frames: events and `lagged` into one `out` notification per
//! connection, each `done` into a response sent after the records before it.

use std::borrow::Cow;
use std::collections::HashSet;

use acacia_api::{ApiError, BotId, BotParams, CallId, Method};
use serde::Deserialize;
use serde_json::value::RawValue;
use serde_json::{json, Value};

use super::{BotEntry, ConnId, Kind, Router};
use crate::rpc;

/// What the router needs of a record; the rest is passed on as it came.
#[derive(Deserialize)]
struct Peek<'a> {
    #[serde(rename = "type", borrow)]
    kind: Cow<'a, str>,
    #[serde(default, borrow)]
    bot: Option<Cow<'a, str>>,
    #[serde(default)]
    call: Option<u64>,
    #[serde(default, borrow)]
    event: Option<EventPeek<'a>>,
    #[serde(default)]
    result: Option<Box<RawValue>>,
}

#[derive(Deserialize)]
struct EventPeek<'a> {
    #[serde(rename = "type", borrow)]
    kind: Cow<'a, str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Done {
    Ok(Box<RawValue>),
    Err(ApiError),
}

impl Router {
    pub(super) fn route(&mut self, batch: &str) {
        self.pass += 1;
        let records: Vec<&RawValue> = serde_json::from_str(batch).expect("the host sends JSON arrays");
        let mut touched = HashSet::new();
        for raw in records {
            let peek: Peek = serde_json::from_str(raw.get()).expect("the host sends records");
            let bot = peek.bot.map(|b| BotId(b.into_owned()));
            match (peek.kind.as_ref(), bot) {
                ("event", Some(bot)) => {
                    let end = peek.event.is_some_and(|e| e.kind == "end");
                    self.event(&bot, raw.get(), &mut touched, end);
                }
                ("done", bot) => {
                    let result = peek.result.and_then(|r| serde_json::from_str(r.get()).ok());
                    if let (Some(call), Some(result)) = (peek.call, result) {
                        self.done(CallId(call), bot, result);
                    }
                }
                ("msaCode", _) => {
                    if let Some(call) = peek.call {
                        self.msa_code(CallId(call), raw.get(), &mut touched);
                    }
                }
                ("lagged", _) => {
                    for (id, c) in self.conns.iter_mut().filter(|(_, c)| c.hello) {
                        c.pending.push(raw.get().to_owned());
                        touched.insert(*id);
                    }
                }
                _ => {}
            }
        }
        for conn in touched {
            self.flush(conn);
        }
    }

    fn event(&mut self, bot: &BotId, raw: &str, touched: &mut HashSet<ConnId>, end: bool) {
        let subs: Vec<ConnId> = self.bots.get(bot).map(|e| e.subs.iter().copied().collect()).unwrap_or_default();
        for conn in subs {
            self.look(conn, bot);
            if let Some(c) = self.conns.get_mut(&conn) {
                c.pending.push(raw.to_owned());
                touched.insert(conn);
            }
        }
        if end && let Some(entry) = self.bots.remove(bot) {
            for conn in entry.subs {
                if let Some(c) = self.conns.get_mut(&conn) {
                    c.subs.remove(bot);
                }
            }
        }
    }

    /// A sign-in prompt of an open call: to that call's connection, under the client's request id.
    fn msa_code(&mut self, call: CallId, raw: &str, touched: &mut HashSet<ConnId>) {
        let Some((conn, Some(id))) = self.calls.get(&call).map(|c| (c.conn, c.id.clone())) else { return };
        let Ok(mut record) = serde_json::from_str::<Value>(raw) else { return };
        record["call"] = id;
        if let Some(c) = self.conns.get_mut(&conn) {
            c.pending.push(record.to_string());
            touched.insert(conn);
        }
    }

    fn done(&mut self, call: CallId, bot: Option<BotId>, result: Done) {
        let Some(open) = self.calls.remove(&call) else { return };
        let conn = open.conn;
        let alive = self.conns.contains_key(&conn);
        if let (Some(n), Some(c)) = (open.id.as_ref().and_then(Value::as_u64), self.conns.get_mut(&conn)) {
            c.calls.remove(&n);
        }
        let answer = match (open.kind, result) {
            (_, Done::Err(e)) => Err(e),
            (Kind::Plain, Done::Ok(raw)) => Ok(raw.get().to_owned()),
            (Kind::CreateBot { username, persist }, Done::Ok(raw)) => {
                if let Ok(created) = serde_json::from_str::<acacia_api::CreateBotResult>(raw.get()) {
                    self.created(conn, alive, created.bot, username, persist);
                }
                Ok(raw.get().to_owned())
            }
            (Kind::Attach { bot, username, persist }, Done::Ok(raw)) => {
                Ok(format!(r#"{{"bot":{},"username":{},"persist":{persist},"snapshot":{}}}"#, json!(bot), json!(username), raw.get()))
            }
        };
        let (Some(id), true) = (open.id, alive) else { return };
        if let Some(bot) = bot.or(open.bot) {
            self.look(conn, &bot);
        }
        self.flush(conn);
        let frame = match answer {
            Ok(result) => rpc::result(&id, &result),
            Err(e) => rpc::error(&id, &e),
        };
        self.send(conn, frame);
    }

    /// A `createBot` succeeded: the bot belongs to its connection, or closes at once if that is gone (unless kept).
    fn created(&mut self, conn: ConnId, alive: bool, bot: BotId, username: String, persist: bool) {
        if !alive && !persist {
            return self.internal(Method::Close(BotParams { bot }));
        }
        self.bots.insert(bot.clone(), BotEntry { username, persist, subs: HashSet::new() });
        if alive {
            self.subscribe(conn, &bot);
        }
    }
}
