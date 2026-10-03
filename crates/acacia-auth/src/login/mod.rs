//! Network-free Login packet payload: the client's builder and the server's verifier ([`verify`]).

mod client_data;
mod device;
mod handshake;
mod request;
mod skin;
#[cfg(test)]
mod tests;
pub mod verify;

pub use client_data::{ClientData, DEVICE_OS_ANDROID, INPUT_MODE_TOUCH, MAX_VIEW_DISTANCE};
pub use skin::Skin;
pub use handshake::{build_server_handshake, client_public_key, parse_server_handshake};
pub use request::{
    MULTIPLAYER_AUDIENCE, OfflineIdentity, build_connection_request, build_offline_connection_request,
    build_offline_connection_request_for, offline_identity, split_connection_request, xuid_identity,
};
pub use verify::{Identity, SigningKeys, VerifiedLogin, Verifier, VerifyError};
