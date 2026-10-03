//! Minecraft Bedrock authentication.
//!
//! - Always available (network-free): the Login packet payload builder ([`login`]), ES384 JWT
//!   helpers ([`jwt`]) and the server handshake parser.
//! - Feature `online` (default): [`AuthClient`] (device code → Xbox → XSTS → Mojang chain +
//!   PlayFab → session/start → multiplayer token), [`TokenCache`] and [`Account`].
//!
//! Protocol details and verified endpoints: `docs/auth.md`.

mod credentials;
mod error;
pub mod jwt;
pub mod login;

#[cfg(feature = "online")]
mod online;

pub use credentials::LoginCredentials;
pub use error::{Error, Result, XboxError};
pub use jwt::public_key_der_b64;
pub use login::{
    ClientData, MAX_VIEW_DISTANCE, build_connection_request, build_offline_connection_request,
    parse_server_handshake,
};

#[cfg(feature = "online")]
pub use online::*;
