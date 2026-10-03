//! Network-free LAN discovery (UDP 7551): the packet codec and advertised world data. Signals ride in
//! [`LanPacket::Message`]; the client's I/O lives in acacia-client. Spec: docs/research/nethernet-signaling.md §1.

mod packet;
mod server_data;

pub use packet::LanPacket;
pub use server_data::ServerData;

pub const DISCOVERY_PORT: u16 = 7551;
