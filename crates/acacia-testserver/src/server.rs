use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use acacia_raknet::{Reliability, Server, ServerConfig, ServerEvent};
use acacia_proto::{packets, GAME_VERSION, Packet as _, PROTOCOL_VERSION};
use bytes::Bytes;
use serde_json::json;
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

use crate::peer::{Peer, Received};
use crate::script::Script;

#[derive(Default)]
struct Shared {
    received: Vec<Received>,
    sent: Vec<Received>,
    /// When the server sent PlayStatus(PlayerSpawn), after Login.
    spawned: Option<Duration>,
}

/// A local RakNet server that replays a [`Script`] to the first client that joins, answers its
/// sub-chunk and blob requests, and records what it sends. Stops when dropped.
pub struct FakeServer {
    addr: SocketAddr,
    shared: Arc<Mutex<Shared>>,
    task: JoinHandle<()>,
}

impl FakeServer {
    pub async fn start(script: Script) -> io::Result<Self> {
        let socket = UdpSocket::bind("127.0.0.1:0").await?;
        let addr = socket.local_addr()?;
        let shared = Arc::new(Mutex::new(Shared::default()));
        let task = tokio::spawn(run(socket, script, shared.clone()));
        Ok(Self { addr, shared, task })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// What the client sent so far, in order.
    pub fn received(&self) -> Vec<Received> {
        self.shared.lock().expect("fake server lock").received.clone()
    }

    /// Time from Login to the server's PlayStatus(PlayerSpawn).
    pub fn spawned_after_login(&self) -> Option<Duration> {
        self.shared.lock().expect("fake server lock").spawned
    }

    /// Both directions as a tools/mitm capture, for `capdiff` against a vanilla capture.
    pub fn write_capture(&self, path: &std::path::Path) -> io::Result<()> {
        let shared = self.shared.lock().expect("fake server lock");
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

async fn run(socket: UdpSocket, script: Script, shared: Arc<Mutex<Shared>>) {
    let port = socket.local_addr().map_or(0, |a| a.port());
    let motd = format!("MCPE;fake server;{PROTOCOL_VERSION};{GAME_VERSION};0;10;1;fake;Survival;1;{port};{port};0;");
    let mut server = Server::new(ServerConfig::new(1, motd), Instant::now());
    let mut peer: Option<(SocketAddr, Peer)> = None;
    let mut buf = vec![0u8; 2048];
    loop {
        // Events first: replies they queue must leave in this turn, not at the next wake-up.
        let now = Instant::now();
        while let Some(event) = server.poll_event() {
            match event {
                ServerEvent::Connected(addr) if peer.is_none() => peer = Some((addr, Peer::new(script.clone(), now))),
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
            let mut s = shared.lock().expect("fake server lock");
            s.received.extend(p.received.drain(..));
            s.sent.extend(p.sent.drain(..));
            s.spawned = s.spawned.or_else(|| Some(p.spawned_at? - p.login_at()?));
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
            _ = tokio::time::sleep_until(deadline.into()) => server.handle_timeout(Instant::now()),
        }
    }
}
