use std::sync::Arc;

use acacia_session::blob_store::BlobStore;
use acacia_session::proto::Packet;
use bytes::Bytes;

use crate::client::ClientBuilder;
use crate::driver::Command;
use crate::event_queue::EventReceiver;
use crate::filter::PacketFilter;
use crate::packet_trace::PacketTrace;
use crate::sender::ClientSender;
use crate::Event;

/// A spawned connection. Dropping it disconnects.
pub struct Client {
    pub(crate) sender: ClientSender,
    pub(crate) events: EventReceiver,
    pub(crate) runtime_entity_id: u64,
    pub(crate) blob_store: Option<Arc<dyn BlobStore>>,
    pub(crate) trace: PacketTrace,
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

    /// A handle that sends on this connection from other threads (chat while the owner is busy).
    pub fn sender(&self) -> ClientSender {
        self.sender.clone()
    }

    pub fn display_name(&self) -> &str {
        self.sender.display_name()
    }

    /// Empty for offline logins.
    pub fn xuid(&self) -> &str {
        self.sender.xuid()
    }

    /// Next packet or the final disconnect; `None` once the connection is gone.
    pub async fn recv(&mut self) -> Option<Event> {
        self.events.recv().await
    }

    /// The next event if one is already queued.
    pub fn try_recv(&mut self) -> Option<Event> {
        self.events.try_recv()
    }

    /// Queues a packet; returns false if the connection has closed.
    pub fn send<T: Packet + std::fmt::Debug>(&self, packet: &T) -> bool {
        self.sender.send(packet)
    }

    /// Queues an already-encoded packet (header + body).
    pub fn send_raw(&self, packet: Bytes) -> bool {
        self.sender.send_raw(packet)
    }

    /// Replaces the filter set at connect ([`ClientBuilder::filter`]); packets already queued passed the old one.
    pub fn set_filter(&self, filter: PacketFilter) -> bool {
        self.command_raw(Command::Filter(filter))
    }

    pub fn close(&self) {
        self.command_raw(Command::Close);
    }

    pub(crate) fn command_raw(&self, command: Command) -> bool {
        self.sender.command_raw(command)
    }
}

impl Drop for Client {
    // Senders may outlive the client, so the command channel alone would not close.
    fn drop(&mut self) {
        self.close();
    }
}
