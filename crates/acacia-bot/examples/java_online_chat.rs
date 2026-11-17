//! Java bot on an online-mode server: joins as a cached Microsoft account, chats as soon as it is placed (as a script
//! would right after `createBot`), then again after 5 s, and prints the chat session as it changes. On a fresh server's
//! first join the server can refuse the first line (see docs/java-edition.md, Joining).
//! `cargo run -p acacia-bot --features java --example java_online_chat -- <token dir> <account id> <host> <port>`;
//! the account id is the cache entry (`<username>.java` for one the Node package signed in). Never signs in.

use std::sync::Arc;

use acacia_bot::{Bot, BotConfig};
use acacia_client::auth::{Account, AuthClient, AuthConfig, FileTokenCache, Title};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [tokens, id, host, port] = &args[..] else { return Err("usage: java_online_chat <token dir> <account id> <host> <port>".into()) };
    let client = AuthClient::new(AuthConfig { title: Title::NintendoSwitch, ..AuthConfig::default() })?;
    let account = Account::new(Arc::new(client), Arc::new(FileTokenCache::new(tokens)?), id.as_str());
    if !account.is_signed_in().await? {
        return Err(format!("{id} is not signed in").into());
    }
    let mut bot = Bot::connect_java(host, port.parse()?, &account, BotConfig::default()).await?;
    let session = |bot: &Bot| bot.java().and_then(|j| j.client().chat_session());
    let key_error = bot.java().and_then(|j| j.client().profile_key_error());
    println!("joined: session {:?}, key error {key_error:?}", session(&bot).map(|s| s as u16));
    bot.wait_ticks(1).await?;
    let mut last = session(&bot);
    for (tick, line) in [(0, "chat at once"), (100, "chat after 5 s")] {
        println!("tick {tick} {line}: session {:?}", last.map(|s| s as u16));
        bot.chat(line);
        for t in 1..=100 {
            bot.wait_ticks(1).await?;
            let now = session(&bot);
            if now != last {
                println!("tick {}: session {:?} -> {:?}", tick + t, last.map(|s| s as u16), now.map(|s| s as u16));
                last = now;
            }
        }
    }
    bot.disconnect().await;
    Ok(())
}
