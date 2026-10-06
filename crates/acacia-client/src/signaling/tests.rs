//! `dial` against a fake JSON-RPC signaling service in front of a str0m host.

use std::time::Duration;

use acacia_nethernet::{Identity, Signal};
use futures_util::{SinkExt, StreamExt};
use p384::ecdsa::SigningKey;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use super::*;
use crate::test_host::RtcHost;

const HOST_ID: &str = "fake-host";

/// The text frame delivering `signal` from the host, as the service relays it.
fn delivery(signal: &Signal) -> Message {
    let inner = json!({"jsonrpc": "2.0", "method": "Signaling_WebRtc_v1_0", "params": {"netherNetId": "1", "message": signal.to_string()}});
    let frame = json!({"jsonrpc": "2.0", "id": 7, "method": "Signaling_ReceiveMessage_v1_0", "params": [{"From": HOST_ID, "Message": inner.to_string()}]});
    Message::Text(frame.to_string().into())
}

async fn on_frame(ws: &mut WebSocketStream<tokio::net::TcpStream>, host: &mut RtcHost, text: &str) {
    let v: Value = serde_json::from_str(text).unwrap();
    let reply = |result: Value| Message::Text(json!({"jsonrpc": "2.0", "id": v["id"], "result": result}).to_string().into());
    match v["method"].as_str() {
        Some("Signaling_TurnAuth_v1_0") => {
            ws.send(reply(json!({"ExpirationInSeconds": 60, "TurnAuthServers": []}))).await.unwrap();
        }
        Some("Signaling_SendClientMessage_v1_0") => {
            assert_eq!(v["params"]["toPlayerId"], HOST_ID);
            ws.send(reply(Value::Null)).await.unwrap();
            let inner: Value = serde_json::from_str(v["params"]["message"].as_str().unwrap()).unwrap();
            let signal = Signal::parse(inner["params"]["message"].as_str().unwrap()).unwrap();
            for out in host.on_signal(&signal) {
                ws.send(delivery(&out)).await.unwrap();
            }
        }
        _ => {}
    }
}

#[cfg(feature = "online")]
#[test]
fn friend_targets_follow_the_connection_type() {
    use acacia_auth::{ConnectionKind as K, WorldConnection};
    let conn = |kind, pmsg: Option<&str>, id: Option<&str>| WorldConnection {
        kind,
        nethernet_id: id.map(str::to_owned),
        pmsg_id: pmsg.map(str::to_owned),
        host_ip: None,
        host_port: 0,
    };
    let host = "wss://signal.example.net/";
    let t = |c: &WorldConnection| SignalingTarget::from_friend(c, "MCToken x".into(), host).map(|t| (t.protocol, t.peer, t.host));
    let jsonrpc = conn(K::SignalingJsonRpc, Some("pmsg-uuid"), Some("42"));
    assert_eq!(t(&jsonrpc), Some((SignalingProtocol::JsonRpc, "pmsg-uuid".into(), "signal.example.net".into())));
    let no_pmsg = conn(K::SignalingJsonRpc, None, Some("42"));
    assert_eq!(t(&no_pmsg).map(|(p, peer, _)| (p, peer)), Some((SignalingProtocol::Legacy, "42".into())));
    assert_eq!(t(&conn(K::SignalingLegacy, None, Some("7"))).map(|(p, ..)| p), Some(SignalingProtocol::Legacy));
    assert_eq!(t(&conn(K::Lan, None, Some("7"))), None);
    assert_eq!(t(&conn(K::SignalingLegacy, None, None)), None);
}

/// Starts the fake service with a host behind it (which echoes what it is sent) and returns how to dial it.
async fn fake_service() -> (SignalingTarget, Identity) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut host = RtcHost::bind(true).await;
    tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
        loop {
            tokio::select! {
                frame = ws.next() => match frame {
                    Some(Ok(Message::Text(t))) => on_frame(&mut ws, &mut host, t.as_str()).await,
                    Some(Ok(_)) => {}
                    _ => return,
                },
                _ = host.step() => {}
            }
        }
    });
    let target = SignalingTarget {
        host: format!("ws://127.0.0.1:{port}"),
        protocol: SignalingProtocol::JsonRpc,
        peer: HOST_ID.into(),
        mc_token: "token".into(),
    };
    (target, Identity::multiplayer(SigningKey::from_slice(&[5; 48]).unwrap(), "token".into()))
}

#[tokio::test(flavor = "current_thread")]
async fn dials_through_a_fake_jsonrpc_host() {
    let (target, identity) = fake_service().await;
    let dialed = tokio::time::timeout(Duration::from_secs(20), dial(&target, &identity, None)).await;
    let dialed = dialed.expect("dial timed out").expect("dial failed");
    assert!(dialed.wire.conn.is_open());
    assert!(dialed.wire.turn().is_none(), "a reachable host needs no TURN");
}

#[tokio::test(flavor = "current_thread")]
async fn a_raw_link_carries_batches_both_ways() {
    let (target, identity) = fake_service().await;
    let link = tokio::time::timeout(Duration::from_secs(20), crate::RawLink::dial(&target, &identity, None)).await;
    let mut link = link.expect("dial timed out").expect("dial failed");
    // Larger than one WebRTC message, so it crosses the link in segments.
    let batch = bytes::Bytes::from(vec![0xfe; 30_000]);
    assert!(link.send(batch.clone()));
    let echoed = tokio::time::timeout(Duration::from_secs(10), link.recv()).await.expect("no echo within 10 s");
    assert_eq!(echoed, Some(batch));
}
