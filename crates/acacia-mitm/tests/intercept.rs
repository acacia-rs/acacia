//! An acacia client joins the fake server through the proxy over loopback RakNet: interceptors
//! rewrite and drop, the injector adds packets both ways, all through encryption and compression.

use std::time::Duration;

use acacia_client::{Client, Event, TransportKind};
use acacia_mitm::proto::packets::{SetTime, Text, TextCategory, TextContent, TextContentRaw, TextType};
use acacia_mitm::proto::{Packet, RawPacket};
use acacia_mitm::{Injector, Interceptor, Proxy, Verdict};
use acacia_testserver::{FakeServer, Script};
use tokio::sync::mpsc;

fn text(message: &str) -> Text {
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

fn message(packet: &Text) -> &str {
    match &packet.content {
        TextContent::Raw(c) => &c.message,
        other => panic!("{other:?}"),
    }
}

/// Redacts "secret" in the server's Text, drops SetTime, and signs the game's Text.
struct Censor;

impl Interceptor for Censor {
    fn on_server_packet(&mut self, packet: &RawPacket) -> Verdict {
        if packet.is::<SetTime>() {
            return Verdict::Drop;
        }
        match packet.decode::<Text>() {
            Ok(t) => Verdict::replace(&text(&message(&t).replace("secret", "[redacted]"))),
            Err(_) => Verdict::Forward,
        }
    }

    fn on_game_packet(&mut self, packet: &RawPacket) -> Verdict {
        match packet.decode::<Text>() {
            Ok(t) => Verdict::replace(&text(&format!("{} (via proxy)", message(&t)))),
            Err(_) => Verdict::Forward,
        }
    }
}

/// The next Text the client gets, failing on any SetTime before it.
async fn next_text(client: &mut Client) -> String {
    let wait = async {
        loop {
            match client.recv().await.expect("client still connected") {
                Event::Packet(p) if p.is::<SetTime>() => panic!("SetTime got through: {p:?}"),
                Event::Packet(p) if p.is::<Text>() => return message(&p.decode::<Text>().unwrap()).to_owned(),
                Event::Packet(_) | Event::Violation(_) => {}
                Event::Disconnected(reason) => panic!("disconnected: {reason:?}"),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(10), wait).await.expect("a Text within 10 s")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn proxy_rewrites_drops_and_injects_both_ways() {
    let server = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let (injectors_tx, mut injectors) = mpsc::unbounded_channel::<Injector>();
    let proxy = Proxy::new(server.addr())
        .listen("127.0.0.1:0".parse().unwrap())
        .intercept(move |session| {
            let _ = injectors_tx.send(session.injector.clone());
            Censor
        })
        .bind()
        .await
        .unwrap();
    let addr = proxy.local_addr().unwrap();
    let proxy = tokio::spawn(proxy.run());

    let builder = Client::builder(addr.to_string()).offline("MitmTester").transport(TransportKind::RakNet).subscribe([Text::ID, SetTime::ID]);
    let mut client = builder.connect().await.expect("client joins through the proxy");
    server.spawned().await.expect("script reaches PlayerSpawn");
    let injector = injectors.recv().await.expect("one session");

    server.send(&SetTime { time: 1234 });
    server.send(&text("the secret plan"));
    assert_eq!(next_text(&mut client).await, "the [redacted] plan");

    assert!(client.send(&text("hello")));
    assert_eq!(message(&server.recv::<Text>().await.unwrap()), "hello (via proxy)");

    assert!(injector.to_game(&text("from the proxy")));
    assert_eq!(next_text(&mut client).await, "from the proxy");
    assert!(injector.to_server(&text("also from the proxy")));
    assert_eq!(message(&server.recv::<Text>().await.unwrap()), "also from the proxy");

    client.close();
    proxy.abort();
}
