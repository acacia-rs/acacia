//! Joins a realm the account is a member of, idles, and reports what arrived.
//! `cargo run -p acacia-client --example realm_join -- <account> <realm id> [seconds]`
//! Tokens come from ./.tokens (see acacia-auth's device_login); list realm ids with its realms_list.
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_client::{realm_builder, Event};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let mut args = std::env::args().skip(1);
    let (Some(account), Some(realm)) = (args.next(), args.next()) else {
        return Err("usage: realm_join <account> <realm id> [seconds]".into());
    };
    let secs: u64 = args.next().map_or(30, |s| s.parse().expect("seconds"));
    let start = Instant::now();

    let auth = Arc::new(AuthClient::new(AuthConfig::default())?);
    let account = Account::new(auth.clone(), Arc::new(FileTokenCache::new(".tokens")?), account);
    let builder = realm_builder(&auth, &account, realm.parse()?, None).await?;
    println!("join target: {} ({:?})", builder.server(), start.elapsed());

    let mut client = builder.connect().await?;
    println!("spawned as {} in {:?}", client.display_name(), start.elapsed());
    let deadline = tokio::time::sleep(Duration::from_secs(secs));
    tokio::pin!(deadline);
    let mut packets = 0usize;
    loop {
        tokio::select! {
            event = client.recv() => match event {
                Some(Event::Packet(_)) => packets += 1,
                Some(Event::Violation(_)) => {}
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
