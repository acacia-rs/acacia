use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::*;
use crate::turn::IceServers;
use crate::SignalKind;

fn frame(s: &mut SignalingSession) -> Value {
    serde_json::from_str(&s.poll_text().expect("a frame")).unwrap()
}

#[test]
fn jsonrpc_requests_turn_auth_and_surfaces_credentials() {
    let now = Instant::now();
    let mut s = SignalingSession::new(SignalingProtocol::JsonRpc, 42, DEFAULT_PING_INTERVAL, now);
    let req = frame(&mut s);
    assert_eq!((req["jsonrpc"].as_str(), req["method"].as_str()), (Some("2.0"), Some("Signaling_TurnAuth_v1_0")));
    let creds = json!({"ExpirationInSeconds": 3600, "TurnAuthServers": [{"Username": "u", "Password": "p", "Urls": ["turn:1.2.3.4:3478"]}]});
    s.handle_text(&json!({"jsonrpc": "2.0", "id": req["id"], "result": creds}).to_string());
    assert_eq!(s.poll_event(), Some(SignalingEvent::Credentials(IceServers::from_json(&creds).unwrap())));
    assert!(s.poll_text().is_none());
}

#[test]
fn jsonrpc_signal_wraps_webrtc_call_with_our_id() {
    let now = Instant::now();
    let mut s = SignalingSession::new(SignalingProtocol::JsonRpc, 42, DEFAULT_PING_INTERVAL, now);
    s.poll_text();
    s.send_signal("realm-peer", &Signal::new(SignalKind::ConnectRequest, 9, "v=0"), now);
    let f = frame(&mut s);
    assert_eq!(f["method"], "Signaling_SendClientMessage_v1_0");
    assert_eq!(f["params"]["toPlayerId"], "realm-peer");
    let inner: Value = serde_json::from_str(f["params"]["message"].as_str().unwrap()).unwrap();
    assert_eq!(inner["method"], "Signaling_WebRtc_v1_0");
    assert_eq!(inner["params"], json!({"netherNetId": "42", "message": "CONNECTREQUEST 9 v=0"}));
}

#[test]
fn jsonrpc_receive_batches_signals_and_acks() {
    let mut s = SignalingSession::new(SignalingProtocol::JsonRpc, 42, DEFAULT_PING_INTERVAL, Instant::now());
    s.poll_text();
    let wrapped = json!({"jsonrpc": "2.0", "method": "Signaling_WebRtc_v1_0", "params": {"netherNetId": "7", "message": "CONNECTRESPONSE 9 v=0"}}).to_string();
    let frame_in = json!({"jsonrpc": "2.0", "id": "srv-1", "method": "Signaling_ReceiveMessage_v1_0", "params": [
        {"From": "peer", "Message": wrapped},
        {"From": "peer", "Message": "CANDIDATEADD 9 candidate:1 1 udp 1 1.2.3.4 5 typ host"},
    ]});
    s.handle_text(&frame_in.to_string());
    let kinds: Vec<SignalKind> = std::iter::from_fn(|| s.poll_event())
        .map(|e| match e {
            SignalingEvent::Signal { from, signal } => {
                assert_eq!(from, "peer");
                signal.kind
            }
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(kinds, [SignalKind::ConnectResponse, SignalKind::CandidateAdd]);
    assert_eq!(frame(&mut s), json!({"jsonrpc": "2.0", "id": "srv-1", "result": null}));

    // The live service numbers its requests and drops the socket if the echo is a string.
    let delivery = json!({"jsonrpc": "2.0", "method": "Signaling_DeliveryNotification_V1_0", "params": {"messageId": "m"}}).to_string();
    s.handle_text(&json!({"jsonrpc": "2.0", "id": 2, "method": "Signaling_ReceiveMessage_v1_0", "params": {"From": "peer", "Message": delivery}}).to_string());
    assert_eq!(s.poll_event(), None);
    assert_eq!(frame(&mut s), json!({"jsonrpc": "2.0", "id": 2, "result": null}));
}

#[test]
fn legacy_signals_credentials_errors_and_pings() {
    let now = Instant::now();
    let mut s = SignalingSession::new(SignalingProtocol::Legacy, 42, Duration::from_secs(15), now);
    assert!(s.poll_text().is_none());
    s.send_signal("1234", &Signal::error(9, 2), now);
    let f = frame(&mut s);
    assert_eq!((f["Type"].as_u64(), f["To"].as_u64(), f["Message"].as_str()), (Some(1), Some(1234), Some("CONNECTERROR 9 2")));

    s.handle_text(r#"{"Type":1,"From":1234,"Message":"CONNECTRESPONSE 9 v=0"}"#);
    s.handle_text(r#"{"Type":2,"From":"Server","Message":"{\"ExpirationInSeconds\":60,\"TurnAuthServers\":[]}"}"#);
    s.handle_text(r#"{"Type":2,"From":"1234","Message":"{\"ExpirationInSeconds\":60,\"TurnAuthServers\":[]}"}"#);
    s.handle_text(r#"{"Type":0,"Message":"{\"Code\":1,\"Message\":\"PlayerNotFound\"}"}"#);
    let events: Vec<_> = std::iter::from_fn(|| s.poll_event()).collect();
    assert!(matches!(&events[0], SignalingEvent::Signal { from, .. } if from == "1234"));
    assert_eq!(events[1], SignalingEvent::Credentials(IceServers { expires_in: Duration::from_secs(60), servers: vec![] }));
    assert!(matches!(&events[2], SignalingEvent::Error(e) if e.contains("PlayerNotFound")));
    assert_eq!(events.len(), 3);

    assert_eq!(s.poll_timeout(), now + Duration::from_secs(15));
    s.handle_timeout(now + Duration::from_secs(15));
    assert_eq!(frame(&mut s), json!({"Type": 0}));
}

#[test]
fn unanswered_requests_time_out() {
    let now = Instant::now();
    let mut s = SignalingSession::new(SignalingProtocol::JsonRpc, 42, Duration::from_secs(60), now);
    s.poll_text();
    assert_eq!(s.poll_timeout(), now + Duration::from_secs(15));
    s.handle_timeout(now + Duration::from_secs(15));
    assert_eq!(s.poll_event(), Some(SignalingEvent::Error("Signaling_TurnAuth_v1_0 timed out".into())));
}

#[test]
fn urls() {
    assert_eq!(SignalingProtocol::Legacy.url("signal.x", 5), "wss://signal.x/ws/v1.0/signaling/5");
    assert_eq!(SignalingProtocol::JsonRpc.url("signal.x", 5), "wss://signal.x/ws/v1.0/messaging/connect");
}
