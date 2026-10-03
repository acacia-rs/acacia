use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use acacia_raknet::{self as raknet, Reliability};
use bytes::Bytes;

pub enum LinkConfig {
    RakNet(raknet::Config),
    /// A message transport (NetherNet) that the driver keeps connected and reliable.
    Message,
}

/// What carries game messages: RakNet datagrams, or whole messages handed over by the driver.
pub(super) enum Link {
    RakNet(raknet::Client),
    Message { inbound: VecDeque<raknet::Event>, outbound: VecDeque<Bytes> },
}

impl Link {
    pub fn new(cfg: LinkConfig, server: SocketAddr, now: Instant) -> Self {
        match cfg {
            LinkConfig::RakNet(cfg) => Self::RakNet(raknet::Client::new(cfg, server, now)),
            LinkConfig::Message => Self::Message {
                inbound: VecDeque::from([raknet::Event::Connected { mtu: 0 }]),
                outbound: VecDeque::new(),
            },
        }
    }

    pub fn is_message(&self) -> bool {
        matches!(self, Self::Message { .. })
    }

    pub fn rtt(&self) -> Option<Duration> {
        match self {
            Self::RakNet(r) => r.rtt(),
            Self::Message { .. } => None,
        }
    }

    pub fn is_connected(&self) -> bool {
        match self {
            Self::RakNet(r) => r.is_connected(),
            Self::Message { .. } => true,
        }
    }

    pub fn send(&mut self, batch: Bytes) {
        match self {
            Self::RakNet(r) => {
                r.send(batch, Reliability::ReliableOrdered);
            }
            Self::Message { outbound, .. } => outbound.push_back(batch),
        }
    }

    pub fn handle_datagram(&mut self, now: Instant, data: Bytes) {
        if let Self::RakNet(r) = self {
            r.handle_datagram(now, data);
        }
    }

    pub fn handle_message(&mut self, msg: Bytes) {
        if let Self::Message { inbound, .. } = self {
            inbound.push_back(raknet::Event::Message(msg));
        }
    }

    pub fn handle_timeout(&mut self, now: Instant) {
        if let Self::RakNet(r) = self {
            r.handle_timeout(now);
        }
    }

    pub fn poll_timeout(&self) -> Option<Instant> {
        match self {
            Self::RakNet(r) => r.poll_timeout(),
            Self::Message { .. } => None,
        }
    }

    /// RakNet datagrams, or whole game messages on a message link.
    pub fn poll_transmit(&mut self, now: Instant) -> Option<Bytes> {
        match self {
            Self::RakNet(r) => r.poll_transmit(now),
            Self::Message { outbound, .. } => outbound.pop_front(),
        }
    }

    pub fn poll_event(&mut self) -> Option<raknet::Event> {
        match self {
            Self::RakNet(r) => r.poll_event(),
            Self::Message { inbound, .. } => inbound.pop_front(),
        }
    }

    /// Closes the link and drops whatever it had not yet delivered.
    pub fn close(&mut self, now: Instant) {
        match self {
            Self::RakNet(r) => {
                r.close(now);
                while r.poll_event().is_some() {}
            }
            Self::Message { inbound, .. } => inbound.clear(),
        }
    }
}
