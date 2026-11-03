//! JSON-RPC 2.0 framing: parsing requests, writing responses and `out` notifications (codes: the crate docs).

use acacia_api::{ApiError, ErrorCode};
use serde::Deserialize;
use serde_json::{json, Value};

/// One request (or client notification, without `id`).
#[derive(Debug)]
pub(crate) struct Request {
    /// `None` for a notification: it runs, and nothing answers it.
    pub id: Option<Value>,
    pub method: String,
    pub params: Value,
}

#[derive(Deserialize)]
struct Envelope {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// A frame that is no request: what to answer, to `id` `null` unless the id could be read.
#[derive(Debug)]
pub(crate) struct Malformed {
    pub id: Value,
    pub rpc: i64,
    pub message: String,
}

impl Request {
    pub(crate) fn parse(text: &str) -> Result<Self, Malformed> {
        let value: Value = serde_json::from_str(text)
            .map_err(|e| Malformed { id: Value::Null, rpc: -32700, message: format!("parse error: {e}") })?;
        if value.is_array() {
            return Err(Malformed { id: Value::Null, rpc: -32600, message: "batches are not supported".into() });
        }
        let id = value.get("id").cloned().unwrap_or(Value::Null);
        let invalid = |message: String| Malformed { id: id.clone(), rpc: -32600, message };
        let envelope: Envelope = serde_json::from_value(value.clone()).map_err(|e| invalid(format!("invalid request: {e}")))?;
        if envelope.jsonrpc != "2.0" {
            return Err(invalid("jsonrpc must be \"2.0\"".into()));
        }
        if envelope.id.as_ref().is_some_and(|id| !(id.is_string() || id.is_number() || id.is_null())) {
            return Err(invalid("id must be a string, a number or null".into()));
        }
        Ok(Self { id: envelope.id, method: envelope.method, params: envelope.params })
    }
}

/// The JSON-RPC code for an [`ErrorCode`]; clients branch on `error.data.code` instead.
pub(crate) fn rpc_code(code: ErrorCode) -> i64 {
    match code {
        ErrorCode::Timeout => -32001,
        ErrorCode::Disconnected => -32002,
        ErrorCode::Rejected => -32003,
        ErrorCode::NotPossible => -32004,
        ErrorCode::NotVisible => -32005,
        ErrorCode::Cancelled => -32010,
        ErrorCode::Busy => -32020,
        ErrorCode::UnknownBot => -32021,
        ErrorCode::Closed => -32022,
        ErrorCode::Resolve => -32030,
        ErrorCode::Io => -32031,
        ErrorCode::NetherNet => -32032,
        ErrorCode::VersionMismatch => -32040,
        ErrorCode::Unauthorized => -32041,
        ErrorCode::UnknownMethod => -32601,
        ErrorCode::BadParams => -32602,
        ErrorCode::Internal => -32603,
        _ => -32000,
    }
}

pub(crate) fn result(id: &Value, result: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{result}}}"#)
}

pub(crate) fn error(id: &Value, error: &ApiError) -> String {
    error_with(id, rpc_code(error.code), error)
}

pub(crate) fn error_with(id: &Value, rpc: i64, error: &ApiError) -> String {
    let body = json!({ "code": rpc, "message": error.message, "data": { "code": error.code } });
    json!({ "jsonrpc": "2.0", "id": id, "error": body }).to_string()
}

pub(crate) fn malformed(m: &Malformed) -> String {
    error_with(&m.id, m.rpc, &ApiError::new(ErrorCode::BadParams, m.message.clone()))
}

/// One `out` notification carrying `records` (each already JSON).
pub(crate) fn out(records: &[String]) -> String {
    let mut frame = String::from(r#"{"jsonrpc":"2.0","method":"out","params":{"records":["#);
    for (i, record) in records.iter().enumerate() {
        if i > 0 {
            frame.push(',');
        }
        frame.push_str(record);
    }
    frame.push_str("]}}");
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_json_rpc() {
        let request = Request::parse(r#"{"jsonrpc":"2.0","id":3,"method":"bot.chat","params":{"bot":"bot-1"}}"#).unwrap();
        assert_eq!((request.id, request.method.as_str()), (Some(json!(3)), "bot.chat"));
        assert!(Request::parse(r#"{"jsonrpc":"2.0","method":"cancel"}"#).unwrap().id.is_none());
        assert_eq!(Request::parse("{").err().unwrap().rpc, -32700);
        assert_eq!(Request::parse("[]").err().unwrap().rpc, -32600);
        let bad = Request::parse(r#"{"jsonrpc":"1.0","id":"a","method":"x"}"#).err().unwrap();
        assert_eq!((bad.rpc, bad.id), (-32600, json!("a")));
        let records = [r#"{"type":"lagged","dropped":1}"#.to_owned()];
        let frame: Value = serde_json::from_str(&out(&records)).unwrap();
        assert_eq!(frame["params"]["records"][0]["dropped"], 1);
        let e: Value = serde_json::from_str(&error(&json!(1), &ApiError::new(ErrorCode::Cancelled, "x"))).unwrap();
        assert_eq!((e["error"]["code"].as_i64(), e["error"]["data"]["code"].as_str()), (Some(-32010), Some("CANCELLED")));
    }
}
