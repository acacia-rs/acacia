//! Runs commands as an operator account, for players BDS will not op (offline players; `/op` is
//! console-only), then disconnects. Wrap a command in `execute as @a[rm=2] at @s run ...` to run it at
//! every other player.
//! `cargo run -p acacia-bot --example cmd -- <server> <@account> <command>...`
use std::sync::Arc;

use acacia_bot::client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_bot::client::Client;
use acacia_bot::{Bot, BotConfig};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(server), Some(name)) = (args.next(), args.next()) else {
        return Err("usage: cmd <server> <@account> <command>...".into());
    };
    let account = name.strip_prefix('@').ok_or("the account must be an online one (@name)")?;
    let account = Account::new(Arc::new(AuthClient::new(AuthConfig::default())?), Arc::new(FileTokenCache::new(".tokens")?), account);
    let (key, credentials) = account.login_credentials().await?;
    let mut bot = Bot::connect(Client::builder(&server).online(credentials, key), BotConfig::default()).await?;
    bot.wait_ticks(40).await?;
    for command in args {
        println!("{command}");
        bot.client().command(&command);
        bot.wait_ticks(4).await?;
    }
    bot.wait_ticks(20).await?;
    bot.disconnect().await;
    Ok(())
}
