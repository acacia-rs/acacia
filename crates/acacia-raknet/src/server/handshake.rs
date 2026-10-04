//! The offline half of the server: pings and the two open-connection rounds.
//! Who gets a reply 2 follows RakNet's `ProcessOfflineNetworkPacket`.

use std::net::SocketAddr;
use std::time::Instant;

use bytes::BytesMut;

use super::{Peer, Server};
use crate::conn::{Conn, ConnConfig};
use crate::wire::{offline as o, WireError};

impl Server {
    pub(super) fn handle_offline(&mut self, now: Instant, from: SocketAddr, id: u8, data: &[u8]) -> Result<(), WireError> {
        let mut out = BytesMut::new();
        match id {
            _ if o::is_unconnected_ping(id) => {
                let time = o::parse_unconnected_ping(data)?;
                o::unconnected_pong(&mut out, time, self.cfg.guid, &self.cfg.motd);
            }
            o::ID_OPEN_CONNECTION_REQUEST_1 => self.open_1(now, from, data, &mut out)?,
            o::ID_OPEN_CONNECTION_REQUEST_2 => self.open_2(now, from, data, &mut out)?,
            _ => {}
        }
        if !out.is_empty() {
            self.outbox.push_back((from, out.freeze()));
        }
        Ok(())
    }

    fn open_1(&mut self, now: Instant, from: SocketAddr, data: &[u8], out: &mut BytesMut) -> Result<(), WireError> {
        let (protocol, mtu) = o::parse_request_1(data)?;
        if mtu < o::MIN_MTU {
            return Ok(());
        }
        if self.banned.contains(&from.ip()) {
            o::refusal(out, o::ID_CONNECTION_BANNED, self.cfg.guid);
            return Ok(());
        }
        if protocol != self.cfg.protocol_version {
            o::incompatible_protocol(out, self.cfg.protocol_version, self.cfg.guid);
            return Ok(());
        }
        let cookie = self.cfg.cookies.then(|| self.cookies.issue(from, now.saturating_duration_since(self.epoch)));
        o::reply_1(out, self.cfg.guid, cookie, mtu.min(self.cfg.max_mtu));
        Ok(())
    }

    fn open_2(&mut self, now: Instant, from: SocketAddr, data: &[u8], out: &mut BytesMut) -> Result<(), WireError> {
        let request = o::parse_request_2(data, self.cfg.cookies)?;
        let age = now.saturating_duration_since(self.epoch);
        if request.cookie.is_some_and(|c| !self.cookies.verify(from, age, c)) || request.mtu < o::MIN_MTU {
            return Ok(());
        }
        let refuse = match self.peers.get(&from) {
            _ if self.banned.contains(&from.ip()) => Some(o::ID_CONNECTION_BANNED),
            // Our reply 2 got lost, or the client restarted mid-handshake: start over.
            Some(p) if !p.connected && p.guid == request.client_guid => None,
            Some(_) => Some(o::ID_ALREADY_CONNECTED),
            None if self.peers.len() >= self.cfg.max_peers => Some(o::ID_NO_FREE_INCOMING_CONNECTIONS),
            None => None,
        };
        if let Some(id) = refuse {
            o::refusal(out, id, self.cfg.guid);
            return Ok(());
        }
        let mtu = request.mtu.min(self.cfg.max_mtu);
        let cfg = ConnConfig {
            mtu,
            idle_timeout: self.cfg.idle_timeout,
            ping_interval: self.cfg.ping_interval,
            recv_limits: self.cfg.recv_limits,
            congestion_window: true,
        };
        let mut peer = Peer::new(Conn::new(self.epoch, cfg, now), request.client_guid, now + self.cfg.handshake_timeout);
        self.schedule.touch(from, &mut peer);
        self.peers.insert(from, peer);
        o::reply_2(out, self.cfg.guid, from, mtu);
        Ok(())
    }
}
