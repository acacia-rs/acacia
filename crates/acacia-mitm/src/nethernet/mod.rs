//! NetherNet mode. Serves direct-connect signaling on the TCP port as BDS does (a TLS refusal, then
//! `GET /v1/join` and `POST /v1/join/{id}` over plain HTTP). For each offer it dials the server the
//! same way, answers the game, and relays between them (link.rs). BDS wants a real
//! MultiplayerToken in the offer even offline, so this mode needs an account.

mod link;

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use acacia_auth::Account;
use acacia_nethernet::{http, Connection, Identity};
use p384::ecdsa::SigningKey;
use rand_core::{OsRng, RngCore};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::time::timeout;

use crate::record::Recorder;
use crate::relay::{Relay, Wire};
use link::Link;

const SIGNALING_TIMEOUT: Duration = Duration::from_secs(15);

pub struct Host {
    pub server: SocketAddr,
    pub account: Account,
    /// Signs our answers. The game pins it per server address on first use.
    pub key: SigningKey,
    pub rec: Arc<Mutex<Recorder>>,
}

pub async fn run(listen: SocketAddr, host: Host) -> io::Result<()> {
    let listener = TcpListener::bind(listen).await?;
    let host = Arc::new(host);
    loop {
        let (tcp, peer) = listener.accept().await?;
        let host = host.clone();
        tokio::spawn(async move {
            if let Err(e) = serve(tcp, &host).await {
                eprintln!("{peer}: {e}");
            }
        });
    }
}

/// Kept in the capture folder and reused: a new key would break the game's pin of the old one.
pub fn host_key(dir: &Path) -> io::Result<SigningKey> {
    let path = dir.join("nethernet-host.key");
    if let Ok(text) = std::fs::read_to_string(&path)
        && let Ok(bytes) = hex::decode(text.trim())
        && let Ok(key) = SigningKey::from_slice(&bytes)
    {
        return Ok(key);
    }
    let key = SigningKey::random(&mut OsRng);
    std::fs::write(&path, hex::encode(key.to_bytes()))?;
    Ok(key)
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// One signaling request. A join then runs its link on this task until the player leaves.
async fn serve(mut tcp: TcpStream, host: &Host) -> Result<(), String> {
    let raw = timeout(SIGNALING_TIMEOUT, read_http(&mut tcp, |r| http::is_tls(r) || http::request_complete(r)))
        .await
        .map_err(|_| "request timed out")?
        .map_err(err)?;
    if http::is_tls(&raw) {
        return tcp.write_all(&http::TLS_REFUSAL).await.map_err(err);
    }
    let req = http::parse_request(&raw).map_err(err)?;
    let is_join = req.path.strip_prefix("/v1/join/").is_some_and(|id| id.parse::<u64>().is_ok());
    match req.method {
        "GET" if req.path == acacia_nethernet::PROBE_PATH => tcp.write_all(&http::response(200, "application/json", "")).await.map_err(err),
        "POST" if is_join => {
            let local = tcp.local_addr().map_err(err)?.ip();
            if local.is_loopback() {
                eprintln!("the game joined on a loopback address; its WebRTC may not reach it. Use this machine's LAN IP");
            }
            match join(&String::from_utf8_lossy(req.body), local, host).await {
                Ok((answer, link)) => {
                    tcp.write_all(&http::response(200, "application/sdp", &answer)).await.map_err(err)?;
                    drop(tcp);
                    link.run(host.rec.clone()).await;
                    Ok(())
                }
                Err(e) => {
                    let _ = tcp.write_all(&http::response(503, "text/plain", "")).await;
                    Err(format!("join: {e}"))
                }
            }
        }
        _ => tcp.write_all(&http::response(404, "text/plain", "")).await.map_err(err),
    }
}

/// Dials the server for a game whose offer reached us on `local`, then answers the game.
async fn join(offer: &str, local: IpAddr, host: &Host) -> Result<(String, Link), String> {
    let key = SigningKey::random(&mut OsRng);
    let credentials = host.account.credentials(&key).await.map_err(err)?;
    let token = credentials.multiplayer_token.clone().ok_or("the account has no MultiplayerToken")?;
    let (up, up_udp) = timeout(SIGNALING_TIMEOUT, dial(host.server, &Identity::multiplayer(key.clone(), token)))
        .await
        .map_err(|_| "server signaling timed out")??;
    let game_udp = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.map_err(err)?;
    let game_addr = SocketAddr::new(local, game_udp.local_addr().map_err(err)?.port());
    let (game, answer) = Connection::answer(offer, game_addr, &host.key, Instant::now()).map_err(err)?;
    println!("NetherNet player joining: game side {game_addr}, server side {:?}", up.host_candidate());
    host.rec.lock().unwrap_or_else(|p| p.into_inner()).write(json!({ "event": "connected", "transport": "nethernet" }));
    let relay = Relay::new(Wire::NetherNet, key, Some(credentials));
    Ok((answer, Link { game, game_udp, up, up_udp, relay }))
}

/// Posts our offer to the server and accepts its answer; ICE and DTLS follow in the link.
async fn dial(server: SocketAddr, identity: &Identity) -> Result<(Connection, UdpSocket), String> {
    let udp = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await.map_err(err)?;
    // BDS answers only with LAN candidates, so advertise the address the OS routes to it from.
    let route = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(err)?;
    route.connect(server).map_err(err)?;
    let local = SocketAddr::new(route.local_addr().map_err(err)?.ip(), udp.local_addr().map_err(err)?.port());
    let (mut conn, offer) = Connection::offer(local, identity, Instant::now()).map_err(err)?;
    let mut tcp = TcpStream::connect(server).await.map_err(err)?;
    tcp.write_all(&http::offer_request(&server.to_string(), OsRng.next_u64(), &offer)).await.map_err(err)?;
    let raw = read_http(&mut tcp, http::response_complete).await.map_err(err)?;
    conn.accept_answer(&http::answer_sdp(&raw).map_err(err)?, Instant::now()).map_err(err)?;
    Ok((conn, udp))
}

/// Reads until `done` holds, EOF, or the signaling size cap.
async fn read_http(tcp: &mut TcpStream, done: fn(&[u8]) -> bool) -> io::Result<Vec<u8>> {
    let (mut raw, mut chunk) = (Vec::new(), [0u8; 8192]);
    while !done(&raw) && raw.len() < http::MAX_MESSAGE {
        match tcp.read(&mut chunk).await? {
            0 => break,
            n => raw.extend_from_slice(&chunk[..n]),
        }
    }
    Ok(raw)
}
