use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use acacia_raknet::{Reliability, Server, ServerConfig, ServerEvent};
use acacia_proto::packets::{Disconnect, DisconnectContent};
use acacia_proto::types::DisconnectFailReason;
use acacia_proto::{packets, GAME_VERSION, Packet, PROTOCOL_VERSION};
use bytes::Bytes;
use serde_json::json;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::peer::{encode, Peer, Received};
use crate::script::Script;

/// How long [`FakeServer::recv`] and [`FakeServer::spawned`] wait before failing the test.
pub const WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const LOCK: &str = "fake server lock";

#[derive(Default)]
struct Shared {
    received: Vec<Received>,
    sent: Vec<Received>,
    /// When the server sent PlayStatus(PlayerSpawn), after Login.
    spawned: Option<Duration>,
}

enum Command {
    Send(Bytes),
    Kick(String),
}

/// A local RakNet server that replays a [`Script`] to the first client that joins, answers its
/// sub-chunk and blob requests, and records what it sends. Tests can also [`send`](Self::send),
/// [`recv`](Self::recv) and [`kick`](Self::kick). Stops when dropped.
pub struct FakeServer {
    addr: SocketAddr,
    shared: Arc<Mutex<Shared>>,
    commands: mpsc::UnboundedSender<Command>,
    /// Ticks whenever `shared` changes.
    changes: watch::Receiver<()>,
    /// Index into `received` after the last packet [`recv`](Self::recv) returned.
    cursor: Mutex<usize>,
    task: JoinHandle<()>,
}

impl FakeServer {
    pub async fn start(script: Script) -> io::Result<Self> {
        let socket = UdpSocket::bind("127.0.0.1:0").await?;
        let addr = socket.local_addr()?;
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (commands, commands_rx) = mpsc::unbounded_channel();
        let (changes_tx, changes) = watch::channel(());
        let task = tokio::spawn(run(socket, script, shared.clone(), commands_rx, changes_tx));
        Ok(Self { addr, shared, commands, changes, cursor: Mutex::new(0), task })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// What the client sent so far, in order.
    pub fn received(&self) -> Vec<Received> {
        self.shared.lock().expect(LOCK).received.clone()
    }

    /// Time from Login to the server's PlayStatus(PlayerSpawn).
    pub fn spawned_after_login(&self) -> Option<Duration> {
        self.shared.lock().expect(LOCK).spawned
    }

    /// Waits until the script has sent PlayStatus(PlayerSpawn).
    pub async fn spawned(&self) -> io::Result<()> {
        self.wait_for(|s| s.spawned.map(drop)).await
    }

    /// Sends `packet` to the client in its own batch.
    pub fn send<T: Packet>(&self, packet: &T) {
        let _ = self.commands.send(Command::Send(encode(packet)));
    }

    /// Disconnects the client with `message`, as a server kick.
    pub fn kick(&self, message: impl Into<String>) {
        let _ = self.commands.send(Command::Kick(message.into()));
    }

    /// The next `T` the client sent after the last packet `recv` returned (of any type): reads the
    /// client's packets in order, skipping other ids.
    pub async fn recv<T: Packet>(&self) -> io::Result<T> {
        let raw = self
            .wait_for(|s| {
                let mut cursor = self.cursor.lock().expect(LOCK);
                let i = *cursor + s.received[*cursor..].iter().position(|r| r.packet.id == T::ID)?;
                *cursor = i + 1;
                Some(s.received[i].packet.clone())
            })
            .await?;
        raw.decode().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
    }

    async fn wait_for<R>(&self, mut check: impl FnMut(&Shared) -> Option<R>) -> io::Result<R> {
        let mut changes = self.changes.clone();
        let wait = async {
            loop {
                changes.borrow_and_update();
                if let Some(r) = check(&self.shared.lock().expect(LOCK)) {
                    return Ok(r);
                }
                if changes.changed().await.is_err() {
                    return Err(io::Error::new(io::ErrorKind::BrokenPipe, "fake server stopped"));
                }
            }
        };
        tokio::time::timeout(WAIT_TIMEOUT, wait).await.unwrap_or_else(|_| Err(io::Error::new(io::ErrorKind::TimedOut, "fake server wait timed out")))
    }

    /// Both directions as an acacia-mitm capture, for `capdiff` against a vanilla capture.
    pub fn write_capture(&self, path: &std::path::Path) -> io::Result<()> {
        let shared = self.shared.lock().expect(LOCK);
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let mut all: Vec<(bool, &Received)> = shared.sent.iter().map(|r| (false, r)).chain(shared.received.iter().map(|r| (true, r))).collect();
        all.sort_by_key(|(_, r)| r.t);
        let mut lines = Vec::new();
        let mut line = |v: serde_json::Value| lines.push(v.to_string());
        let mut spawned = shared.spawned;
        for (from_client, r) in all {
            let name = acacia_proto::packet_name(r.packet.id).unwrap_or("?");
            if r.packet.id == packets::Login::ID {
                line(json!({ "dir": "C>S", "id": r.packet.id, "name": name, "t": ms(r.t) }));
                line(json!({ "event": "login", "t": ms(r.t) }));
                continue;
            }
            if let Some(s) = spawned.filter(|s| *s <= r.t) {
                line(json!({ "event": "spawned", "t": ms(s) }));
                spawned = None;
            }
            let dir = if from_client { "C>S" } else { "S>C" };
            line(json!({ "dir": dir, "id": r.packet.id, "len": r.packet.body.len(), "name": name, "raw": hex::encode(&r.packet.body), "t": ms(r.t) }));
        }
        std::fs::write(path, lines.join("\n") + "\n")
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn run(socket: UdpSocket, script: Script, shared: Arc<Mutex<Shared>>, mut commands: mpsc::UnboundedReceiver<Command>, changes: watch::Sender<()>) {
    let port = socket.local_addr().map_or(0, |a| a.port());
    let motd = format!("MCPE;fake server;{PROTOCOL_VERSION};{GAME_VERSION};0;10;1;fake;Survival;1;{port};{port};0;");
    let mut server = Server::new(ServerConfig::new(1, motd), Instant::now());
    let mut peer: Option<(SocketAddr, Peer)> = None;
    let mut kicked = false;
    let mut buf = vec![0u8; 2048];
    loop {
        // Events first: replies they queue must leave in this turn, not at the next wake-up.
        let now = Instant::now();
        while let Some(event) = server.poll_event() {
            match event {
                ServerEvent::Connected { addr, .. } if peer.is_none() => peer = Some((addr, Peer::new(script.clone(), now))),
                ServerEvent::Message(addr, msg) => {
                    if let Some((a, p)) = &mut peer
                        && *a == addr
                        && let Err(e) = p.on_message(now, &msg)
                    {
                        tracing::warn!(error = %e, "fake server: bad client batch");
                    }
                }
                _ => {}
            }
        }
        if let Some((addr, p)) = &mut peer {
            p.advance(now);
            for batch in p.outbox.drain(..) {
                server.send(*addr, batch, Reliability::ReliableOrdered);
            }
            let mut s = shared.lock().expect(LOCK);
            let changed = !p.received.is_empty() || !p.sent.is_empty();
            s.received.append(&mut p.received);
            s.sent.append(&mut p.sent);
            s.spawned = s.spawned.or_else(|| Some(p.spawned_at? - p.login_at()?));
            if changed {
                changes.send_replace(());
            }
            if std::mem::take(&mut kicked) {
                server.close(*addr, now);
                peer = None;
            }
        }
        while let Some((to, d)) = server.poll_transmit(now) {
            let _ = socket.send_to(&d, to).await;
        }
        let due = peer.as_ref().and_then(|(_, p)| p.next_due());
        let deadline = server.poll_timeout().into_iter().chain(due).min().unwrap_or(now + Duration::from_millis(100));
        tokio::select! {
            r = socket.recv_from(&mut buf) => {
                if let Ok((n, from)) = r {
                    server.handle_datagram(Instant::now(), from, Bytes::copy_from_slice(&buf[..n]));
                }
            }
            Some(command) = commands.recv() => {
                if let Some((_, p)) = &mut peer {
                    match command {
                        Command::Send(packet) => p.send(&[packet]),
                        Command::Kick(message) => {
                            p.send(&[encode(&kick(message))]);
                            kicked = true;
                        }
                    }
                }
            }
            _ = tokio::time::sleep_until(deadline.into()) => server.handle_timeout(Instant::now()),
        }
    }
}

fn kick(message: String) -> Disconnect {
    Disconnect {
        reason: DisconnectFailReason::Kicked,
        hide_disconnect_reason: false,
        content: Some(DisconnectContent { message, filtered_message: String::new() }),
    }
}
