use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Why a call failed. `message` is free text for people; branch on `code`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum ErrorCode {
    /// The server did not answer in time.
    Timeout,
    /// The bot's connection ended (calls still queued fail with this too).
    Disconnected,
    /// The server refused the action.
    Rejected,
    /// The action cannot be done from here (no path, out of reach, wrong form).
    NotPossible,
    /// Nothing was sent: the target is in reach but hidden behind blocks, so moving and retrying is safe.
    NotVisible,
    /// `cancel` ended the call, or removed it before it ran.
    Cancelled,
    /// The bot's call queue is full.
    Busy,
    UnknownBot,
    UnknownMethod,
    /// The command or its params do not fit the schema.
    BadParams,
    /// The server address did not resolve.
    Resolve,
    /// A socket error while connecting.
    Io,
    /// NetherNet or its signaling service failed while connecting.
    NetherNet,
    /// The host is shutting down.
    Closed,
    /// A socket client's `hello` named a protocol version the server does not speak.
    VersionMismatch,
    /// A socket client sent a wrong token, or a call before `hello`.
    Unauthorized,
    /// A bug: a bot task panicked, or a value did not serialize.
    Internal,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}
