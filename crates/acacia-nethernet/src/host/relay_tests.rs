//! A host behind a NAT nobody gets through: its STUN-mapped and TURN-relayed candidates, and a client
//! joining through the relay alone. Datagrams pass by hand on a virtual clock.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use bytes::Bytes;
use p384::ecdsa::SigningKey;

use super::{Host, HostConfig, HostEvent, IceConfig, TurnServer};
use crate::turn::stun::{self, Attr, Class, Message, Method};
use crate::{Connection, Identity};

const HOST: &str = "10.0.0.2:19132";
const STUN: &str = "192.0.2.1:3478";
const TURN: &str = "192.0.2.10:3478";
const RELAY: &str = "192.0.2.10:49152";
const MAPPED: &str = "203.0.113.1:19132";
const CLIENT: &str = "198.51.100.7:4000";

fn addr(s: &str) -> SocketAddr {
    s.parse().unwrap()
}

/// Just enough of a TURN server: no auth, every request succeeds.
#[derive(Default)]
struct FakeTurn {
    permits: HashSet<IpAddr>,
    channels: HashMap<u16, SocketAddr>,
}

/// An answer for the allocation's owner, and a datagram on to a peer.
type Handled = (Option<Vec<u8>>, Option<(SocketAddr, Vec<u8>)>);

impl FakeTurn {
    /// From the allocation's owner: answers for it, and what goes on to peers.
    fn owner_sent(&mut self, data: &[u8]) -> Handled {
        if !stun::is_stun(data) {
            let (channel, payload) = stun::parse_channel_data(data).unwrap();
            return (None, Some((self.channels[&channel], payload.to_vec())));
        }
        let msg = Message::decode(data).unwrap();
        let mut reply = Message::new(Class::Success, msg.method, msg.transaction_id);
        match (msg.class, msg.method) {
            (Class::Indication, Method::Send) => {
                let peer = msg.xor_peer().unwrap();
                assert!(self.permits.contains(&peer.ip()), "send to {peer} without a permission");
                return (None, Some((peer, msg.into_data().unwrap())));
            }
            (_, Method::Allocate) => reply = reply.with(Attr::XorRelayedAddress(addr(RELAY))).with(Attr::XorMappedAddress(addr(MAPPED))).with(Attr::Lifetime(600)),
            (_, Method::CreatePermission) => {
                self.permits.insert(msg.xor_peer().unwrap().ip());
            }
            (_, Method::ChannelBind) => {
                let channel = msg.attrs.iter().find_map(|a| if let Attr::ChannelNumber(n) = a { Some(*n) } else { None }).unwrap();
                self.channels.insert(channel, msg.xor_peer().unwrap());
            }
            _ => {}
        }
        (Some(reply.encode(None, true)), None)
    }

    /// From a peer to the relay: what reaches the owner, if its address is permitted.
    fn peer_sent(&self, peer: SocketAddr, data: &[u8]) -> Option<Vec<u8>> {
        if !self.permits.contains(&peer.ip()) {
            return None;
        }
        Some(match self.channels.iter().find(|(_, p)| **p == peer) {
            Some((&channel, _)) => stun::channel_data(channel, data),
            None => Message::new(Class::Indication, Method::Data, stun::transaction_id())
                .with(Attr::XorPeerAddress(peer))
                .with(Attr::Data(data.to_vec()))
                .encode(None, false),
        })
    }
}

struct Net {
    host: Host,
    turn: FakeTurn,
    client: Option<Connection>,
    events: Vec<HostEvent>,
    now: Instant,
}

impl Net {
    /// Host datagrams to the client are lost: only the relay reaches it.
    fn trade(&mut self) {
        loop {
            let mut moved = false;
            while let Some((to, data)) = self.host.poll_transmit() {
                moved = true;
                if to == addr(STUN) {
                    let request = Message::decode(&data).unwrap();
                    let answer = Message::new(Class::Success, Method::Binding, request.transaction_id).with(Attr::XorMappedAddress(addr(MAPPED)));
                    assert!(self.host.handle_datagram(self.now, to, &answer.encode(None, true)));
                } else if to == addr(TURN) {
                    let (reply, onward) = self.turn.owner_sent(&data);
                    if let Some(reply) = reply {
                        assert!(self.host.handle_datagram(self.now, to, &reply));
                    }
                    if let (Some((peer, payload)), Some(client)) = (onward, self.client.as_mut()) {
                        assert_eq!(peer, addr(CLIENT));
                        client.handle_datagram(self.now, addr(RELAY), &payload);
                    }
                }
            }
            while let Some(t) = self.client.as_mut().and_then(Connection::poll_transmit) {
                moved = true;
                if let Some(wrapped) = (t.destination == addr(RELAY)).then(|| self.turn.peer_sent(addr(CLIENT), &t.contents)).flatten() {
                    assert!(self.host.handle_datagram(self.now, addr(TURN), &wrapped));
                }
            }
            self.events.extend(std::iter::from_fn(|| self.host.poll_event()));
            if !moved {
                return;
            }
        }
    }

    fn run_until(&mut self, mut done: impl FnMut(&mut Self) -> bool) {
        let end = self.now + Duration::from_secs(40);
        loop {
            self.trade();
            if done(self) {
                return;
            }
            assert!(self.now < end, "not done after 40 s; host events: {:?}", self.events);
            let timers = self.client.as_ref().and_then(Connection::poll_timeout).into_iter().chain(self.host.poll_timeout());
            self.now = timers.min().unwrap_or(self.now).max(self.now + Duration::from_millis(1));
            if let Some(client) = self.client.as_mut() {
                client.handle_timeout(self.now);
            }
            self.host.handle_timeout(self.now);
        }
    }
}

#[test]
fn a_client_joins_through_the_hosts_relay() {
    std::thread::Builder::new().stack_size(32 << 20).spawn(join_through_relay).unwrap().join().unwrap();
}

fn join_through_relay() {
    let ice = IceConfig { stun: vec![addr(STUN)], turn: vec![TurnServer { addr: addr(TURN), username: "u".into(), password: "p".into() }] };
    let host = Host::new(SigningKey::from_slice(&[9; 48]).unwrap(), HostConfig { ice, ..HostConfig::default() });
    let mut net = Net { host, turn: FakeTurn::default(), client: None, events: Vec::new(), now: Instant::now() };
    assert!(net.host.gathered(addr(HOST)).is_empty());
    net.host.gather(net.now);
    net.trade();
    assert_eq!(net.host.gathered(addr(HOST)).len(), 2, "one srflx (STUN and TURN agree) and one relay");

    let identity = Identity::self_signed(SigningKey::from_slice(&[5; 48]).unwrap());
    let (mut client, offer) = Connection::offer(addr(CLIENT), &identity, net.now).unwrap();
    let answer = net.host.answer(&offer, addr(HOST), net.now).unwrap();
    assert!(answer.sdp.contains(" 203.0.113.1 19132 typ srflx raddr 10.0.0.2 rport 19132 "), "{}", answer.sdp);
    assert!(answer.sdp.contains(" 192.0.2.10 49152 typ relay raddr 203.0.113.1 rport 19132 "), "{}", answer.sdp);
    client.accept_answer(&answer.sdp, net.now).unwrap();
    net.client = Some(client);
    net.run_until(|n| n.events.contains(&HostEvent::Open(answer.peer)));
    assert_eq!(net.host.remote_addr(answer.peer), Some(addr(CLIENT)));

    net.host.send(answer.peer, Bytes::from_static(b"relayed"), net.now);
    let mut got = None;
    net.run_until(|n| {
        got = got.take().or_else(|| n.client.as_mut()?.poll_message());
        got.is_some()
    });
    assert_eq!(got.as_deref(), Some(&b"relayed"[..]));
}
