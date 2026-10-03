//! Joins a realm the account is a member of, idles, and reports what arrived.
//! `cargo run -p acacia-client --example realm_join -- <account> <realm id> [seconds]`
//! Tokens come from ./.tokens (see acacia-auth's device_login); list realm ids with its realms_list.
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_client::auth::{Account, AuthClient, AuthConfig, FileTokenCache, RealmProtocol};
use acacia_client::{measure_ping_regions, Client, Event, SignalingTarget};

const DEFAULT_SIGNALING_HOST: &str = "signal.franchise.minecraft-services.net";

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
    let ping_regions = measure_ping_regions(&auth.qos_beacons().await?, None).await?;
    println!("pinged {} regions ({:?})", ping_regions.len(), start.elapsed());
    let join = account.join_realm(realm.parse()?, &ping_regions).await?;
    println!("join target: {:?} {} region {:?} ({:?})", join.protocol, join.address, join.region, start.elapsed());
    let (key, credentials) = account.login_credentials().await?;
    let mut builder = Client::builder(&join.address).online(credentials, key);
    if join.protocol != RealmProtocol::RakNet {
        let token = account.service_token().await?.authorization_header;
        let host = auth.signaling_environment().await?.map_or_else(|| DEFAULT_SIGNALING_HOST.to_owned(), |env| env.service_uri);
        let target = SignalingTarget::from_realm(&join, token, &host).ok_or("unsupported realm protocol")?;
        println!("signaling via {} ({:?})", target.host, target.protocol);
        builder = builder.signaling(target);
    }

    let mut client = builder.connect().await?;
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
