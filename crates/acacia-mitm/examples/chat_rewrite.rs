//! A modifying proxy in front of the local test BDS: upper-cases chat from the server, freezes the
//! game's clock by dropping SetTime, and answers `!ping` typed in the game itself, without the
//! server ever seeing it.
//!
//! `cargo run -p acacia-mitm --example chat_rewrite -- [server 127.0.0.1:19140]`, then join
//! this machine on port 19180 from the game.

use acacia_mitm::proto::packets::{SetTime, Text, TextCategory, TextContent, TextContentRaw, TextType};
use acacia_mitm::proto::RawPacket;
use acacia_mitm::{Injector, Interceptor, Proxy, Verdict};

struct ChatRewrite {
    injector: Injector,
}

impl Interceptor for ChatRewrite {
    fn on_server_packet(&mut self, packet: &RawPacket) -> Verdict {
        if packet.is::<SetTime>() {
            return Verdict::Drop;
        }
        let Ok(mut text) = packet.decode::<Text>() else { return Verdict::Forward };
        match &mut text.content {
            TextContent::Chat(c) => c.message = c.message.to_uppercase(),
            TextContent::Raw(c) => c.message = c.message.to_uppercase(),
            _ => return Verdict::Forward,
        }
        Verdict::replace(&text)
    }

    fn on_game_packet(&mut self, packet: &RawPacket) -> Verdict {
        let Ok(text) = packet.decode::<Text>() else { return Verdict::Forward };
        let TextContent::Chat(chat) = &text.content else { return Verdict::Forward };
        if chat.message.trim() != "!ping" {
            return Verdict::Forward;
        }
        self.injector.to_game(&raw_text("pong (from acacia-mitm)"));
        Verdict::Drop
    }
}

fn raw_text(message: &str) -> Text {
    Text {
        needs_translation: false,
        category: TextCategory::MessageOnly,
        r#type: TextType::Raw,
        content: TextContent::Raw(TextContentRaw { message: message.into() }),
        xuid: String::new(),
        platform_chat_id: String::new(),
        has_filtered_message: false,
        filtered_message: None,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:19140".into()).parse()?;
    let proxy = Proxy::new(server).intercept(|session| ChatRewrite { injector: session.injector.clone() }).bind().await?;
    println!("join {} from the game", proxy.local_addr()?);
    Ok(proxy.run().await?)
}
