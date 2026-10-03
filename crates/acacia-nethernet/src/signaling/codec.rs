//! JSON frames of the two signaling-service flavours (docs/research/nethernet-signaling.md §2c-2d).

use rand_core::{OsRng, RngCore};
use serde_json::{json, Value};

use crate::Signal;

pub(super) const METHOD_SEND: &str = "Signaling_SendClientMessage_v1_0";
pub(super) const METHOD_WEBRTC: &str = "Signaling_WebRtc_v1_0";
pub(super) const METHOD_RECEIVE: &str = "Signaling_ReceiveMessage_v1_0";
pub(super) const METHOD_TURN_AUTH: &str = "Signaling_TurnAuth_v1_0";
pub(super) const METHOD_PING: &str = "System_Ping_v1_0";

/// Legacy `Type` values.
pub(super) const LEGACY_PING: u64 = 0;
pub(super) const LEGACY_SIGNAL: u64 = 1;
pub(super) const LEGACY_CREDENTIALS: u64 = 2;

/// What one received frame means.
#[derive(Debug, PartialEq)]
pub(super) enum Inbound {
    Signal { from: String, signal: Signal },
    Credentials(Value),
    /// A JSON-RPC response to our request `id`.
    Response { id: String, result: Result<Value, String> },
    /// A server error report (legacy `Type 0` with a body).
    ServerError(String),
    /// A JSON-RPC request the server wants answered with `result: null`. The id is echoed as
    /// received: the service sends numeric ids and closes the socket on a string echo.
    Ack(Value),
    Ignored,
}

pub(super) fn uuid() -> String {
    let mut b = [0u8; 16];
    OsRng.fill_bytes(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

pub(super) fn rpc_request(id: &str, method: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

pub(super) fn rpc_ack(id: &Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "result": null}).to_string()
}

/// `toPlayerId` is the peer's id as a string; the inner message names our NetherNet id.
pub(super) fn rpc_signal(id: &str, to: &str, own_network_id: u64, signal: &Signal) -> String {
    let inner = json!({
        "jsonrpc": "2.0",
        "method": METHOD_WEBRTC,
        "params": {"netherNetId": own_network_id.to_string(), "message": signal.to_string()},
    });
    rpc_request(id, METHOD_SEND, json!({"toPlayerId": to, "messageId": uuid(), "message": inner.to_string()}))
}

/// Legacy `To` is a JSON number when the id is numeric.
pub(super) fn legacy_signal(to: &str, signal: &Signal) -> String {
    let to = to.parse::<u64>().map_or_else(|_| Value::from(to), Value::from);
    json!({"Type": LEGACY_SIGNAL, "To": to, "Message": signal.to_string(), "MessageId": uuid()}).to_string()
}

pub(super) fn legacy_ping() -> String {
    json!({"Type": LEGACY_PING}).to_string()
}

/// Everything one frame carries (a JSON-RPC receive may batch several signals).
pub(super) fn decode(text: &str) -> Vec<Inbound> {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return vec![] };
    if v.get("jsonrpc").is_some() { decode_rpc(&v) } else { vec![decode_legacy(&v)] }
}

fn decode_legacy(v: &Value) -> Inbound {
    let message = v.get("Message").and_then(Value::as_str).unwrap_or_default();
    match v.get("Type").and_then(Value::as_u64) {
        Some(LEGACY_SIGNAL) => signal_from(id_string(v.get("From")), message),
        Some(LEGACY_CREDENTIALS) if id_string(v.get("From")).as_deref() == Some("Server") => {
            serde_json::from_str(message).map_or(Inbound::Ignored, Inbound::Credentials)
        }
        Some(LEGACY_PING) if !message.is_empty() => Inbound::ServerError(message.to_owned()),
        _ => Inbound::Ignored,
    }
}

fn decode_rpc(v: &Value) -> Vec<Inbound> {
    let raw_id = v.get("id").filter(|id| !id.is_null()).cloned();
    let id = id_string(v.get("id"));
    let mut out = match (v.get("method").and_then(Value::as_str), v.get("params")) {
        (Some(METHOD_RECEIVE), Some(Value::Array(items))) => items.iter().map(rpc_receive_item).collect(),
        (Some(METHOD_RECEIVE), Some(item)) => vec![rpc_receive_item(item)],
        (Some(_), _) => vec![],
        (None, _) => {
            let Some(id) = id else { return vec![] };
            let result = match v.get("error") {
                Some(e) if !e.is_null() => Err(e.get("message").and_then(Value::as_str).map_or_else(|| e.to_string(), str::to_owned)),
                _ => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
            };
            return vec![Inbound::Response { id, result }];
        }
    };
    out.extend(raw_id.map(Inbound::Ack));
    out
}

fn rpc_receive_item(item: &Value) -> Inbound {
    let from = id_string(item.get("From").or_else(|| item.get("from")));
    let message = item.get("Message").or_else(|| item.get("message")).and_then(Value::as_str).unwrap_or_default();
    // The message is a nested Signaling_WebRtc_v1_0 call, or bare signal text.
    match serde_json::from_str::<Value>(message) {
        Ok(inner) if inner.is_object() => {
            let params = inner.get("params").unwrap_or(&Value::Null);
            let text = params.get("message").and_then(Value::as_str).unwrap_or_default();
            signal_from(from.or_else(|| id_string(params.get("netherNetId"))), text)
        }
        _ => signal_from(from, message),
    }
}

fn signal_from(from: Option<String>, text: &str) -> Inbound {
    match (from, Signal::parse(text)) {
        (Some(from), Ok(signal)) => Inbound::Signal { from, signal },
        _ => Inbound::Ignored,
    }
}

fn id_string(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}
