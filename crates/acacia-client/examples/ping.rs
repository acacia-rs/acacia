//! Pings servers without logging in, optionally through the SOCKS5 proxy in `BEDROCK_PROXY`
//! (`host:port[:user:pass]` or `socks5://...`).
//! `cargo run -p acacia-client --example ping -- play.example.net`
use acacia_client::{ping, Socks5Proxy};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proxy = std::env::var("BEDROCK_PROXY").ok().map(|p| Socks5Proxy::parse(&p)).transpose()?;
    if let Some(p) = &proxy {
        println!("via proxy {}:{}", p.host, p.port);
    }
    for server in std::env::args().skip(1) {
        match ping(&server, proxy.as_ref()).await {
            Ok(s) => println!(
                "{server:<28} {:>4}ms  {} ({} / {})  {}/{} players",
                s.latency.as_millis(),
                s.motd,
                s.version,
                s.protocol,
                s.players_online,
                s.players_max
            ),
            Err(e) => println!("{server:<28} failed: {e}"),
        }
    }
    Ok(())
}
