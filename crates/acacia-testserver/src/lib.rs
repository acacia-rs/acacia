//! Test servers and capture tooling: [`FakeServer`] replays a recorded BDS session to a real client over
//! loopback RakNet and records what the client sends; [`capture`] reads tools/mitm captures.
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
pub use server::FakeServer;
