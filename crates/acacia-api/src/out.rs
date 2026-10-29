use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::{ApiError, BotId, CallId, ErrorCode, Event, Method};

/// One record of the outbound stream (ordering: see the crate docs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Out {
    /// `seq` counts the bot's events from 1, so a consumer can see a gap.
    Event { bot: BotId, seq: u64, event: Event },
    /// Call `call` ended; `result` is the method's result or why it failed.
    Done { bot: BotId, call: CallId, result: CallResult },
    /// The host dropped this many lossy events because nobody drained the stream.
    Lagged { dropped: u64 },
}

/// `{"ok": result}` or `{"err": ApiError}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum CallResult {
    Ok(Value),
    Err(ApiError),
}

impl CallResult {
    pub fn ok(value: &impl Serialize) -> Self {
        match serde_json::to_value(value) {
            Ok(value) => Self::Ok(value),
            Err(e) => Self::Err(ApiError::new(ErrorCode::Internal, format!("result did not serialize: {e}"))),
        }
    }
}

impl From<Result<Value, ApiError>> for CallResult {
    fn from(result: Result<Value, ApiError>) -> Self {
        result.map_or_else(Self::Err, Self::Ok)
    }
}

/// One call: `{"call": 1, "method": "bot.chat", "params": {...}}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Command {
    pub call: CallId,
    #[serde(flatten)]
    pub method: Method,
}

#[derive(Deserialize)]
struct Envelope {
    call: CallId,
    method: String,
    #[serde(default)]
    params: Value,
}

impl Command {
    /// Parses one command. The error carries the call id when that much could be read.
    pub fn from_json(json: &str) -> Result<Self, (Option<CallId>, ApiError)> {
        let bad = |e: serde_json::Error| ApiError::new(ErrorCode::BadParams, e.to_string());
        let envelope: Envelope = serde_json::from_str(json).map_err(|e| (None, bad(e)))?;
        let method = Method::parse(&envelope.method, envelope.params).map_err(|e| (Some(envelope.call), e))?;
        Ok(Self { call: envelope.call, method })
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("commands serialize")
    }
}

/// One batch of the stream as a JSON array, the unit a transport moves.
pub fn batch_json(records: &[Out]) -> String {
    serde_json::to_string(records).expect("records serialize")
}
