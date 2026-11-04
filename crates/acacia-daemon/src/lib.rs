//! Serves an [`acacia_host::Host`] over WebSocket: JSON-RPC 2.0 in text frames, the same acacia-api schema the napi
//! addon moves, plus a handshake and bot ownership. The one place the socket protocol is written down (values,
//! methods and ordering: acacia-api's crate docs; the host: acacia-host's; the TS client: `@acacia-rs/bot`'s README):
//!
//! - **Handshake.** The first request is `hello {protocolVersion, client?, token?, mirror?}`. It answers
//!   `{protocolVersion, server, methods, reads: false, mirror}`; `methods` is a hash of the method table (equal
//!   hashes, same methods). A different `protocolVersion` fails `VERSION_MISMATCH`; with `--token`, a wrong one fails
//!   `UNAUTHORIZED` and the daemon closes the socket. Any other request first fails `UNAUTHORIZED`. Without a token,
//!   handshakes carrying an `Origin` header (browsers) are refused, so a web page cannot drive local bots.
//! - **Calls.** A request's `method` and `params` are an acacia-api method (`createBot`, `bot.goto`, ...). Its `id`
//!   (string or number) is the client's; the daemon renumbers calls for the host. The response is the call's
//!   `done`: `result` is the method's result, `error` is `{code, message, data: {code}}`, where `data.code` is the
//!   acacia-api `ErrorCode` string to branch on and `code` its JSON-RPC number (`rpc::rpc_code`: -32001..-32005
//!   action errors, -32010 `CANCELLED`, -32020.. host, -32030.. connect, -32040 `VERSION_MISMATCH`, -32041
//!   `UNAUTHORIZED`, -32601 `UNKNOWN_METHOD`, -32602 `BAD_PARAMS`, -32603 `INTERNAL`, -32700/-32600 for frames that
//!   are not a request). A refused call (`BUSY`, `UNKNOWN_BOT`, ...) is answered at once. A request without `id` (a
//!   notification) runs and is not answered. Batches (arrays) are refused. `cancel {bot, call}` names the request
//!   `id` of the call to cancel (numeric ids only).
//! - **Events.** The daemon sends `out` notifications, `{"method": "out", "params": {"records": [...]}}`, holding
//!   acacia-api `Out` records (`event`, `lagged`) and `mirror` records, at most one per host batch and connection.
//!   A response comes after the `out` holding the events raised before its call ended, so the host's ordering
//!   (events before `done`) holds across the socket. `seq` numbers each bot's events from 1; a client attaching to
//!   a running bot sees it mid-count. An `msaCode` record (a sign-in waiting on a person) goes to the connection
//!   whose `createBot` or `signIn` it belongs to, its `call` set to that request's `id`; a notification gets none.
//! - **Sign-in.** Tokens are cached in the daemon's `--profiles-folder` (default acacia-host's
//!   `default_profiles_folder` of the daemon's user), per `username`: any client with the token can play an account
//!   someone signed in there. Params naming `profilesFolder` are refused `BAD_PARAMS`, so clients cannot pick
//!   where token files are written.
//! - **Ownership.** A bot belongs to the connections that created or attached to it: only they can call it or get
//!   its events, and `bot.close` from any of them closes it. When a connection drops, its bots close, and its open
//!   calls on persisted bots are cancelled. `createBot {..., persist: true}` keeps the bot running with no
//!   connection; `attach {bot | username}` subscribes a connection to a persisted bot (or its own) and answers
//!   `{bot, username, persist, snapshot}` with a fresh `bot.snapshot`. Events raised while no connection is
//!   attached are dropped. `bots` lists every bot: `{bots: [{bot, username, persist, attached, mine}]}`.
//! - **Reads.** Sync reads cannot cross a socket (`reads: false`). With `hello {mirror: true}` the daemon pushes
//!   each attached bot's published state instead: a `mirror` record `{bot, hot?, cold?, entities?, items?}` carries
//!   the parts that changed (`HotSnapshot` from the bot's first tick, `ColdSnapshot`, an entity delta, `ItemTable`,
//!   the last once), sent at
//!   most every [`DaemonConfig::mirror_every`] and before any event or response of that bot, so a client's copy is
//!   at least as new as the record it is handling. Terrain (`blockAt`, `findBlocks`) is not mirrored; call
//!   `bot.findBlocks`.
//! - **Entity deltas.** `entities` is `{version, full?, removed?, added?, numbers?}`: what changed since the last
//!   one this connection got. Every entity has a slot, a small integer it keeps while listed and that is reused
//!   after it left. Apply in order: with `full: true`, drop every entity not re-added in `added` (same slot and
//!   id); drop `removed` (`[slot, x, y, z, vx, vy, vz, yaw, pitch, health | null]`, its last numbers); add `added`
//!   (`[slot, id, uniqueId, kind, username | null]`); then write `numbers` (rows as `removed`) for every added or
//!   moved entity. Positions are feet. The first record of a bot on a connection (a new mirror, an `attach`) is
//!   full, and so is one after the connection fell behind the bot's change log (acacia-bot `snapshot::delta`).
//! - **Backpressure.** Each connection queues [`DaemonConfig::queue`] frames; a client that falls that far behind
//!   is disconnected (its bots close as above). The host's own `lagged` records go to every connection.
//!
//! Not yet: MessagePack framing, TLS (put a proxy in front), reconnect resumption beyond `attach`, an MCP endpoint
//! (`@acacia-rs/mcp` can run over the TS client's `WsTransport` instead).

mod conn;
mod mirror;
mod router;
mod rpc;

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use acacia_host::{Host, HostConfig};
use tokio::net::{TcpListener, ToSocketAddrs};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::router::Router;

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub host: HostConfig,
    /// The shared secret `hello` must carry (default none).
    pub token: Option<String>,
    /// How often mirrored state is pushed when the bot raises nothing (default 50 ms, a tick).
    pub mirror_every: Duration,
    /// Frames a connection may have unsent before it is dropped as too slow (default 4096).
    pub queue: usize,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self { host: HostConfig::default(), token: None, mirror_every: Duration::from_millis(50), queue: 4096 }
    }
}

/// A running daemon: a host, its listener and the router between them.
pub struct Daemon {
    addr: SocketAddr,
    host: Arc<Host>,
    router: JoinHandle<()>,
    accept: JoinHandle<()>,
}

impl Daemon {
    /// Starts the host and listens on `addr` (port 0 picks one: see [`Daemon::local_addr`]).
    pub async fn bind(addr: impl ToSocketAddrs, config: DaemonConfig) -> io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        let (host, outbound) = Host::start(config.host)?;
        let host = Arc::new(host);
        let (tx, rx) = mpsc::unbounded_channel();
        let has_token = config.token.is_some();
        let router = Router::new(host.clone(), config.token);
        let router = tokio::spawn(router.run(outbound, rx, config.mirror_every));
        let accept = tokio::spawn(conn::accept(listener, tx, config.queue.max(1), has_token));
        Ok(Self { addr, host, router, accept })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Stops accepting, closes every bot (as the host's shutdown does) and closes the connections.
    pub async fn shutdown(self) {
        self.accept.abort();
        self.host.shutdown().await;
        let _ = self.router.await;
    }
}
