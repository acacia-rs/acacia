//! Test servers and capture tooling: [`FakeServer`] replays a recorded BDS session to a real client over
//! loopback RakNet, records what the client sends and lets the test send, receive and kick;
//! [`capture`] reads acacia-mitm captures.
//!
//! ```no_run
//! # async fn t() -> std::io::Result<()> {
//! let server = acacia_testserver::FakeServer::start(acacia_testserver::Script::bds_spawn()).await?;
//! // connect a client to server.addr(), then inspect server.received()
//! # Ok(()) }
//! ```

pub mod capture;
mod peer;
mod script;
mod server;

pub use peer::Received;
pub use script::{Script, Step};
pub use server::{FakeServer, WAIT_TIMEOUT};
