//! Joins an offline-mode server and counts packets: `java_join [host] [port] [seconds]`.
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use acacia_java_client::Client;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let host = args.next().unwrap_or_else(|| "127.0.0.1".into());
    let port = args.next().map_or(Ok(25565), |p| p.parse())?;
    let seconds = args.next().map_or(Ok(45), |s| s.parse())?;
    let mut client = Client::join_offline(&host, port, "AcaciaJava").await?;
    println!("joined as {} {:032x}", client.name, client.uuid);
    let (end, mut counts) = (Instant::now() + Duration::from_secs(seconds), BTreeMap::<i32, u32>::new());
    while let Ok(packet) = tokio::time::timeout_at(end.into(), client.next()).await {
        *counts.entry(packet?.id).or_default() += 1;
    }
    println!("still connected after {seconds} s; packets by id: {counts:?}");
    Ok(())
}
