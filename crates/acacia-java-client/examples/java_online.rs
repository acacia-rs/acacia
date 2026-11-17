//! Signs in with a Microsoft account and joins an online-mode server:
//! `java_online <token dir> [host] [port] [seconds] [chat]`. With `chat`, 5 s in it says a line and sends `/msg`
//! to itself, signed when the server takes a chat session, then fetches the profile key again and repeats both.
//! `ACACIA_NO_SIGN_IN=1` fails instead of starting a
//! device-code sign-in when the token dir has no account.
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_auth::{Account, AuthClient, AuthConfig, FileTokenCache, Title};
use acacia_java_client::Client;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let tokens = args.next().ok_or("usage: java_online <token dir> [host] [port] [seconds] [chat]")?;
    let host = args.next().unwrap_or_else(|| "127.0.0.1".into());
    let port = args.next().map_or(Ok(25565), |p| p.parse())?;
    let seconds = args.next().map_or(Ok(30), |s| s.parse())?;
    let mut chat = args.next().as_deref() == Some("chat");
    let config = AuthConfig { title: Title::NintendoSwitch, ..AuthConfig::default() };
    let account = Account::new(Arc::new(AuthClient::new(config)?), Arc::new(FileTokenCache::new(tokens)?), "java");
    if !account.is_signed_in().await? {
        if std::env::var_os("ACACIA_NO_SIGN_IN").is_some() {
            return Err("not signed in (ACACIA_NO_SIGN_IN set)".into());
        }
        account.sign_in(|p| println!("SIGN IN: open {} and enter {}", p.verification_uri, p.user_code)).await?;
    }
    let mut client = Client::join_online(&host, port, &account).await?;
    println!("joined as {} {:032x}", client.name, client.uuid);
    if let Some(e) = client.profile_key_error() {
        println!("no profile key: {e}");
    }
    let end = Instant::now() + Duration::from_secs(seconds);
    // Nothing ticks here, so `loaded()` never turns true: chat a few seconds in instead.
    let chat_at = Instant::now() + Duration::from_secs(5);
    let mut refreshed = None;
    while let Ok(packet) = tokio::time::timeout_at(end.into(), client.next()).await {
        packet?;
        client.tick_profile_key()?;
        if chat && Instant::now() >= chat_at {
            chat = false;
            println!("chat signed: {}", client.signs_chat());
            client.chat("hello from acacia, signed")?;
            client.command(&format!("msg {} a signed whisper", client.name))?;
            refreshed = Some((client.chat_session(), Instant::now() + Duration::from_secs(5)));
            client.refresh_profile_key();
        }
        if let Some((session, at)) = refreshed
            && Instant::now() >= at
            && !client.refreshing_profile_key()
        {
            refreshed = None;
            println!("key refreshed, new session: {}", client.chat_session() != session);
            client.chat("hello again, after the key refresh")?;
            client.command(&format!("msg {} a whisper after the refresh", client.name))?;
        }
        if let Some(message) = client.message.take() {
            println!("message {:?}: {}", message.kind, message.text);
        }
    }
    println!("still connected after {seconds} s: {} chunks, {} entities", client.world.chunk_count(), client.world.entities.len());
    Ok(())
}
