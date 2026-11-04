//! Requests from a connection: `hello`, the daemon's own methods and acacia-api's (contract: the crate docs).

use acacia_api::{ApiError, BotId, BotParams, CancelCallParams, CancelParams, ErrorCode, Method, METHODS, PROTOCOL_VERSION};
use serde::Deserialize;
use serde_json::{json, Value};
use subtle::ConstantTimeEq;

use super::{Call, ConnId, Kind, Router};
use crate::rpc::{self, Request};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Hello {
    protocol_version: u32,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    mirror: bool,
}

#[derive(Deserialize)]
struct Attach {
    #[serde(default)]
    bot: Option<BotId>,
    #[serde(default)]
    username: Option<String>,
}

/// `Ok(Some(result))` answers now, `Ok(None)` when the call's `done` comes.
type Outcome = Result<Option<String>, ApiError>;

fn bad(e: impl std::fmt::Display) -> ApiError {
    ApiError::new(ErrorCode::BadParams, e.to_string())
}

impl Router {
    pub(super) fn on_text(&mut self, conn: ConnId, text: &str) {
        let request = match Request::parse(text) {
            Ok(request) => request,
            Err(malformed) => return self.send(conn, rpc::malformed(&malformed)),
        };
        let greeted = self.conns.get(&conn).is_some_and(|c| c.hello);
        let outcome = match request.method.as_str() {
            "hello" => self.hello(conn, &request.params),
            _ if !greeted => Err(ApiError::new(ErrorCode::Unauthorized, "send hello first")),
            "bots" => Ok(Some(self.list(conn))),
            "attach" => self.attach(conn, &request),
            _ => self.host_call(conn, &request),
        };
        let refused = matches!(&outcome, Err(e) if e.code == ErrorCode::Unauthorized && request.method == "hello");
        if let Some(id) = &request.id {
            match outcome {
                Ok(Some(result)) => self.send(conn, rpc::result(id, &result)),
                Ok(None) => {}
                Err(e) => self.send(conn, rpc::error(id, &e)),
            }
        }
        if refused {
            self.doomed.push(conn);
        }
    }

    fn hello(&mut self, conn: ConnId, params: &Value) -> Outcome {
        let hello: Hello = serde_json::from_value(params.clone()).map_err(bad)?;
        if self.token.as_ref().is_some_and(|t| !hello.token.as_ref().is_some_and(|h| token_matches(h, t))) {
            return Err(ApiError::new(ErrorCode::Unauthorized, "wrong or missing token"));
        }
        if hello.protocol_version != PROTOCOL_VERSION {
            let message = format!("the daemon speaks protocol {PROTOCOL_VERSION}, the client {}", hello.protocol_version);
            return Err(ApiError::new(ErrorCode::VersionMismatch, message));
        }
        if let Some(c) = self.conns.get_mut(&conn) {
            (c.hello, c.mirror) = (true, hello.mirror);
        }
        let server = concat!("acacia-daemon ", env!("CARGO_PKG_VERSION"));
        let info = json!({ "protocolVersion": PROTOCOL_VERSION, "server": server, "methods": methods_hash(), "reads": false, "mirror": hello.mirror });
        Ok(Some(info.to_string()))
    }

    fn list(&self, conn: ConnId) -> String {
        let bots: Vec<Value> = self
            .bots
            .iter()
            .map(|(id, e)| json!({ "bot": id, "username": e.username, "persist": e.persist, "attached": e.subs.len(), "mine": e.subs.contains(&conn) }))
            .collect();
        json!({ "bots": bots }).to_string()
    }

    fn attach(&mut self, conn: ConnId, request: &Request) -> Outcome {
        let p: Attach = serde_json::from_value(request.params.clone()).map_err(bad)?;
        if p.bot.is_none() && p.username.is_none() {
            return Err(bad("attach names a bot or a username"));
        }
        let found = self.bots.iter().find(|(id, e)| {
            (e.persist || e.subs.contains(&conn)) && (p.bot.as_ref() == Some(*id) || p.username.as_deref() == Some(e.username.as_str()))
        });
        let (bot, username, persist) = found
            .map(|(id, e)| (id.clone(), e.username.clone(), e.persist))
            .ok_or_else(|| ApiError::new(ErrorCode::UnknownBot, "no persisted bot by that id or username"))?;
        self.subscribe(conn, &bot);
        let kind = Kind::Attach { bot: bot.clone(), username, persist };
        self.submit(conn, request.id.clone(), kind, Method::Snapshot(BotParams { bot }))?;
        Ok(None)
    }

    fn host_call(&mut self, conn: ConnId, request: &Request) -> Outcome {
        let mut params = request.params.clone();
        if params.get("profilesFolder").is_some() {
            return Err(bad("profilesFolder is the daemon's (--profiles-folder): a client may not pick where tokens are written"));
        }
        let persist = match params.as_object_mut() {
            Some(p) if request.method == "createBot" => p.remove("persist").and_then(|v| v.as_bool()).unwrap_or(false),
            _ => false,
        };
        let mut method = Method::parse(&request.method, params)?;
        if let Some(bot) = method.bot().filter(|b| !self.subscribed(conn, b)) {
            return Err(ApiError::new(ErrorCode::UnknownBot, format!("no bot {bot} on this connection")));
        }
        if let Method::Cancel(CancelParams { call, .. }) | Method::CancelCall(CancelCallParams { call }) = &mut method {
            let known = self.conns.get(&conn).and_then(|c| c.calls.get(&call.0).copied());
            *call = known.unwrap_or_else(|| self.fresh_call());
        }
        let kind = match &method {
            Method::CreateBot(p) => Kind::CreateBot { username: p.username.clone(), persist },
            _ => Kind::Plain,
        };
        self.submit(conn, request.id.clone(), kind, method)?;
        Ok(None)
    }

    /// Hands a call to the host under a call id of the daemon's, remembered until its `done`.
    fn submit(&mut self, conn: ConnId, id: Option<Value>, kind: Kind, method: Method) -> Result<(), ApiError> {
        let call = self.fresh_call();
        let bot = method.bot().cloned();
        self.host.submit(acacia_api::Command { call, method })?;
        if let (Some(n), Some(c)) = (id.as_ref().and_then(Value::as_u64), self.conns.get_mut(&conn)) {
            c.calls.insert(n, call);
        }
        self.calls.insert(call, Call { conn, id, bot, kind });
        Ok(())
    }
}

/// Constant time in the contents, so response timing cannot leak the token a prefix at a time (only its length).
fn token_matches(given: &str, token: &str) -> bool {
    given.as_bytes().ct_eq(token.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::token_matches;

    #[test]
    fn tokens_match_only_when_equal() {
        assert!(token_matches("s3cret", "s3cret"));
        assert!(!token_matches("s3creT", "s3cret"));
        assert!(!token_matches("s3cre", "s3cret"));
        assert!(!token_matches("", "s3cret"));
    }
}

/// FNV-1a over the method table's names and lanes: equal hashes mean the same methods.
fn methods_hash() -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for m in METHODS {
        for byte in format!("{}:{:?};", m.name, m.lane).bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}
