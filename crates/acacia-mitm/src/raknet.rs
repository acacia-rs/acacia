//! RakNet mode: one RakNet server for every game, and per player a connection to its server
//! (pair.rs): a RakNet client on a socket of its own, or a raw link to a realm. A new player's
//! login and server are fetched off the loop (reach.rs), so a slow sign-in or dial holds up nobody
//! else.

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_auth::Account;
use acacia_raknet::{Reliability, Server, ServerConfig, ServerEvent};
use bytes::Bytes;
use serde_json::json;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::intercept::{Injection, Injector, Session};
use crate::pair::Pair;
use crate::proxy::Setup;
use crate::reach::{Arrived, Dialed, Reach, carry, dial};
use crate::record::DatagramLog;
use crate::relay::Wire;
use crate::status::watch_status;
use crate::transfer::{self, Arrival, Doors};

/// A proxied player plus what carries its server side: over RakNet a socket and the task reading
/// it; over a raw link only [`carry`], which ends by itself.
struct Link {
    pair: Pair,
    socket: Option<Arc<UdpSocket>>,
    reader: Option<JoinHandle<()>>,
}

enum Slot {
    /// Being dialed: what the game sent meanwhile.
    Dialing(Vec<Bytes>),
    Up(Box<Link>),
}

/// What every link is opened with.
struct Hub {
    setup: Setup,
    listen: SocketAddr,
    upstream_tx: mpsc::UnboundedSender<Arrived>,
    inject_tx: mpsc::UnboundedSender<Injection>,
}

impl Hub {
    /// `door` is the port the game is to come back through if its server transfers it.
    async fn open(&self, game: SocketAddr, dialed: Dialed, door: Option<u16>, now: Instant) -> io::Result<Link> {
        let proxy = transfer::proxy_addr(game, self.listen.port());
        let session = Session { game, proxy: *proxy.as_ref().unwrap_or(&self.listen), injector: Injector::new(game, self.inject_tx.clone()) };
        let server_wire = match &dialed.upstream {
            Reach::Address(_) => Wire::RakNet,
            Reach::Link(_) => Wire::NetherNet,
        };
        let mut relay = self.setup.relay((Wire::RakNet, server_wire), dialed.key, dialed.credentials, &session);
        if let (Some(port), Ok(proxy)) = (door, proxy) {
            relay = relay.follow_transfers(SocketAddr::new(proxy.ip(), port));
        }
        match dialed.upstream {
            Reach::Address(server) => {
                let socket = Arc::new(UdpSocket::bind(transfer::any_port(server)).await?);
                socket.connect(server).await?;
                let reader = tokio::spawn(read_upstream(socket.clone(), game, self.upstream_tx.clone()));
                relay.note(json!({ "event": "connected", "transport": "raknet", "server": server.to_string() }));
                Ok(Link { pair: Pair::new(server, now, relay), socket: Some(socket), reader: Some(reader) })
            }
            Reach::Link(link) => {
                let (to_link, from_pair) = mpsc::unbounded_channel();
                tokio::spawn(carry(link, game, self.upstream_tx.clone(), from_pair));
                relay.note(json!({ "event": "connected", "transport": "raknet", "server_transport": "nethernet" }));
                Ok(Link { pair: Pair::raw(to_link, relay), socket: None, reader: None })
            }
        }
    }
}

pub(crate) async fn run(listener: UdpSocket, setup: Setup, account: Option<Account>, mut trace: Option<DatagramLog>) -> io::Result<()> {
    let target = setup.target.clone();
    let guid = rand_core::RngCore::next_u64(&mut rand_core::OsRng);
    let mut server = Server::new(ServerConfig::new(guid, String::new()), Instant::now());
    let listen = listener.local_addr()?;
    let mut motd = watch_status(setup.address(), guid, listen.port());
    motd.mark_changed();
    let (door_tx, mut door_rx) = mpsc::unbounded_channel::<Arrival>();
    let mut doors = Doors::new(door_tx);
    // The door each game talks through, for those that did not come in on the listener.
    let mut via: HashMap<SocketAddr, u16> = HashMap::new();
    let (upstream_tx, mut upstream_rx) = mpsc::unbounded_channel::<Arrived>();
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
                    let route = via.get(&game).and_then(|&port| doors.target(port, now));
                    let (account, target, tx) = (account.clone(), target.clone(), dialed_tx.clone());
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
                doors.lead(game, to, now);
            }
            for batch in link.pair.take_to_game() {
                server.send(game, batch, Reliability::ReliableOrdered);
            }
            while let Some(d) = link.pair.poll_transmit(now) {
                if let Some(socket) = &link.socket {
                    let _ = socket.send(&d).await;
                }
            }
        }
        slots.retain(|&game, slot| {
            let Slot::Up(link) = slot else { return true };
            if link.pair.is_closed() {
                server.close(game, now);
                link.reader.iter().for_each(JoinHandle::abort);
                doors.left(game);
            }
            !link.pair.is_closed()
        });
        via.retain(|game, port| slots.contains_key(game) || doors.target(*port, now).is_some());
        doors.sweep(now, |port| via.values().any(|&used| used == port));
        while let Some((to, d)) = server.poll_transmit(now) {
            if let Some(log) = &mut trace {
                log.write(false, to, &d);
            }
            // A game hears back from the port it spoke to.
            let socket = via.get(&to).and_then(|&port| doors.socket(port)).map_or(&listener, |door| &**door);
            if let Err(e) = socket.send_to(&d, to).await {
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
            Some((port, from, data)) = door_rx.recv() => {
                // Through a door come the game it leads somewhere, and whoever already joined through it.
                let now = Instant::now();
                let known = via.get(&from).copied();
                if known == Some(port) || (known.is_none() && doors.target(port, now).is_some()) {
                    via.insert(from, port);
                    if let Some(log) = &mut trace {
                        log.write(true, from, &data);
                    }
                    server.handle_datagram(now, from, data);
                }
            }
            Some((game, data)) = upstream_rx.recv() => {
                if let Some(Slot::Up(link)) = slots.get_mut(&game) {
                    match data {
                        Some(data) => link.pair.on_upstream(Instant::now(), data),
                        None => link.pair.on_upstream_ended(),
                    }
                }
            }
            Some((game, dialed)) = dialed_rx.recv() => {
                // A game that left while it was dialed has no slot; one that also rejoined is dialing again.
                let Some(Slot::Dialing(held)) = slots.get_mut(&game) else { continue };
                let held = std::mem::take(held);
                let door = match hub.setup.follow_transfers {
                    true => doors.open(listen.ip(), game).await.inspect_err(|e| eprintln!("{game}: no door for transfers: {e}")).ok(),
                    false => None,
                };
                let opened = match dialed {
                    Ok(dialed) => hub.open(game, dialed, door, Instant::now()).await.map_err(|e| format!("server socket: {e}")),
                    Err(e) => Err(e),
                };
                // One player's failure is that player's: the proxy goes on for the others.
                match opened {
                    Ok(mut link) => {
                        held.iter().for_each(|msg| link.pair.on_game_message(msg));
                        slots.insert(game, Slot::Up(Box::new(link)));
                    }
                    Err(e) => {
                        eprintln!("{game}: {e}");
                        slots.remove(&game);
                        doors.left(game);
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

async fn read_upstream(socket: Arc<UdpSocket>, game: SocketAddr, tx: mpsc::UnboundedSender<Arrived>) {
    let mut buf = vec![0u8; 2048];
    loop {
        match socket.recv(&mut buf).await {
            Ok(n) => {
                if tx.send((game, Some(Bytes::copy_from_slice(&buf[..n])))).is_err() {
                    return;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {}
            Err(_) => return,
        }
    }
}
