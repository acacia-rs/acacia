//! RakNet mode: one RakNet server for every game, and per player an upstream RakNet client and
//! socket (pair.rs).

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_auth::Account;
use acacia_raknet::{Reliability, Server, ServerConfig, ServerEvent};
use bytes::{Bytes, BytesMut};
use p384::ecdsa::SigningKey;
use serde_json::json;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::intercept::{Injection, Injector, Session};
use crate::pair::Pair;
use crate::proxy::Setup;
use crate::record::DatagramLog;
use crate::relay::Wire;
use crate::transfer::{self, Routes};

const STATUS_INTERVAL: Duration = Duration::from_secs(3);

/// A proxied player plus the socket and task that carry its server side.
struct Link {
    pair: Pair,
    socket: Arc<UdpSocket>,
    reader: JoinHandle<()>,
}

pub(crate) async fn run(listener: UdpSocket, setup: Setup, account: Option<Account>, mut trace: Option<DatagramLog>) -> io::Result<()> {
    let target = setup.server;
    let guid = rand_core::RngCore::next_u64(&mut rand_core::OsRng);
    let mut server = Server::new(ServerConfig::new(guid, String::new()), Instant::now());
    let port = listener.local_addr()?.port();
    let mut motd = watch_status(target, guid, port);
    motd.mark_changed();
    let mut routes = Routes::default();
    let (upstream_tx, mut upstream_rx) = mpsc::unbounded_channel::<(SocketAddr, Bytes)>();
    let (inject_tx, mut inject_rx) = mpsc::unbounded_channel::<Injection>();
    let mut links: HashMap<SocketAddr, Link> = HashMap::new();

    let mut buf = vec![0u8; 2048];
    loop {
        // Events first: what they queue must leave in this turn, not at the next wake-up.
        let now = Instant::now();
        while let Some(event) = server.poll_event() {
            match event {
                ServerEvent::Connected { addr: game, .. } => {
                    println!("{game} connected (RakNet)");
                    let key = SigningKey::random(&mut rand_core::OsRng);
                    let credentials = match &account {
                        Some(account) => match account.credentials(&key).await {
                            Ok(c) => Some(c),
                            Err(e) => {
                                eprintln!("online login: {e}");
                                server.close(game, now);
                                continue;
                            }
                        },
                        None => None,
                    };
                    let upstream = match routes.take(game.ip(), now) {
                        Some((host, port)) => match transfer::resolve(&host, port).await {
                            Ok(addr) => addr,
                            Err(e) => {
                                eprintln!("transfer to {host}:{port}: {e}");
                                server.close(game, now);
                                continue;
                            }
                        },
                        None => target,
                    };
                    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
                    socket.connect(upstream).await?;
                    let reader = tokio::spawn(read_upstream(socket.clone(), game, upstream_tx.clone()));
                    let session = Session { game, injector: Injector::new(game, inject_tx.clone()) };
                    let mut relay = setup.relay(Wire::RakNet, key, credentials, &session);
                    relay.note(json!({ "event": "connected", "transport": "raknet", "server": upstream.to_string() }));
                    if setup.follow_transfers
                        && let Ok(proxy) = transfer::proxy_addr(game, port)
                    {
                        relay = relay.follow_transfers(proxy);
                    }
                    links.insert(game, Link { pair: Pair::new(upstream, now, relay), socket, reader });
                }
                ServerEvent::Message(game, msg) => {
                    if let Some(link) = links.get_mut(&game) {
                        link.pair.on_game_message(&msg);
                    }
                }
                ServerEvent::Disconnected(game, reason) => {
                    println!("{game} left: {reason:?}");
                    if let Some(link) = links.get_mut(&game) {
                        link.pair.close(now);
                    }
                }
            }
        }
        for (&game, link) in &mut links {
            if let Some(to) = link.pair.take_transfer() {
                println!("{game} transferred to {}:{}", to.0, to.1);
                routes.set(game.ip(), to, now);
            }
            for batch in link.pair.take_to_game() {
                server.send(game, batch, Reliability::ReliableOrdered);
            }
            while let Some(d) = link.pair.poll_transmit(now) {
                let _ = link.socket.send(&d).await;
            }
        }
        links.retain(|&game, link| {
            if link.pair.is_closed() {
                server.close(game, now);
                link.reader.abort();
            }
            !link.pair.is_closed()
        });
        while let Some((to, d)) = server.poll_transmit(now) {
            if let Some(log) = &mut trace {
                log.write(false, to, &d);
            }
            if let Err(e) = listener.send_to(&d, to).await {
                eprintln!("send to {to}: {e}");
            }
        }

        let deadline = links.values().filter_map(|l| l.pair.poll_timeout()).chain(server.poll_timeout()).min().unwrap_or(now + Duration::from_secs(1));
        tokio::select! {
            r = listener.recv_from(&mut buf) => match r {
                Ok((n, from)) => {
                    if let Some(log) = &mut trace {
                        log.write(true, from, &buf[..n]);
                    }
                    server.handle_datagram(Instant::now(), from, Bytes::copy_from_slice(&buf[..n]));
                }
                // Windows reports ICMP port-unreachable from an earlier send_to as an error here.
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {}
                Err(e) => return Err(e),
            },
            Some((game, data)) = upstream_rx.recv() => {
                if let Some(link) = links.get_mut(&game) {
                    link.pair.on_upstream_datagram(Instant::now(), data);
                }
            }
            Some(first) = inject_rx.recv() => {
                let mut by_game: HashMap<SocketAddr, Vec<_>> = HashMap::new();
                for i in std::iter::once(first).chain(std::iter::from_fn(|| inject_rx.try_recv().ok())) {
                    by_game.entry(i.game).or_default().push((i.dir, i.packet));
                }
                for (game, packets) in by_game {
                    if let Some(link) = links.get_mut(&game) {
                        link.pair.inject(packets);
                    }
                }
            }
            Ok(()) = motd.changed() => {
                let status = motd.borrow_and_update().clone();
                println!("server status: {status}");
                server.set_motd(status);
            }
            _ = tokio::time::sleep_until(deadline.into()) => {
                let now = Instant::now();
                server.handle_timeout(now);
                for link in links.values_mut() {
                    link.pair.handle_timeout(now);
                }
            }
        }
    }
}

async fn read_upstream(socket: Arc<UdpSocket>, game: SocketAddr, tx: mpsc::UnboundedSender<(SocketAddr, Bytes)>) {
    let mut buf = vec![0u8; 2048];
    loop {
        match socket.recv(&mut buf).await {
            Ok(n) => {
                if tx.send((game, Bytes::copy_from_slice(&buf[..n]))).is_err() {
                    return;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {}
            Err(_) => return,
        }
    }
}

/// Mirrors the server's ping answer (so the game's server list shows it), with our GUID and port.
/// Until the server answers (BDS 1.26.52 ignores pings), a generic one: the game won't join a blank status.
fn watch_status(server: SocketAddr, guid: u64, port: u16) -> watch::Receiver<String> {
    let (protocol, version) = (acacia_proto::PROTOCOL_VERSION, acacia_proto::GAME_VERSION);
    let fallback = format!("MCPE;acacia-mitm;{protocol};{version};0;10;{guid};acacia-mitm;Survival;1;{port};{port};0;");
    let (tx, rx) = watch::channel(fallback);
    tokio::spawn(async move {
        let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await else { return };
        let mut buf = vec![0u8; 2048];
        while !tx.is_closed() {
            let mut ping = BytesMut::new();
            acacia_raknet::unconnected_ping(&mut ping, 0, guid);
            let _ = socket.send_to(&ping, server).await;
            if let Ok(Ok((n, _))) = tokio::time::timeout(STATUS_INTERVAL, socket.recv_from(&mut buf)).await
                && let Ok(pong) = acacia_raknet::parse_pong(&buf[..n])
            {
                let mut fields: Vec<String> = pong.motd.split(';').map(str::to_owned).collect();
                for (i, value) in [(6, guid.to_string()), (10, port.to_string()), (11, port.to_string())] {
                    if let Some(f) = fields.get_mut(i) {
                        *f = value;
                    }
                }
                let motd = fields.join(";");
                tx.send_if_modified(|m| m != &motd && { *m = motd; true });
            }
            tokio::time::sleep(STATUS_INTERVAL).await;
        }
    });
    rx
}
