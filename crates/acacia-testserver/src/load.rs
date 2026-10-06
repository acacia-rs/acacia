//! The fake server for many clients at once: every client gets its own replay of the script, and
//! nothing is recorded. For profiling a swarm, not for tests ([`crate::FakeServer`]).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use acacia_proto::{GAME_VERSION, PROTOCOL_VERSION};
use acacia_raknet::{Reliability, Server, ServerConfig, ServerEvent};
use bytes::Bytes;
use tokio::net::UdpSocket;

use crate::peer::Peer;
use crate::script::Script;

/// Serves `script` to every client that connects to `socket`, until the task is dropped.
pub async fn serve(socket: UdpSocket, script: Script) {
    let port = socket.local_addr().map_or(0, |a| a.port());
    let motd = format!("MCPE;load server;{PROTOCOL_VERSION};{GAME_VERSION};0;1000;1;load;Survival;1;{port};{port};0;");
    let mut server = Server::new(ServerConfig::new(1, motd), Instant::now());
    let mut peers: HashMap<SocketAddr, Peer> = HashMap::new();
    let mut buf = vec![0u8; 2048];
    loop {
        let now = Instant::now();
        while let Some(event) = server.poll_event() {
            match event {
                ServerEvent::Connected { addr, .. } => {
                    peers.insert(addr, Peer::new(script.clone(), now));
                }
                ServerEvent::Message(addr, msg) => {
                    if let Some(peer) = peers.get_mut(&addr)
                        && let Err(e) = peer.on_message(now, &msg)
                    {
                        tracing::warn!(%addr, error = %e, "load server: bad client batch");
                    }
                }
                ServerEvent::Disconnected(addr, _) => {
                    peers.remove(&addr);
                }
            }
        }
        let mut due = None;
        for (addr, peer) in &mut peers {
            peer.advance(now);
            for batch in peer.outbox.drain(..) {
                server.send(*addr, batch, Reliability::ReliableOrdered);
            }
            peer.received.clear();
            peer.sent.clear();
            due = due.into_iter().chain(peer.next_due()).min();
        }
        while let Some((to, datagram)) = server.poll_transmit(now) {
            let _ = socket.send_to(&datagram, to).await;
        }
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
