use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytes::{Bytes, BytesMut};

use crate::conn::{Conn, ConnConfig};
use crate::reliability::RecvLimits;
use crate::types::{Config, DisconnectReason, Event};
use crate::wire::datagram::{Reliability, FLAG_VALID};
use crate::wire::{connected as c, offline as o, WireError};

const OFFLINE_RETRY: Duration = Duration::from_millis(500);
const ATTEMPTS_PER_MTU: u8 = 4;

enum State {
    Open1 { mtu_idx: usize, attempts: u8, next: Instant },
    Open2 { cookie: Option<u32>, attempts: u8, next: Instant },
    Handshaking(Conn),
    Connected(Conn),
    Closed,
}

/// A network-free RakNet client connection: feed it datagrams and timeouts, drain datagrams and events.
pub struct Client {
    cfg: Config,
    server: SocketAddr,
    epoch: Instant,
    started: Instant,
    state: State,
    mtu: u16,
    outbox: VecDeque<Bytes>,
    events: VecDeque<Event>,
}

impl Client {
    pub fn new(cfg: Config, server: SocketAddr, now: Instant) -> Self {
        let mtu = cfg.mtu_sizes.first().copied().unwrap_or(576);
        let mut client = Self {
            cfg,
            server,
            epoch: now,
            started: now,
            state: State::Open1 { mtu_idx: 0, attempts: 0, next: now },
            mtu,
            outbox: VecDeque::new(),
            events: VecDeque::new(),
        };
        client.handle_timeout(now);
        client
    }

    pub fn server(&self) -> SocketAddr {
        self.server
    }

    pub fn mtu(&self) -> u16 {
        self.mtu
    }

    pub fn rtt(&self) -> Option<Duration> {
        self.conn().and_then(Conn::rtt)
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.state, State::Connected(_))
    }

    pub fn is_closed(&self) -> bool {
        matches!(self.state, State::Closed)
    }

    fn conn(&self) -> Option<&Conn> {
        match &self.state {
            State::Handshaking(conn) | State::Connected(conn) => Some(conn),
            _ => None,
        }
    }

    fn conn_mut(&mut self) -> Option<&mut Conn> {
        match &mut self.state {
            State::Handshaking(conn) | State::Connected(conn) => Some(conn),
            _ => None,
        }
    }

    /// Queues a message on ordering channel 0. Returns false if the connection is not open.
    pub fn send(&mut self, data: Bytes, reliability: Reliability) -> bool {
        match &mut self.state {
            State::Connected(conn) => {
                conn.queue(data, reliability);
                true
            }
            _ => false,
        }
    }

    /// Sends a disconnection notification (if connected) and closes; drain `poll_transmit` once more to send it.
    pub fn close(&mut self, now: Instant) {
        if let State::Connected(conn) = &mut self.state {
            self.outbox.extend(conn.close(now));
        }
        self.fail(DisconnectReason::LocalClose);
    }

    fn fail(&mut self, reason: DisconnectReason) {
        if !self.is_closed() {
            self.state = State::Closed;
            self.events.push_back(Event::Disconnected(reason));
        }
    }

    pub fn poll_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    pub fn poll_transmit(&mut self, now: Instant) -> Option<Bytes> {
        if let Some(d) = self.outbox.pop_front() {
            return Some(d);
        }
        self.conn_mut()?.poll_transmit(now)
    }

    pub fn poll_timeout(&self) -> Option<Instant> {
        match &self.state {
            State::Open1 { next, .. } | State::Open2 { next, .. } => Some(*next),
            State::Handshaking(conn) | State::Connected(conn) => Some(conn.poll_timeout()),
            State::Closed => None,
        }
    }

    pub fn handle_timeout(&mut self, now: Instant) {
        if !self.is_closed() && now >= self.started + self.cfg.connect_timeout && !self.is_connected() {
            return self.fail(DisconnectReason::ConnectTimeout);
        }
        match &mut self.state {
            State::Open1 { mtu_idx, attempts, next } if now >= *next => {
                if *attempts >= ATTEMPTS_PER_MTU && *mtu_idx + 1 < self.cfg.mtu_sizes.len() {
                    *mtu_idx += 1;
                    *attempts = 0;
                }
                *attempts += 1;
                *next = now + OFFLINE_RETRY;
                self.mtu = self.cfg.mtu_sizes[*mtu_idx];
                let mut buf = BytesMut::with_capacity(usize::from(self.mtu));
                o::open_connection_request_1(&mut buf, self.cfg.protocol_version, self.mtu);
                self.outbox.push_back(buf.freeze());
            }
            State::Open2 { cookie, attempts, next } if now >= *next => {
                *attempts += 1;
                *next = now + OFFLINE_RETRY;
                let mut buf = BytesMut::with_capacity(64);
                o::open_connection_request_2(&mut buf, *cookie, self.server, self.mtu, self.cfg.guid);
                self.outbox.push_back(buf.freeze());
            }
            State::Handshaking(conn) | State::Connected(conn) => {
                if let Err(reason) = conn.handle_timeout(now) {
                    self.fail(reason);
                }
            }
            _ => {}
        }
    }

    pub fn handle_datagram(&mut self, now: Instant, data: Bytes) {
        let Some(&id) = data.first() else { return };
        if self.is_closed() {
            return;
        }
        let result = match self.conn_mut() {
            Some(conn) if id & FLAG_VALID != 0 => conn.handle_datagram(now, &data).and_then(|()| self.handle_messages(now)),
            Some(_) => Ok(()),
            None => self.handle_offline(now, id, &data).map_err(|e| DisconnectReason::Protocol(e.to_string())),
        };
        if let Err(reason) = result {
            self.fail(reason);
        }
    }

    fn handle_offline(&mut self, now: Instant, id: u8, data: &[u8]) -> Result<(), WireError> {
        let reason = match id {
            o::ID_INCOMPATIBLE_PROTOCOL_VERSION => DisconnectReason::IncompatibleProtocol,
            o::ID_ALREADY_CONNECTED => DisconnectReason::AlreadyConnected,
            o::ID_NO_FREE_INCOMING_CONNECTIONS => DisconnectReason::ServerFull,
            o::ID_CONNECTION_BANNED => DisconnectReason::Banned,
            o::ID_IP_RECENTLY_CONNECTED => DisconnectReason::RecentlyConnected,
            o::ID_OPEN_CONNECTION_REPLY_1 if matches!(self.state, State::Open1 { .. }) => {
                let reply = o::parse_reply_1(data)?;
                self.mtu = self.mtu.min(reply.mtu);
                self.state = State::Open2 { cookie: reply.cookie, attempts: 0, next: now };
                self.handle_timeout(now);
                return Ok(());
            }
            o::ID_OPEN_CONNECTION_REPLY_2 if matches!(self.state, State::Open2 { .. }) => {
                let reply = o::parse_reply_2(data)?;
                self.mtu = self.mtu.min(reply.mtu);
                let cfg = ConnConfig {
                    mtu: self.mtu,
                    idle_timeout: self.cfg.idle_timeout,
                    ping_interval: self.cfg.ping_interval,
                    recv_limits: RecvLimits::CLIENT,
                };
                let mut conn = Conn::new(self.epoch, cfg, now);
                conn.queue(c::connection_request(self.cfg.guid, conn.time(now)), Reliability::Reliable);
                self.state = State::Handshaking(conn);
                return Ok(());
            }
            _ => return Ok(()),
        };
        self.fail(reason);
        Ok(())
    }

    fn handle_messages(&mut self, now: Instant) -> Result<(), DisconnectReason> {
        while let Some(msg) = self.conn_mut().and_then(|conn| conn.pop_message(now)) {
            match msg[0] {
                c::ID_CONNECTION_REQUEST_ACCEPTED => {
                    let State::Handshaking(conn) = &mut self.state else { continue };
                    let server_time = c::parse_request_accepted(&msg).map_err(|e| DisconnectReason::Protocol(e.to_string()))?;
                    let nic = c::new_incoming_connection(self.server, server_time, conn.time(now));
                    conn.queue(nic, Reliability::ReliableOrdered);
                    let State::Handshaking(conn) = std::mem::replace(&mut self.state, State::Closed) else { unreachable!() };
                    self.state = State::Connected(conn);
                    self.events.push_back(Event::Connected { mtu: self.mtu });
                }
                c::ID_DISCONNECTION_NOTIFICATION => {
                    self.fail(DisconnectReason::ServerClosed);
                    return Ok(());
                }
                _ if self.is_connected() => self.events.push_back(Event::Message(msg)),
                _ => {}
            }
        }
        Ok(())
    }
}
