//! Connects N offline bots concurrently, idles, and reports spawn latency and failures.
//! `cargo run --release -p acacia-client --example swarm -- 127.0.0.1:19140 20 15`
use std::time::{Duration, Instant};

use acacia_client::{Client, Event};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let bots: usize = args.next().map_or(10, |s| s.parse().expect("bot count"));
    let secs: u64 = args.next().map_or(10, |s| s.parse().expect("seconds"));

    let tasks: Vec<_> = (0..bots)
        .map(|i| {
            let server = server.clone();
            tokio::spawn(async move {
                let start = Instant::now();
                // Tiny capacity on purpose: exercises backpressure while the login burst arrives.
                let mut client = Client::builder(server).offline(format!("Swarm{i}")).event_capacity(4).blob_cache_dir(".blobs").connect().await?;
                let spawn = start.elapsed();
                let idle_until = tokio::time::Instant::now() + Duration::from_secs(secs);
                let mut packets = 0usize;
                loop {
                    tokio::select! {
                        e = client.recv() => match e {
                            Some(Event::Packet(_)) => packets += 1,
                            Some(Event::Disconnected(r)) => return Err(format!("dropped early: {r:?}").into()),
                            None => return Err("event stream ended".into()),
                        },
                        _ = tokio::time::sleep_until(idle_until) => break,
                    }
                }
                client.close();
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>((spawn, packets))
            })
        })
        .collect();

    let mut spawns = Vec::new();
    let mut failures = 0;
    for (i, t) in tasks.into_iter().enumerate() {
        match t.await.expect("task panicked") {
            Ok((spawn, _)) => spawns.push(spawn),
            Err(e) => {
                failures += 1;
                eprintln!("bot {i}: {e}");
            }
        }
    }
    spawns.sort();
    let pct = |p: usize| spawns.get((spawns.len() * p / 100).min(spawns.len().saturating_sub(1))).copied().unwrap_or_default();
    println!("{} ok, {failures} failed | spawn p50 {:?} p95 {:?} max {:?}", spawns.len(), pct(50), pct(95), spawns.last().copied().unwrap_or_default());
}
