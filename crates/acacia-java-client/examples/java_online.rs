//! Signs in with a Microsoft account and joins an online-mode server:
//! `java_online <token dir> [host] [port] [seconds]`.
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_auth::{Account, AuthClient, AuthConfig, FileTokenCache, Title};
use acacia_java_client::Client;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let tokens = args.next().ok_or("usage: java_online <token dir> [host] [port] [seconds]")?;
    let host = args.next().unwrap_or_else(|| "127.0.0.1".into());
    let port = args.next().map_or(Ok(25565), |p| p.parse())?;
    let seconds = args.next().map_or(Ok(30), |s| s.parse())?;
    let config = AuthConfig { title: Title::NintendoSwitch, ..AuthConfig::default() };
    let account = Account::new(Arc::new(AuthClient::new(config)?), Arc::new(FileTokenCache::new(tokens)?), "java");
    if !account.is_signed_in().await? {
        account.sign_in(|p| println!("SIGN IN: open {} and enter {}", p.verification_uri, p.user_code)).await?;
    }
    let mut client = Client::join_online(&host, port, &account).await?;
    println!("joined as {} {:032x}", client.name, client.uuid);
    let end = Instant::now() + Duration::from_secs(seconds);
    while let Ok(packet) = tokio::time::timeout_at(end.into(), client.next()).await {
        packet?;
    }
    println!("still connected after {seconds} s: {} chunks, {} entities", client.world.chunk_count(), client.world.entities.len());
    Ok(())
}
