//! Prints typed events and answers forms: buttons pick the last, modals say yes, custom forms send
//! their defaults, every 4th form is closed. Test server setup: tools/formtest-pack/README.md.
//! `cargo run -p acacia-bot --example events -- <server> <name> <seconds>`
use std::time::Duration;

use acacia_bot::client::Client;
use acacia_bot::events::ChatPattern;
use acacia_bot::forms::{FormKind, FormReply};
use acacia_bot::{Bot, BotConfig, BotEvent, Events};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19160".into());
    let name = args.next().unwrap_or_else(|| "Bot".into());
    let secs: u64 = args.next().map_or(60, |s| s.parse().expect("seconds"));

    let config = BotConfig {
        events: Events::ALL,
        chat_patterns: vec![ChatPattern::new("done", r"FORMTEST (\w+)")?],
        ..BotConfig::default()
    };
    let mut bot = Bot::connect(Client::builder(&server).offline(&name), config).await?;
    println!("spawned as {}", bot.client().display_name());

    let deadline = tokio::time::sleep(Duration::from_secs(secs));
    tokio::pin!(deadline);
    let mut forms = 0;
    loop {
        let event = tokio::select! {
            e = bot.next() => e,
            _ = &mut deadline => break,
        };
        match event {
            Some(BotEvent::Form(form)) => {
                forms += 1;
                println!("form {}: {:?}", form.id, form.kind);
                let reply = match &form.kind {
                    _ if forms % 4 == 0 => FormReply::Close,
                    FormKind::Simple { buttons, .. } => FormReply::Button(buttons.len().saturating_sub(1)),
                    FormKind::Modal { .. } => FormReply::Modal(true),
                    FormKind::Custom { .. } => FormReply::Custom(form.defaults()),
                };
                println!("  -> {reply:?}: {:?}", bot.answer_form(form.id, reply.clone()).await);
            }
            Some(BotEvent::Disconnected(r)) => {
                println!("disconnected: {r:?}");
                break;
            }
            Some(e) => println!("{e:?}"),
            None => break,
        }
    }
    bot.disconnect().await;
    Ok(())
}
