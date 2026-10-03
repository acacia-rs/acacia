//! Lists LAN worlds and joins the first (or the one whose name contains `filter`), idles, reports.
//! `cargo run -p acacia-client --example lan_join -- <account> [filter] [seconds]`
//! Tokens come from ./.tokens (see acacia-auth's device_login). Host a world with LAN visibility on.
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_client::{discover_lan, Client, Event};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let mut args = std::env::args().skip(1);
    let account = args.next().ok_or("usage: lan_join <account> [filter] [seconds]")?;
    let filter = args.next().unwrap_or_default();
    let secs: u64 = args.next().map_or(30, |s| s.parse().expect("seconds"));
    let start = Instant::now();

    let servers = discover_lan(Duration::from_secs(4)).await?;
    for s in &servers {
        let d = &s.data;
        println!("{} id={} {:?} / {:?} {} players {}/{} protocol {}", s.addr, s.network_id, d.server_name, d.level_name, d.version, d.player_count, d.max_player_count, d.protocol);
    }
    let server = servers.into_iter().find(|s| s.data.server_name.contains(&filter) || s.data.level_name.contains(&filter)).ok_or("no LAN world found")?;

    let auth = Arc::new(AuthClient::new(AuthConfig::default())?);
    let account = Account::new(auth, Arc::new(FileTokenCache::new(".tokens")?), account);
    let (key, credentials) = account.login_credentials().await?;
    let mut client = Client::builder(server.addr.to_string()).online(credentials, key).lan(server).connect().await?;
    println!("spawned as {} in {:?}", client.display_name(), start.elapsed());

    let deadline = tokio::time::sleep(Duration::from_secs(secs));
    tokio::pin!(deadline);
    let mut packets = 0usize;
    loop {
        tokio::select! {
            event = client.recv() => match event {
                Some(Event::Packet(_)) => packets += 1,
                Some(Event::Disconnected(reason)) => {
                    println!("disconnected after {:?}: {reason:?}", start.elapsed());
                    break;
                }
                None => break,
            },
            _ = &mut deadline => client.close(),
        }
    }
    println!("{packets} packets");
    Ok(())
}
