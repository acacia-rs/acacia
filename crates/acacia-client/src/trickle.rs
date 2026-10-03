//! A trickle-ICE NetherNet connection over any signal path (the signaling service, LAN discovery):
//! the offer and host candidate go out at once, TURN only as a fallback, until the channels open.

use std::time::{Duration, Instant};

use acacia_nethernet::turn::{IceServers, Scheme, Transport as IceTransport, TurnClient, TurnEvent};
use acacia_nethernet::{Connection, Event as NetEvent, Identity, LocalCandidate, Signal, SignalKind};
use rand_core::{OsRng, RngCore};

use crate::net_wire::NetherNetWire;
use crate::transport::{resolve, Transport};
use crate::ConnectError;

/// Vanilla joined a realm with host candidates only (ICE up 1.4 s after the join); relay candidates
/// go out only if ICE is still down this long after the offer.
const TURN_FALLBACK: Duration = Duration::from_secs(5);
const RECV_BUFFER: usize = 2048;

/// How signals reach the host.
pub(crate) trait SignalPath {
    /// Queues a signal for the host.
    fn send(&mut self, signal: Signal);
    /// Writes queued output.
    async fn flush(&mut self) -> Result<(), ConnectError>;
    /// Waits for input; returns the signals it carried.
    async fn recv(&mut self) -> Result<Vec<Signal>, ConnectError>;
    fn poll_timeout(&self) -> Option<Instant>;
    fn handle_timeout(&mut self, now: Instant);
    /// TURN credentials, once the path has some.
    fn take_ice_servers(&mut self) -> Option<IceServers>;
    /// Whether the host may answer without `a=identity` (game-hosted worlds do).
    fn identityless_host_ok(&self) -> bool {
        false
    }
}

/// Offers over `path` and runs ICE/DTLS on `transport` until both data channels open.
pub(crate) async fn connect<P: SignalPath>(
    path: &mut P,
    transport: &Transport,
    identity: &Identity,
    deadline: Instant,
) -> Result<NetherNetWire, ConnectError> {
    let host = transport.local_candidate()?;
    let now = Instant::now();
    let (mut conn, offer) = Connection::trickle_offer(identity, now)?;
    if path.identityless_host_ok() {
        conn.allow_identityless_host();
    }
    let mut wire = NetherNetWire::new(conn, None);
    let connection_id = OsRng.next_u64();
    path.send(Signal::new(SignalKind::ConnectRequest, connection_id, offer));
    let line = wire.conn.add_local_candidate(LocalCandidate::Host(host), now)?;
    path.send(Signal::new(SignalKind::CandidateAdd, connection_id, line));
    tracing::debug!(connection_id, "signaled NetherNet offer");

    let fallback_at = now + TURN_FALLBACK;
    let mut ice_servers = None;
    let mut buf = vec![0u8; RECV_BUFFER];
    loop {
        let now = Instant::now();
        ice_servers = ice_servers.or_else(|| path.take_ice_servers());
        if now >= fallback_at && wire.turn().is_none() && !wire.conn.is_open() {
            if let Some((addr, username, password)) = ice_servers.take().as_ref().and_then(turn_server) {
                tracing::debug!(%addr, "ICE still down; falling back to TURN");
                wire.set_turn(TurnClient::new(resolve(&addr).await?, username, password, now), now);
            }
        }
        while let Some(event) = wire.turn_mut().and_then(TurnClient::poll_event) {
            match event {
                TurnEvent::Allocated { relayed, mapped } => {
                    if let Some(addr) = mapped {
                        let line = wire.conn.add_local_candidate(LocalCandidate::ServerReflexive { addr, base: host }, now)?;
                        path.send(Signal::new(SignalKind::CandidateAdd, connection_id, line));
                    }
                    let relay = LocalCandidate::Relayed { addr: relayed, mapped: mapped.unwrap_or(host) };
                    let line = wire.conn.add_local_candidate(relay, now)?;
                    path.send(Signal::new(SignalKind::CandidateAdd, connection_id, line));
                }
                TurnEvent::Failed(e) => tracing::debug!("TURN allocation failed: {e}"),
            }
        }
        while let Some(event) = wire.conn.poll_event() {
            match event {
                NetEvent::Open => {
                    path.flush().await?;
                    return Ok(wire);
                }
                NetEvent::Closed(reason) => return Err(ConnectError::Signaling(format!("connection closed: {reason}"))),
            }
        }
        while let Some((dest, datagram)) = wire.poll_datagram(now) {
            if let Err(e) = transport.send_to(&datagram, dest).await {
                tracing::trace!(%dest, "send failed: {e}");
            }
        }
        path.flush().await?;

        let fallback = wire.turn().is_none().then_some(fallback_at);
        let next = [wire.poll_timeout(), fallback, path.poll_timeout(), Some(deadline)].into_iter().flatten().min().expect("deadline");
        tokio::select! {
            signals = path.recv() => {
                for s in signals? {
                    if s.connection_id == connection_id {
                        on_signal(&mut wire, &s, Instant::now())?;
                    }
                }
            }
            r = transport.recv_from(&mut buf) => {
                let (range, source) = r?;
                wire.handle_datagram(Instant::now(), source, &buf[range]);
            }
            _ = tokio::time::sleep_until(next.into()) => {
                let now = Instant::now();
                if now >= deadline {
                    return Err(ConnectError::Timeout);
                }
                path.handle_timeout(now);
                wire.handle_timeout(now);
            }
        }
    }
}

fn on_signal(wire: &mut NetherNetWire, signal: &Signal, now: Instant) -> Result<(), ConnectError> {
    match signal.kind {
        SignalKind::ConnectResponse => {
            wire.conn.accept_answer(&signal.data, now)?;
            wire.permit_answer(&signal.data, now);
        }
        SignalKind::CandidateAdd => {
            if let Err(e) = wire.add_remote_candidate(&signal.data, now) {
                tracing::debug!("ignoring remote candidate: {e}");
            }
        }
        SignalKind::ConnectError => {
            let code = signal.error_code().unwrap_or(u32::MAX);
            return Err(acacia_nethernet::Error::Signaling { code }.into());
        }
        SignalKind::ConnectRequest => tracing::debug!("ignoring CONNECTREQUEST from the host"),
    }
    Ok(())
}

/// The first UDP TURN server as `host:port` with its credentials.
fn turn_server(servers: &IceServers) -> Option<(String, String, String)> {
    let found = servers.urls().find(|(u, _)| u.scheme == Scheme::Turn && u.transport == IceTransport::Udp);
    if found.is_none() {
        tracing::debug!("no UDP TURN server in the credentials");
    }
    found.map(|(url, s)| (format!("{}:{}", url.host, url.port), s.username.clone(), s.password.clone()))
}
