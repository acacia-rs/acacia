//! Java bot answering dialogs as forms. It must be an operator: it shows itself each dialog of
//! `tools/gate-datapack` (`function acacia:dialog_<name>`), then answers it. In the server's log of player commands:
//! the confirmation answered yes runs `tellraw @a "yes"`, and the inputs dialog's template runs
//! `tellraw @a "acacia yes red 7"`. The menu's second button sends a custom click, which leaves no log line. Closing
//! the notice runs its button's `tellraw @a "closed"`; the registry dialog (`acacia:registry`) runs
//! `tellraw @a "registry"`.
//! `cargo run -p acacia-bot --features java --example java_dialogs -- <host> <port> [name]`

use std::time::Duration;

use acacia_bot::forms::{FormReply, FormValue};
use acacia_bot::{Bot, BotConfig};

const SHOW_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [host, port] = &args[..2] else { return Err("usage: java_dialogs <host> <port> [name]".into()) };
    let name = args.get(2).map_or("Reader", String::as_str);
    let mut bot = Bot::connect_java_offline(host, port.parse()?, name, BotConfig::default()).await?;
    bot.wait_ticks(100).await?;

    let input_values = vec![FormValue::Text("acacia".into()), FormValue::Toggle(true), FormValue::Choice(0), FormValue::Slider(6.6)];
    let steps = [
        ("confirm", FormReply::Modal(true)),
        ("inputs", FormReply::Dialog { button: 0, values: input_values }),
        ("multi", FormReply::Button(1)),
        ("notice", FormReply::Close),
        ("registry", FormReply::Button(0)),
    ];
    for (dialog, reply) in steps {
        bot.command(&format!("function acacia:dialog_{dialog}"));
        let form = match bot.wait_form(SHOW_TIMEOUT).await {
            Ok(form) => form,
            Err(e) => {
                println!("{dialog}: no dialog: {e:?}");
                continue;
            }
        };
        println!("form {} {:?}: {:?}", form.id, form.title, form.kind);
        println!("  -> {reply:?}: {:?}", bot.answer_form(form.id, reply.clone()).await);
        bot.wait_ticks(10).await?;
    }
    bot.disconnect().await;
    Ok(())
}
