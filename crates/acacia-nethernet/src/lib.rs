//! Network-free NetherNet client: direct-connect HTTP signaling, signal text for LAN and the
//! signaling service, the SDP identity assertion, fragment framing and a str0m WebRTC connection.
//! Wire specs: docs/research/nethernet-wire.md, docs/research/nethernet-signaling.md.

mod cert;
mod conn;
mod error;
mod frame;
pub mod http;
mod identity;
pub mod lan;
#[cfg(test)]
mod loopback_tests;
mod sdp;
mod server_identity;
mod signal;
pub mod signaling;
pub mod turn;

pub use conn::{Connection, Event, LocalCandidate, Transmit};
pub use signal::{Signal, SignalKind};
pub use error::{signaling_error_name, Error};
pub use identity::{Identity, AUTH_DOMAIN};
/// For fake hosts in other crates' tests.
#[cfg(feature = "test-support")]
pub use server_identity::sign_as_server;

/// `GET` path whose 2xx answer means the server speaks NetherNet.
pub const PROBE_PATH: &str = "/v1/join";

/// `POST` path carrying the SDP offer; `network_id` is the client's random u64.
pub fn join_path(network_id: u64) -> String {
    format!("/v1/join/{network_id}")
}
