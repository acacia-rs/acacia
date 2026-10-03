use std::fmt;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid JWT: {0}")]
    Jwt(String),
    #[error("invalid key: {0}")]
    Key(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "online")]
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("{endpoint} returned HTTP {status}: {body}")]
    Status { endpoint: String, status: u16, body: String },
    /// Xbox Live refused the account (`X-Err` header). See [`XboxError`].
    #[error("xbox live: {0}")]
    Xbox(XboxError),
    /// Xbox withheld the user token and points at a page the account holder must complete in a
    /// browser, e.g. Yoti age verification (`.../view/yotiagecheck.html`) or profile creation.
    #[error("{endpoint}: account action required at {url}")]
    ActionRequired { endpoint: String, url: String },
    #[error("oauth error {code}: {description}")]
    OAuth { code: String, description: String },
    #[error("device code expired before the user signed in")]
    DeviceCodeExpired,
    #[error("user declined the device code login")]
    DeviceCodeDeclined,
    #[error("unexpected response from {endpoint}: {reason}")]
    Protocol { endpoint: String, reason: String },
}

impl Error {
    /// True when the account cannot play online until the user fixes it on Microsoft's side
    /// (no Xbox profile, child account, ban, ToS, ...). Retrying will not help.
    pub fn requires_user_action(&self) -> bool {
        match self {
            Error::Xbox(e) => !matches!(e, XboxError::Other(_)),
            Error::ActionRequired { .. } => true,
            _ => false,
        }
    }
}

/// Xbox Live `X-Err` codes (SISU/XSTS). Values from go-xsapi `sisu.ErrorCode` and prismarine-auth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XboxError {
    /// 0x8015DC03 (2148916227)
    Banned,
    /// 0x8015DC05 (2148916229)
    ParentallyRestricted,
    /// 0x8015DC09 (2148916233): the Microsoft account has no Xbox profile.
    NoXboxProfile,
    /// 0x8015DC0A (2148916234)
    TermsNotAccepted,
    /// 0x8015DC0B (2148916235)
    CountryNotAuthorized,
    /// 0x8015DC0C (2148916236)
    AgeVerificationRequired,
    /// 0x8015DC0D (2148916237)
    ScreenTimeExceeded,
    /// 0x8015DC0E (2148916238): under-18 account not in a family group.
    ChildAccount,
    /// 0x8015DC13
    GamertagChangeRequired,
    /// 0x8015DC16
    DeviceSignInLimit,
    /// 0x8015DC1E
    SignedInElsewhere,
    Other(u32),
}

impl XboxError {
    pub fn from_code(code: u32) -> Self {
        match code {
            0x8015_DC03 => Self::Banned,
            0x8015_DC05 => Self::ParentallyRestricted,
            0x8015_DC09 => Self::NoXboxProfile,
            0x8015_DC0A => Self::TermsNotAccepted,
            0x8015_DC0B => Self::CountryNotAuthorized,
            0x8015_DC0C => Self::AgeVerificationRequired,
            0x8015_DC0D => Self::ScreenTimeExceeded,
            0x8015_DC0E => Self::ChildAccount,
            0x8015_DC13 => Self::GamertagChangeRequired,
            0x8015_DC16 => Self::DeviceSignInLimit,
            0x8015_DC1E => Self::SignedInElsewhere,
            other => Self::Other(other),
        }
    }

    pub fn code(self) -> u32 {
        match self {
            Self::Banned => 0x8015_DC03,
            Self::ParentallyRestricted => 0x8015_DC05,
            Self::NoXboxProfile => 0x8015_DC09,
            Self::TermsNotAccepted => 0x8015_DC0A,
            Self::CountryNotAuthorized => 0x8015_DC0B,
            Self::AgeVerificationRequired => 0x8015_DC0C,
            Self::ScreenTimeExceeded => 0x8015_DC0D,
            Self::ChildAccount => 0x8015_DC0E,
            Self::GamertagChangeRequired => 0x8015_DC13,
            Self::DeviceSignInLimit => 0x8015_DC16,
            Self::SignedInElsewhere => 0x8015_DC1E,
            Self::Other(c) => c,
        }
    }
}

impl fmt::Display for XboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?} (X-Err {code} / {code:#X})", code = self.code())
    }
}
