use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use acacia_session::{DisconnectReason, Event as SessionEvent, PackFetch, Session};
use acacia_nethernet::{Connection, Event as NetEvent};

use crate::net_wire::NetherNetWire;
use crate::signaling::Keepalive;
use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};

use crate::filter::PacketFilter;
use crate::pack_fetch;
use crate::socks5::Socks5Proxy;
use crate::transport::Transport;
use crate::Event;

/// Largest datagram we accept; above the biggest RakNet MTU and the WebRTC (1200-byte) packets.
const RECV_BUFFER: usize = 2048;

pub(crate) enum Command {
    Send(Bytes),
    Respawn,
    RespawnDone,
    Close,
}

pub(crate) type SpawnResult = Result<u64, DisconnectReason>;

/// What sits between the session and the socket.
pub(crate) enum Wire {
    /// The session speaks RakNet datagrams directly.
    RakNet,
    /// The session's game messages travel over a WebRTC data channel.
    NetherNet(Box<NetherNetWire>),
}

pub(crate) struct Driver {
    pub session: Session,
    pub wire: Wire,
    pub transport: Transport,
    pub commands: mpsc::UnboundedReceiver<Command>,
    pub events: mpsc::Sender<Event>,
    pub spawned: Option<oneshot::Sender<SpawnResult>>,
    pub filter: PacketFilter,
    pub capacity: usize,
    /// The connection's proxy, for pack downloads too.
    pub proxy: Option<Socks5Proxy>,
    /// Held for the connection's life; dropping it closes the signaling socket.
    pub _signaling: Option<Keepalive>,
    /// Held for the connection's life; dropping it leaves a friend's Xbox session.
    pub _friend_session: Option<oneshot::Sender<()>>,
}

impl Driver {
    /// Runs the connection until it closes; one task per connection.
    ///
    /// Never awaits the event channel directly: nobody reads it until login completes, and the
    /// login burst can exceed its capacity. Events queue locally instead; once spawned, socket
    /// reads pause while that queue is over capacity (backpressure without deadlock).
    pub async fn run(mut self) {
        let mut buf = vec![0u8; RECV_BUFFER];
        let mut pending: VecDeque<Event> = VecDeque::new();
        let (fetched_tx, mut fetched) = mpsc::unbounded_channel();
        loop {
            if let Err(e) = self.flush().await {
                return self.finish(pending, DisconnectReason::Io(e.to_string())).await;
            }
            if let Some(reason) = self.wire_closed() {
                return self.finish(pending, reason).await;
            }
            while let Some(event) = self.session.poll_event() {
                match event {
                    SessionEvent::Spawned { runtime_entity_id } => {
                        if let Some(tx) = self.spawned.take() {
                            let _ = tx.send(Ok(runtime_entity_id));
                        }
                    }
                    SessionEvent::Packet(p) if self.filter.allows(p.id) => pending.push_back(Event::Packet(p)),
                    SessionEvent::Packet(_) => {}
                    SessionEvent::Violation(v) => pending.push_back(Event::Violation(v)),
                    SessionEvent::Disconnected(reason) => {
                        let _ = self.flush().await;
                        self.close_wire();
                        let _ = self.flush().await;
                        return self.finish(pending, reason).await;
                    }
                }
            }
            while let Some(pack) = self.session.poll_pack_fetch() {
                tokio::spawn(fetch_pack(pack, self.proxy.clone(), fetched_tx.clone()));
            }
            let can_read = self.spawned.is_some() || pending.len() < self.capacity;
            let deadline = self.poll_timeout().unwrap_or_else(|| Instant::now() + Duration::from_secs(60));
            let failed = tokio::select! {
                permit = self.events.reserve(), if !pending.is_empty() => {
                    match permit {
                        Ok(permit) => permit.send(pending.pop_front().expect("non-empty")),
                        Err(_) => self.session.close(Instant::now()),
                    }
                    None
                }
                r = self.transport.recv_from(&mut buf), if can_read => match r {
                    Ok((range, source)) => {
                        tracing::trace!(len = range.len(), flags = buf[range.start], "datagram");
                        handle_datagram(&mut self.session, &mut self.wire, source, &buf[range]);
                        None
                    }
                    Err(e) => Some(DisconnectReason::Io(e.to_string())),
                },
                cmd = self.commands.recv() => {
                    match cmd {
                        Some(Command::Send(packet)) => self.session.send_raw(packet),
                        Some(Command::Respawn) => self.session.respawn(),
                        Some(Command::RespawnDone) => self.session.send_respawn_done(),
                        Some(Command::Close) | None => self.session.close(Instant::now()),
                    }
                    None
                }
                Some((id, ok)) = fetched.recv() => {
                    self.session.pack_fetched(Instant::now(), &id, ok);
                    None
                }
                _ = tokio::time::sleep_until(deadline.into()) => {
                    handle_timeout(&mut self.session, &mut self.wire, Instant::now());
                    None
                }
            };
            if let Some(reason) = failed {
                return self.finish(pending, reason).await;
            }
        }
    }

    fn poll_timeout(&self) -> Option<Instant> {
        let session = self.session.poll_timeout();
        match &self.wire {
            Wire::RakNet => session,
            Wire::NetherNet(conn) => session.into_iter().chain(conn.poll_timeout()).min(),
        }
    }

    /// The disconnect reason once the NetherNet connection has failed or closed.
    fn wire_closed(&mut self) -> Option<DisconnectReason> {
        let Wire::NetherNet(conn) = &mut self.wire else { return None };
        while let Some(event) = conn.conn.poll_event() {
            if let NetEvent::Closed(reason) = event {
                return Some(DisconnectReason::NetherNet(reason));
            }
        }
        None
    }

    fn close_wire(&mut self) {
        if let Wire::NetherNet(conn) = &mut self.wire {
            conn.close(Instant::now());
        }
    }

    async fn flush(&mut self) -> std::io::Result<()> {
        let now = Instant::now();
        match &mut self.wire {
            Wire::RakNet => {
                while let Some(datagram) = self.session.poll_transmit(now) {
                    self.transport.send(&datagram).await?;
                }
            }
            Wire::NetherNet(conn) => {
                while let Some(msg) = self.session.poll_transmit(now) {
                    tracing::trace!(len = msg.len(), open = conn.conn.is_open(), "game message out");
                    conn.conn.send(msg, now);
                }
                while let Some((target, datagram)) = conn.poll_datagram(now) {
                    // A server lists candidates we may not be able to reach (other families, LANs).
                    if let Err(e) = self.transport.send_to(&datagram, target).await {
                        tracing::trace!(%target, "send failed: {e}");
                    }
                }
            }
        }
        Ok(())
    }

    /// Delivers queued packets, then the disconnect (or fails the pending login).
    async fn finish(mut self, pending: VecDeque<Event>, reason: DisconnectReason) {
        if let Some(tx) = self.spawned.take() {
            let _ = tx.send(Err(reason));
            return;
        }
        for event in pending {
            if self.events.send(event).await.is_err() {
                return;
            }
        }
        let _ = self.events.send(Event::Disconnected(reason)).await;
    }
}

async fn fetch_pack(pack: PackFetch, proxy: Option<Socks5Proxy>, done: mpsc::UnboundedSender<(String, bool)>) {
    let result = tokio::time::timeout(pack_fetch::FETCH_TIMEOUT, pack_fetch::fetch(&pack.url, proxy.as_ref())).await;
    let ok = match result {
        Ok(Ok(size)) => {
            tracing::debug!(pack = pack.id, size, "resource pack downloaded");
            true
        }
        Ok(Err(e)) => {
            tracing::warn!(pack = pack.id, url = pack.url, "resource pack download: {e}");
            false
        }
        Err(_) => {
            tracing::warn!(pack = pack.id, url = pack.url, "resource pack download timed out");
            false
        }
    };
    let _ = done.send((pack.id, ok));
}

// Free functions: `select!` holds `transport` borrowed while these run.
fn handle_datagram(session: &mut Session, wire: &mut Wire, source: SocketAddr, data: &[u8]) {
    let now = Instant::now();
    match wire {
        Wire::RakNet => session.handle_datagram(now, Bytes::copy_from_slice(data)),
        Wire::NetherNet(conn) => {
            conn.handle_datagram(now, source, data);
            deliver_messages(session, &mut conn.conn, now);
        }
    }
}

fn handle_timeout(session: &mut Session, wire: &mut Wire, now: Instant) {
    if let Wire::NetherNet(conn) = wire {
        conn.handle_timeout(now);
        deliver_messages(session, &mut conn.conn, now);
    }
    session.handle_timeout(now);
}

fn deliver_messages(session: &mut Session, conn: &mut Connection, now: Instant) {
    while let Some(msg) = conn.poll_message() {
        tracing::trace!(len = msg.len(), "game message in");
        session.handle_message(now, msg);
    }
}
