//! Prints the built-in ("multi") recipes a server announces by UUID, and its recipes for an item.
//! `cargo run -p acacia-bot --example multi_recipes -- <server> [item, e.g. minecraft:shield]`
use acacia_bot::client::Client;
use acacia_bot::{Bot, BotConfig};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19174".into());
    let item = args.next().unwrap_or_else(|| "minecraft:shield".into());
    let mut bot = Bot::connect(Client::builder(&server).offline("RecipeReader"), BotConfig::default()).await?;
    bot.wait_ticks(60).await?;
    let book = bot.state().recipes.book();
    let mut multi: Vec<(&str, u32)> = book.multi_recipes().collect();
    multi.sort();
    for (uuid, id) in multi {
        println!("multi {uuid} = {id}");
    }
    for recipe in bot.recipes_for(item.parse()?) {
        println!("{item}: {} at {} ({:?}) from {:?}", recipe.id, recipe.block, recipe.kind, recipe.inputs);
    }
    Ok(())
}
