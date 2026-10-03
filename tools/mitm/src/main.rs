//! Recording RakNet proxy for ground-truth captures of the vanilla client: the game joins this proxy,
//! the proxy joins the server as the same player, and every packet on the game's side is logged
//! (record.rs).
//!
//! `cargo run -p acacia-mitm -- [--listen 0.0.0.0:19180] [--server 127.0.0.1:19140] [--out .testserver/mitm] [--online <account>]`
//!
//! Offline by default, for the local test BDS. `--online <account>` signs in with that account
//! (`.tokens`, device code on first use) for real servers; sign the game into the same account so the
//! client data matches. Login is logged as a structural summary only (login.rs): no tokens or signatures.

mod login;
mod pair;
mod record;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_raknet::{Reliability, Server, ServerConfig, ServerEvent};
use bytes::{Bytes, BytesMut};
use p384::ecdsa::SigningKey;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use pair::Pair;
use record::Recorder;

const STATUS_INTERVAL: Duration = Duration::from_secs(3);

struct Args {
    listen: SocketAddr,
    /// `host:port`, resolved once at start.
    server: String,
    out: String,
    online: Option<String>,
}

fn args() -> Result<Args, String> {
    let mut args = Args { listen: "0.0.0.0:19180".parse().unwrap(), server: "127.0.0.1:19140".into(), out: ".testserver/mitm".into(), online: None };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--listen" => args.listen = value.parse().map_err(|e| format!("--listen: {e}"))?,
            "--server" => args.server = value,
            "--out" => args.out = value,
            "--online" => args.online = Some(value),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    Ok(args)
}

async fn sign_in(id: &str) -> Result<Account, Box<dyn std::error::Error>> {
    let account = Account::new(Arc::new(AuthClient::new(AuthConfig::default())?), Arc::new(FileTokenCache::new(".tokens")?), id);
    if !account.is_signed_in().await? {
        account.sign_in(|p| println!("sign in at {} with code {}", p.verification_uri, p.user_code)).await?;
    }
    Ok(account)
}

/// A proxied player plus the socket and task that carry its server side.
struct Link {
    pair: Pair,
    socket: Arc<UdpSocket>,
    reader: JoinHandle<()>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = args()?;
    // MITM_TRACE=1 logs every datagram on the game's side (first byte and size), to debug joins.
    let trace = std::env::var_os("MITM_TRACE").is_some();
    let stamp = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let name = stamp.format(time::macros::format_description!("[year][month][day]-[hour][minute][second]"))?;
    let target = tokio::net::lookup_host(&args.server).await?.next().ok_or(format!("{}: no address", args.server))?;
    let account = match &args.online {
        Some(id) => Some(sign_in(id).await?),
        None => None,
    };
    let mut rec = Recorder::create(args.out.as_ref(), &name)?;
    let listener = UdpSocket::bind(args.listen).await?;
    let guid = rand_core::RngCore::next_u64(&mut rand_core::OsRng);
    let mut server = Server::new(ServerConfig::new(guid, String::new()), Instant::now());
    let mut motd = watch_status(target, guid, args.listen.port());
    motd.mark_changed();
    let (upstream_tx, mut upstream_rx) = mpsc::unbounded_channel::<(SocketAddr, Bytes)>();
    let mut links: HashMap<SocketAddr, Link> = HashMap::new();
    println!("join {} from the game; recording to {}", args.listen, rec.path().display());

    let mut buf = vec![0u8; 2048];
    loop {
        // Events first: what they queue must leave in this turn, not at the next wake-up.
        let now = Instant::now();
        while let Some(event) = server.poll_event() {
            match event {
                ServerEvent::Connected(game) => {
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
                    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
                    socket.connect(target).await?;
                    let reader = tokio::spawn(read_upstream(socket.clone(), game, upstream_tx.clone()));
                    links.insert(game, Link { pair: Pair::new(target, now, key, credentials), socket, reader });
                }
                ServerEvent::Message(game, msg) => {
                    if let Some(link) = links.get_mut(&game) {
                        link.pair.on_game_message(&msg, &mut rec);
                    }
                }
                ServerEvent::Disconnected(game, reason) => {
                    println!("{game} left: {reason:?}");
                    if let Some(link) = links.get_mut(&game) {
                        link.pair.close(now, &mut rec);
                    }
                }
            }
        }
        for (&game, link) in &mut links {
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
            if trace {
                eprintln!("-> {to} {:#04x} {}B", d[0], d.len());
            }
            if let Err(e) = listener.send_to(&d, to).await {
                eprintln!("send to {to}: {e}");
            }
        }

        let deadline = links.values().filter_map(|l| l.pair.poll_timeout()).chain(server.poll_timeout()).min().unwrap_or(now + Duration::from_secs(1));
        tokio::select! {
            r = listener.recv_from(&mut buf) => match r {
                Ok((n, from)) => {
                    if trace {
                        eprintln!("<- {from} {:#04x} {n}B", buf[0]);
                    }
                    server.handle_datagram(Instant::now(), from, Bytes::copy_from_slice(&buf[..n]));
                }
                // Windows reports ICMP port-unreachable from an earlier send_to as an error here.
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {}
                Err(e) => return Err(e.into()),
            },
            Some((game, data)) = upstream_rx.recv() => {
                if let Some(link) = links.get_mut(&game) {
                    link.pair.on_upstream_datagram(Instant::now(), data, &mut rec);
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
                    link.pair.handle_timeout(now, &mut rec);
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
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {}
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
        loop {
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
