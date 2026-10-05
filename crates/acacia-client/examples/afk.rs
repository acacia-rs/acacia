//! Joins a server, idles, prints chat/command output, and reports what arrived.
//! `cargo run -p acacia-client --features socks --example afk -- <server> <name|@account> <seconds>`
//! - `@account` signs in online with tokens cached in ./.tokens (see acacia-auth's device_login).
//! - `BEDROCK_PROXY=host:port:user:pass` routes the game connection and sign-in through SOCKS5.
//! - `BEDROCK_CMD="/list;hello;!respawn"` runs one entry per second after spawn: `/x` = command,
//!   `!respawn` = respawn, anything else = chat.
//! - `BEDROCK_AUTO_RESPAWN=1` respawns automatically on death.
//! - `BEDROCK_TRANSPORT=raknet|nethernet` forces a transport (default: auto, RakNet preferred).
//! - `BEDROCK_STRICT=1` prints strict-mode violations (docs/testing.md).
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_client::proto::packets::{CommandOutput, Respawn, Text, TextContent};
use acacia_client::{Client, ClientBuilder, Event, Socks5Proxy, TransportKind};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "Bot".into());
    let secs: u64 = args.next().map_or(10, |s| s.parse().expect("seconds"));
    let proxy = std::env::var("BEDROCK_PROXY").ok().map(|p| Socks5Proxy::parse(&p)).transpose()?;

    let start = Instant::now();
    let transport = match std::env::var("BEDROCK_TRANSPORT").as_deref() {
        Ok("raknet") => TransportKind::RakNet,
        Ok("nethernet") => TransportKind::NetherNet,
        _ => TransportKind::Auto,
    };
    let strict = std::env::var("BEDROCK_STRICT").is_ok();
    let mut builder =
        Client::builder(&server).auto_respawn(std::env::var("BEDROCK_AUTO_RESPAWN").is_ok()).transport(transport).strict(strict).blob_cache_dir(".blobs").pack_cache_dir(".packs");
    if let Some(p) = &proxy {
        println!("via proxy {}:{}", p.host, p.port);
        builder = builder.proxy(p.clone());
    }
    let builder = login(builder, &name, proxy.as_ref(), start).await?;
    let mut client = builder.connect().await?;
    println!("spawned as {} (runtime id {}) in {:?}", client.display_name(), client.runtime_entity_id(), start.elapsed());

    let script: Vec<String> = std::env::var("BEDROCK_CMD").unwrap_or_default().split(';').filter(|l| !l.is_empty()).map(str::to_owned).collect();
    let mut script = script.into_iter();
    let mut step = tokio::time::interval(Duration::from_secs(1));

    let mut counts: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
    let mut violations = 0;
    let deadline = tokio::time::sleep(Duration::from_secs(secs));
    tokio::pin!(deadline);
    let mut status = tokio::time::interval_at(tokio::time::Instant::now() + Duration::from_secs(60), Duration::from_secs(60));
    loop {
        tokio::select! {
            event = client.recv() => match event {
                Some(Event::Packet(p)) => {
                    let e = counts.entry(p.id).or_default();
                    e.0 += 1;
                    e.1 += p.body.len();
                    if let Ok(text) = p.decode::<Text>() {
                        println!("[chat] {}", text_line(&text));
                    } else if let Ok(out) = p.decode::<CommandOutput>() {
                        println!("[command output] {:?}", out.output);
                    } else if let Ok(r) = p.decode::<Respawn>() {
                        println!("[respawn] state {} at {:?}", r.state, r.position);
                    }
                }
                Some(Event::Violation(v)) => {
                    violations += 1;
                    println!("[violation] {v}");
                }
                Some(Event::Disconnected(reason)) => {
                    println!("disconnected after {:?}: {reason:?}", start.elapsed());
                    break;
                }
                None => break,
            },
            _ = step.tick() => if let Some(line) = script.next() {
                let sent = match line.as_str() {
                    "!respawn" => client.respawn(),
                    l if l.starts_with('/') => client.command(l),
                    l => client.chat(l),
                };
                println!("> {line} (sent: {sent})");
            },
            _ = status.tick() => {
                let (n, bytes) = counts.values().fold((0, 0), |a, c| (a.0 + c.0, a.1 + c.1));
                println!("[{:>4}s] connected, {n} packets / {} KB so far", start.elapsed().as_secs(), bytes / 1024);
            }
            _ = &mut deadline => { client.close(); }
        }
    }
    println!("{:>5} {:>7} {:>10}", "id", "count", "bytes");
    for (id, (n, bytes)) in counts {
        println!("{id:>5} {n:>7} {bytes:>10}");
    }
    if strict {
        println!("{violations} strict-mode violations");
    }
    Ok(())
}

async fn login(builder: ClientBuilder, name: &str, proxy: Option<&Socks5Proxy>, start: Instant) -> Result<ClientBuilder, Box<dyn std::error::Error>> {
    let Some(account) = name.strip_prefix('@') else { return Ok(builder.offline(name)) };
    use acacia_client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
    let config = AuthConfig { proxy: proxy.map(Socks5Proxy::to_url), ..AuthConfig::default() };
    let account = Account::new(Arc::new(AuthClient::new(config)?), Arc::new(FileTokenCache::new(".tokens")?), account);
    let (key, credentials) = account.login_credentials().await?;
    println!("signed in as {} in {:?}", credentials.display_name, start.elapsed());
    Ok(builder.online(credentials, key))
}

fn text_line(t: &Text) -> String {
    match &t.content {
        TextContent::Chat(c) => format!("<{}> {}", c.source_name, c.message),
        TextContent::Whisper(c) => format!("[whisper] <{}> {}", c.source_name, c.message),
        TextContent::Raw(c) => c.message.clone(),
        TextContent::System(c) => c.message.clone(),
        TextContent::Tip(c) => c.message.clone(),
        other => format!("{other:?}"),
    }
}
