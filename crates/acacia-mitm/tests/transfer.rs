//! A Transfer from the first fake server sends the client back to the proxy, which dials the
//! second for it; the capture holds the two connections as two sessions.

use std::future::Future;
use std::net::SocketAddr;
use std::time::Duration;

use acacia_client::{Client, DisconnectReason, Event, TransportKind};
use acacia_mitm::proto::packets::Transfer;
use acacia_mitm::proto::Packet;
use acacia_mitm::{Proxy, Recorder};
use acacia_testserver::{capture, FakeServer, Script};

async fn soon<T>(what: &str, wait: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), wait).await.unwrap_or_else(|_| panic!("{what} within 10 s"))
}

async fn join(proxy: SocketAddr) -> Client {
    let builder = Client::builder(proxy.to_string()).offline("MitmTester").transport(TransportKind::RakNet).subscribe([Transfer::ID]);
    builder.connect().await.expect("client joins through the proxy")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_transfer_is_followed_through_the_proxy() {
    let lobby = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let game = FakeServer::start(Script::bds_spawn()).await.unwrap();
    let dir = std::env::temp_dir().join(format!("acacia-mitm-transfer-{}", std::process::id()));
    let rec = Recorder::create(&dir, "capture").unwrap();
    let capture = rec.path().to_owned();
    let proxy = Proxy::new(lobby.addr()).listen("127.0.0.1:0".parse().unwrap()).record(rec).bind().await.unwrap();
    let addr = proxy.local_addr().unwrap();
    let proxy = tokio::spawn(proxy.run());

    let mut client = join(addr).await;
    soon("the lobby's spawn", lobby.spawned()).await.unwrap();
    lobby.send(&Transfer { server_address: "127.0.0.1".into(), port: game.addr().port(), reload_world: false, gatherings_configuration: None });
    let sent_to = soon("the transfer", async {
        loop {
            match client.recv().await.expect("client still connected") {
                Event::Disconnected(DisconnectReason::Transfer { address, port }) => return format!("{address}:{port}"),
                Event::Disconnected(reason) => panic!("disconnected: {reason:?}"),
                Event::Packet(_) | Event::Violation(_) => {}
            }
        }
    })
    .await;
    assert_eq!(sent_to, addr.to_string(), "the game is told to rejoin the proxy");

    let client = join(addr).await;
    soon("the second server's spawn", game.spawned()).await.unwrap();
    client.close();
    proxy.abort();

    let text = std::fs::read_to_string(&capture).unwrap();
    assert!(text.contains(r#""event":"transfer","session":0,"t":"#), "the transfer is noted on the first session");
    assert!(text.contains(&format!(r#""to":"127.0.0.1:{}""#, game.addr().port())), "with the server's own target");
    assert_eq!(capture::read(capture.to_str().unwrap()).unwrap().len(), 2);
    std::fs::remove_dir_all(&dir).unwrap();
}
