//! Lists the worlds friends are hosting, or joins one, idles, and reports what arrived.
//! `cargo run -p acacia-client --example friend_join -- <account>` lists them;
//! `cargo run -p acacia-client --example friend_join -- <account> <host gamertag or #> [seconds]` joins one.
//! Tokens come from ./.tokens (see acacia-auth's device_login).
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_client::{friend_builder, Event};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let mut args = std::env::args().skip(1);
    let Some(account) = args.next() else {
        return Err("usage: friend_join <account> [host gamertag or #] [seconds]".into());
    };
    let pick = args.next();
    let secs: u64 = args.next().map_or(30, |s| s.parse().expect("seconds"));
    let start = Instant::now();

    let auth = Arc::new(AuthClient::new(AuthConfig::default())?);
    let account = Account::new(auth.clone(), Arc::new(FileTokenCache::new(".tokens")?), account);
    let worlds = account.friend_worlds().await?;
    for (i, w) in worlds.iter().enumerate() {
        let kinds: Vec<_> = w.connections.iter().map(|c| c.kind).collect();
        println!(
            "#{i} {} hosts {:?} ({}/{} players, {} protocol {}, closed={}) via {kinds:?}",
            w.host_name, w.world_name, w.members, w.max_members, w.version, w.protocol, w.closed
        );
    }
    let Some(pick) = pick else { return Ok(()) };
    let world = match pick.strip_prefix('#') {
        Some(i) => worlds.get(i.parse::<usize>()?),
        None => worlds.iter().find(|w| w.host_name.eq_ignore_ascii_case(&pick)),
    };
    let world = world.ok_or("no such world")?;

    let builder = friend_builder(&auth, &account, world, None).await?;
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
    // Lets the dropped connection's leave reach MPSD before the runtime stops.
    drop(client);
    tokio::time::sleep(Duration::from_secs(2)).await;
    Ok(())
}
