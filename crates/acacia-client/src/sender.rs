use std::sync::Arc;

use acacia_session::proto::{encode_packet, Packet};
use bytes::{Bytes, BytesMut};
use tokio::sync::mpsc;

use crate::driver::Command;
use crate::login::Identity;

/// Sends on a connection from any thread or task, without the [`crate::Client`] (which one task owns). Cheap to
/// clone; every method returns false once the connection has closed. It does not keep the connection open.
#[derive(Clone)]
pub struct ClientSender {
    pub(crate) commands: mpsc::UnboundedSender<Command>,
    pub(crate) identity: Arc<Identity>,
}

impl ClientSender {
    pub fn display_name(&self) -> &str {
        &self.identity.display_name
    }

    /// Empty for offline logins.
    pub fn xuid(&self) -> &str {
        &self.identity.xuid
    }

    /// Queues a packet.
    pub fn send<T: Packet + std::fmt::Debug>(&self, packet: &T) -> bool {
        tracing::trace!(target: "acacia_client::send", ?packet);
        // Fits a PlayerAuthInput, the packet sent every tick.
        let mut buf = BytesMut::with_capacity(128);
        encode_packet(packet, &mut buf);
        self.send_raw(buf.freeze())
    }

    /// Queues an already-encoded packet (header + body).
    pub fn send_raw(&self, packet: Bytes) -> bool {
        self.command_raw(Command::Send(packet))
    }

    pub(crate) fn command_raw(&self, command: Command) -> bool {
        self.commands.send(command).is_ok()
    }
}
