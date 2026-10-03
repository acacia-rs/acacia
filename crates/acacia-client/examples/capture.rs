//! Records packets (header id + body) to `<outdir>/<seq>_<id>.bin` for fixtures and replay.
//! `cargo run -p acacia-client --example capture -- <server> <name|@account> <seconds> <outdir> [ids...]`
//! No ids = record everything. `BEDROCK_PROXY` works as in the `afk` example.
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use acacia_client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_client::{Client, Event, PacketFilter, Socks5Proxy};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [server, name, secs, outdir, ids @ ..] = args.as_slice() else {
        return Err("usage: capture <server> <name|@account> <seconds> <outdir> [ids...]".into());
    };
    let outdir = PathBuf::from(outdir);
    std::fs::create_dir_all(&outdir)?;
    let filter = if ids.is_empty() { PacketFilter::all() } else { ids.iter().map(|i| i.parse::<u32>()).collect::<Result<_, _>>()? };
    let proxy = std::env::var("BEDROCK_PROXY").ok().map(|p| Socks5Proxy::parse(&p)).transpose()?;

    let mut builder = Client::builder(server.as_str()).filter(filter).event_capacity(4096);
    if let Some(p) = &proxy {
        builder = builder.proxy(p.clone());
    }
    builder = match name.strip_prefix('@') {
        Some(account) => {
            let config = AuthConfig { proxy: proxy.as_ref().map(Socks5Proxy::to_url), ..AuthConfig::default() };
            let account = Account::new(Arc::new(AuthClient::new(config)?), Arc::new(FileTokenCache::new(".tokens")?), account);
            let (key, credentials) = account.login_credentials().await?;
            builder.online(credentials, key)
        }
        None => builder.offline(name.as_str()),
    };
    let mut client = builder.connect().await?;
    let deadline = tokio::time::sleep(Duration::from_secs(secs.parse()?));
    tokio::pin!(deadline);
    let mut n = 0usize;
    loop {
        tokio::select! {
            event = client.recv() => match event {
                Some(Event::Packet(p)) => {
                    let mut bytes = bytes::BytesMut::with_capacity(p.body.len() + 2);
                    acacia_client::proto::codec::write_varint(&mut bytes, p.id);
                    bytes.extend_from_slice(&p.body);
                    std::fs::write(outdir.join(format!("{n:06}_{}.bin", p.id)), bytes)?;
                    n += 1;
                }
                Some(Event::Disconnected(r)) => { println!("disconnected: {r:?}"); break; }
                None => break,
            },
            _ = &mut deadline => client.close(),
        }
    }
    println!("captured {n} packets to {}", outdir.display());
    Ok(())
}
