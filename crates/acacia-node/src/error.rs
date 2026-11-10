use acacia_api::{ApiError, ErrorCode};
use napi::Status;

/// A JS error's `code`: an acacia-api [`ErrorCode`] name, or a napi status.
pub struct Code(String);

impl AsRef<str> for Code {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Status> for Code {
    fn from(status: Status) -> Self {
        Self(status.as_ref().to_owned())
    }
}

impl From<ErrorCode> for Code {
    fn from(code: ErrorCode) -> Self {
        let name = serde_json::to_value(code).ok().and_then(|v| v.as_str().map(str::to_owned));
        Self(name.unwrap_or_else(|| "INTERNAL".into()))
    }
}

pub fn refusal(e: ApiError) -> napi::Error<Code> {
    napi::Error::new(e.code.into(), e.message)
}
