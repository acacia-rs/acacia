use std::fmt;

/// Which JWT of a Login request a check was made on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// The multiplayer token (envelope `Token`).
    Token,
    /// The legacy certificate chain's JWT at this index.
    Chain(usize),
    /// The client-data JWT.
    ClientData,
}

impl fmt::Display for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Part::Token => f.write_str("multiplayer token"),
            Part::Chain(i) => write!(f, "chain[{i}]"),
            Part::ClientData => f.write_str("client data"),
        }
    }
}

/// Why a Login request was refused. Every variant is a rejection: an unauthenticated but valid
/// login is `Ok` with `authenticated: false`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    #[error("malformed connection request: {0}")]
    Framing(&'static str),
    #[error("malformed login envelope: {0}")]
    Envelope(String),
    #[error("guest (split-screen) logins are not supported")]
    GuestLogin,
    #[error("login carries neither a multiplayer token nor a certificate chain")]
    NoCredentials,
    #[error("{part} is not a valid JWT: {reason}")]
    Malformed { part: Part, reason: String },
    #[error("{part} uses algorithm {alg:?}")]
    Algorithm { part: Part, alg: String },
    #[error("multiplayer token is signed by an unknown key (kid {kid:?})")]
    UnknownSigningKey { kid: Option<String> },
    #[error("{part} signature does not verify")]
    Signature { part: Part },
    #[error("{part} has issuer {found:?}")]
    Issuer { part: Part, found: Option<String> },
    #[error("multiplayer token is for another audience")]
    Audience,
    #[error("{part} has expired")]
    Expired { part: Part },
    #[error("{part} is not valid yet")]
    NotYetValid { part: Part },
    #[error("{part} claim {claim} is missing or invalid")]
    Claim { part: Part, claim: &'static str },
    #[error("certificate chain has {0} JWTs, expected 1 or 3")]
    ChainLength(usize),
    /// A chain that names an XUID without being rooted at Mojang's key.
    #[error("certificate chain claims an Xbox identity but is not signed by Mojang")]
    UntrustedChain,
    #[error("invalid JWKS: {0}")]
    Jwks(String),
}
