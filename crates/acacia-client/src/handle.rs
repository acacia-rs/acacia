use std::sync::Arc;

use acacia_session::blob_store::BlobStore;
use acacia_session::proto::{encode_packet, Packet};
use bytes::{Bytes, BytesMut};
use tokio::sync::mpsc;

use crate::client::ClientBuilder;
use crate::driver::Command;
use crate::login::Identity;
use crate::Event;

/// A spawned connection. Dropping it disconnects.
pub struct Client {
    pub(crate) commands: mpsc::UnboundedSender<Command>,
    pub(crate) events: mpsc::Receiver<Event>,
    pub(crate) runtime_entity_id: u64,
    pub(crate) identity: Identity,
    pub(crate) blob_store: Option<Arc<dyn BlobStore>>,
}

impl Client {
    /// The blob cache this connection reports from; terrain readers fetch blob payloads here.
    pub fn blob_store(&self) -> Option<&Arc<dyn BlobStore>> {
        self.blob_store.as_ref()
    }

    pub fn builder(server: impl Into<String>) -> ClientBuilder {
        ClientBuilder::new(server)
    }

    pub fn runtime_entity_id(&self) -> u64 {
        self.runtime_entity_id
    }

    pub fn display_name(&self) -> &str {
        &self.identity.display_name
    }

    /// Empty for offline logins.
    pub fn xuid(&self) -> &str {
        &self.identity.xuid
    }

    /// Next packet or the final disconnect; `None` once the connection is gone.
    pub async fn recv(&mut self) -> Option<Event> {
        self.events.recv().await
    }

    /// Queues a packet; returns false if the connection has closed.
    pub fn send<T: Packet>(&self, packet: &T) -> bool {
        let mut buf = BytesMut::new();
        encode_packet(packet, &mut buf);
        self.send_raw(buf.freeze())
    }

    /// Queues an already-encoded packet (header + body).
    pub fn send_raw(&self, packet: Bytes) -> bool {
        self.command_raw(Command::Send(packet))
    }

    pub fn close(&self) {
        self.command_raw(Command::Close);
    }

    pub(crate) fn command_raw(&self, command: Command) -> bool {
        self.commands.send(command).is_ok()
    }
}
