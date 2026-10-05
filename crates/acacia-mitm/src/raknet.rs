//! RakNet mode: one RakNet server for every game, and per player an upstream RakNet client and
//! socket (pair.rs). A new player's login and server address are fetched off the loop, so a slow
//! sign-in or lookup holds up nobody else.

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_auth::{Account, LoginCredentials};
use acacia_raknet::{Reliability, Server, ServerConfig, ServerEvent};
use bytes::Bytes;
use p384::ecdsa::SigningKey;
use serde_json::json;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::intercept::{Injection, Injector, Session};
use crate::pair::Pair;
use crate::proxy::Setup;
use crate::record::DatagramLog;
use crate::relay::Wire;
use crate::status::watch_status;
use crate::transfer::{self, Routes};

/// A proxied player plus the socket and task that carry its server side.
struct Link {
    pair: Pair,
    socket: Arc<UdpSocket>,
    reader: JoinHandle<()>,
}

enum Slot {
    /// Being dialed: what the game sent meanwhile.
    Dialing(Vec<Bytes>),
    Up(Box<Link>),
}

/// What a new player's server side is built from.
struct Dialed {
    key: SigningKey,
    credentials: Option<LoginCredentials>,
    upstream: SocketAddr,
}

/// `route` is a transfer target to dial in place of `server`.
async fn dial(account: Option<Account>, route: Option<(String, u16)>, server: SocketAddr) -> Result<Dialed, String> {
    let key = SigningKey::random(&mut rand_core::OsRng);
    let credentials = match &account {
        Some(account) => Some(account.credentials(&key).await.map_err(|e| format!("online login: {e}"))?),
        None => None,
    };
    let upstream = match route {
        Some((host, port)) => transfer::resolve(&host, port).await.map_err(|e| format!("transfer to {host}:{port}: {e}"))?,
        None => server,
    };
    Ok(Dialed { key, credentials, upstream })
}

/// What every link is opened with.
struct Hub {
    setup: Setup,
    listen: SocketAddr,
    upstream_tx: mpsc::UnboundedSender<(SocketAddr, Bytes)>,
    inject_tx: mpsc::UnboundedSender<Injection>,
}

impl Hub {
    async fn open(&self, game: SocketAddr, dialed: Dialed, now: Instant) -> io::Result<Link> {
        let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        socket.connect(dialed.upstream).await?;
        let reader = tokio::spawn(read_upstream(socket.clone(), game, self.upstream_tx.clone()));
        let proxy = transfer::proxy_addr(game, self.listen.port());
        let session = Session { game, proxy: *proxy.as_ref().unwrap_or(&self.listen), injector: Injector::new(game, self.inject_tx.clone()) };
        let mut relay = self.setup.relay(Wire::RakNet, dialed.key, dialed.credentials, &session);
        relay.note(json!({ "event": "connected", "transport": "raknet", "server": dialed.upstream.to_string() }));
        if self.setup.follow_transfers
            && let Ok(proxy) = proxy
        {
            relay = relay.follow_transfers(proxy);
        }
        Ok(Link { pair: Pair::new(dialed.upstream, now, relay), socket, reader })
    }
}

pub(crate) async fn run(listener: UdpSocket, setup: Setup, account: Option<Account>, mut trace: Option<DatagramLog>) -> io::Result<()> {
    let target = setup.server;
    let guid = rand_core::RngCore::next_u64(&mut rand_core::OsRng);
    let mut server = Server::new(ServerConfig::new(guid, String::new()), Instant::now());
    let listen = listener.local_addr()?;
    let mut motd = watch_status(target, guid, listen.port());
    motd.mark_changed();
    let mut routes = Routes::default();
    let (upstream_tx, mut upstream_rx) = mpsc::unbounded_channel::<(SocketAddr, Bytes)>();
    let (inject_tx, mut inject_rx) = mpsc::unbounded_channel::<Injection>();
    let (dialed_tx, mut dialed_rx) = mpsc::unbounded_channel::<(SocketAddr, Result<Dialed, String>)>();
    let hub = Hub { setup, listen, upstream_tx, inject_tx };
    let mut slots: HashMap<SocketAddr, Slot> = HashMap::new();

    let mut buf = vec![0u8; 2048];
    loop {
        // Events first: what they queue must leave in this turn, not at the next wake-up.
        let now = Instant::now();
        while let Some(event) = server.poll_event() {
            match event {
                ServerEvent::Connected { addr: game, .. } => {
                    println!("{game} connected (RakNet)");
                    slots.insert(game, Slot::Dialing(Vec::new()));
                    let (account, route, tx) = (account.clone(), routes.take(game.ip(), now), dialed_tx.clone());
                    tokio::spawn(async move {
                        let _ = tx.send((game, dial(account, route, target).await));
                    });
                }
                ServerEvent::Message(game, msg) => match slots.get_mut(&game) {
                    Some(Slot::Dialing(held)) => held.push(msg),
                    Some(Slot::Up(link)) => link.pair.on_game_message(&msg),
                    None => {}
                },
                ServerEvent::Disconnected(game, reason) => {
                    println!("{game} left: {reason:?}");
                    match slots.get_mut(&game) {
                        Some(Slot::Up(link)) => link.pair.close(now),
                        _ => drop(slots.remove(&game)),
                    }
                }
            }
        }
        for (&game, slot) in &mut slots {
            let Slot::Up(link) = slot else { continue };
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
        slots.retain(|&game, slot| {
            let Slot::Up(link) = slot else { return true };
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

        let timeouts = slots.values().filter_map(|s| match s {
            Slot::Up(link) => link.pair.poll_timeout(),
            Slot::Dialing(_) => None,
        });
        let deadline = timeouts.chain(server.poll_timeout()).min().unwrap_or(now + Duration::from_secs(1));
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
                if let Some(Slot::Up(link)) = slots.get_mut(&game) {
                    link.pair.on_upstream_datagram(Instant::now(), data);
                }
            }
            Some((game, dialed)) = dialed_rx.recv() => {
                // A game that left while it was dialed has no slot; one that also rejoined is dialing again.
                let Some(Slot::Dialing(held)) = slots.get_mut(&game) else { continue };
                let held = std::mem::take(held);
                match dialed {
                    Ok(dialed) => {
                        let mut link = hub.open(game, dialed, Instant::now()).await?;
                        held.iter().for_each(|msg| link.pair.on_game_message(msg));
                        slots.insert(game, Slot::Up(Box::new(link)));
                    }
                    Err(e) => {
                        eprintln!("{game}: {e}");
                        slots.remove(&game);
                        server.close(game, Instant::now());
                    }
                }
            }
            Some(first) = inject_rx.recv() => {
                let mut by_game: HashMap<SocketAddr, Vec<_>> = HashMap::new();
                for i in std::iter::once(first).chain(std::iter::from_fn(|| inject_rx.try_recv().ok())) {
                    by_game.entry(i.game).or_default().push((i.dir, i.packet));
                }
                for (game, packets) in by_game {
                    if let Some(Slot::Up(link)) = slots.get_mut(&game) {
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
                for slot in slots.values_mut() {
                    if let Slot::Up(link) = slot {
                        link.pair.handle_timeout(now);
                    }
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
