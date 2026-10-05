//! Our clients against one [`Host`] address, datagrams passed by hand on a virtual clock.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytes::Bytes;
use p384::ecdsa::SigningKey;

use super::{Host, HostConfig, HostEvent, PeerId};
use crate::{Connection, Error, Identity, LocalCandidate};

const HOST: &str = "10.0.0.2:19132";

struct Net {
    host: Host,
    clients: Vec<(SocketAddr, Connection)>,
    events: Vec<HostEvent>,
    now: Instant,
}

fn host_key() -> SigningKey {
    SigningKey::from_slice(&[9; 48]).unwrap()
}

fn identity(seed: u8) -> Identity {
    Identity::self_signed(SigningKey::from_slice(&[seed; 48]).unwrap())
}

impl Net {
    fn new(config: HostConfig) -> Self {
        Self { host: Host::new(host_key(), config), clients: Vec::new(), events: Vec::new(), now: Instant::now() }
    }

    fn host_addr() -> SocketAddr {
        HOST.parse().unwrap()
    }

    /// A direct-connect join from `addr`: the offer answered and the answer accepted.
    fn join(&mut self, addr: &str, seed: u8) -> PeerId {
        let addr: SocketAddr = addr.parse().unwrap();
        let (mut client, offer) = Connection::offer(addr, &identity(seed), self.now).unwrap();
        let answer = self.host.answer(&offer, Self::host_addr(), self.now).unwrap();
        assert!(answer.candidates.is_empty() && answer.sdp.contains(" 10.0.0.2 19132 typ host "), "{}", answer.sdp);
        client.accept_answer(&answer.sdp, self.now).unwrap();
        assert_eq!(client.server_key(), Some(host_key().verifying_key()));
        self.clients.push((addr, client));
        answer.peer
    }

    /// Delivers datagrams both ways until nobody has more. Ones the host does not claim are lost,
    /// as they are on a socket shared with RakNet.
    fn trade(&mut self) {
        loop {
            let mut moved = false;
            for (addr, client) in &mut self.clients {
                while let Some(t) = client.poll_transmit() {
                    self.host.handle_datagram(self.now, *addr, &t.contents);
                    moved = true;
                }
            }
            while let Some((to, data)) = self.host.poll_transmit() {
                if let Some((_, client)) = self.clients.iter_mut().find(|(addr, _)| *addr == to) {
                    client.handle_datagram(self.now, Self::host_addr(), &data);
                }
                moved = true;
            }
            self.events.extend(std::iter::from_fn(|| self.host.poll_event()));
            if !moved {
                return;
            }
        }
    }

    fn tick(&mut self) {
        let timers = self.clients.iter().filter_map(|(_, c)| c.poll_timeout()).chain(self.host.poll_timeout());
        self.now = timers.min().unwrap_or(self.now).max(self.now + Duration::from_millis(1));
        for (_, client) in &mut self.clients {
            client.handle_timeout(self.now);
        }
        self.host.handle_timeout(self.now);
    }

    fn run_until(&mut self, mut done: impl FnMut(&mut Self) -> bool) {
        let end = self.now + Duration::from_secs(40);
        loop {
            self.trade();
            if done(self) {
                return;
            }
            assert!(self.now < end, "not done after 40 s; host events: {:?}", self.events);
            self.tick();
        }
    }

    fn messages(&self, peer: PeerId) -> Vec<&[u8]> {
        self.events.iter().filter_map(|e| matches!(e, HostEvent::Message(p, _) if *p == peer).then(|| e.payload())).collect()
    }
}

impl HostEvent {
    fn payload(&self) -> &[u8] {
        match self {
            HostEvent::Message(_, msg) => msg,
            _ => &[],
        }
    }
}

fn with_big_stack(test: fn()) {
    std::thread::Builder::new().stack_size(32 << 20).spawn(test).unwrap().join().unwrap();
}

#[test]
fn two_clients_share_the_host_address() {
    with_big_stack(|| {
        let mut net = Net::new(HostConfig::default());
        let (a, b) = (net.join("10.0.0.1:5000", 5), net.join("10.0.0.3:6000", 6));
        net.clients[0].1.send(Bytes::from_static(b"from a"), net.now);
        net.clients[1].1.send(Bytes::from_static(b"from b"), net.now);
        net.run_until(|n| !n.messages(a).is_empty() && !n.messages(b).is_empty());
        assert_eq!((net.messages(a), net.messages(b)), (vec![&b"from a"[..]], vec![&b"from b"[..]]));
        let opened = |p| net.events.iter().position(|e| *e == HostEvent::Open(p));
        assert!(opened(a).is_some() && opened(b).is_some(), "{:?}", net.events);
        assert_eq!(net.host.remote_addr(b), Some("10.0.0.3:6000".parse().unwrap()));

        net.host.send(a, Bytes::from_static(b"to a"), net.now);
        net.host.send(b, Bytes::from_static(b"to b"), net.now);
        let mut got = [None, None];
        net.run_until(|n| {
            for (slot, (_, client)) in got.iter_mut().zip(&mut n.clients) {
                *slot = slot.take().or_else(|| client.poll_message());
            }
            got.iter().all(Option::is_some)
        });
        assert_eq!(got, [Some(Bytes::from_static(b"to a")), Some(Bytes::from_static(b"to b"))]);

        let now = net.now;
        net.clients[0].1.close(now);
        net.run_until(|n| n.events.iter().any(|e| matches!(e, HostEvent::Closed(p, _) if *p == a)));
        assert_eq!(net.host.len(), 1);
        assert!(!net.host.handle_datagram(net.now, "10.0.0.1:5000".parse().unwrap(), &[0x16, 0xfe, 0xfd, 0, 0]));
    });
}

#[test]
fn trickled_join_opens() {
    with_big_stack(|| {
        let mut net = Net::new(HostConfig::default());
        let addr: SocketAddr = "10.0.0.1:5000".parse().unwrap();
        let (mut client, offer) = Connection::trickle_offer(&identity(5), net.now).unwrap();
        let answer = net.host.trickle_answer(&offer, Net::host_addr(), net.now).unwrap();
        assert!(!answer.sdp.contains("a=candidate") && answer.candidates.len() == 1, "{answer:?}");
        client.add_remote_candidate(&answer.candidates[0], net.now).unwrap();
        client.accept_answer(&answer.sdp, net.now).unwrap();
        let line = client.add_local_candidate(LocalCandidate::Host(addr), net.now).unwrap();
        net.host.add_remote_candidate(answer.peer, &line, net.now).unwrap();
        net.clients.push((addr, client));
        net.run_until(|n| n.events.contains(&HostEvent::Open(answer.peer)));
    });
}

#[test]
fn silent_offers_time_out_and_a_full_host_refuses() {
    let mut net = Net::new(HostConfig { max_peers: 1, handshake_timeout: Duration::from_secs(15) });
    let (_, offer) = Connection::offer("10.0.0.1:5000".parse().unwrap(), &identity(5), net.now).unwrap();
    let peer = net.host.answer(&offer, Net::host_addr(), net.now).unwrap().peer;
    assert!(matches!(net.host.answer(&offer, Net::host_addr(), net.now), Err(Error::HostFull)));
    // Nothing of ours: a RakNet unconnected ping.
    assert!(!net.host.handle_datagram(net.now, "10.0.0.9:1".parse().unwrap(), &[0x01, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0xff, 0xff, 0]));

    let start = net.now;
    net.run_until(|n| n.events.iter().any(|e| matches!(e, HostEvent::Closed(p, _) if *p == peer)));
    let waited = net.now - start;
    assert!((Duration::from_secs(15)..Duration::from_secs(16)).contains(&waited), "{waited:?}");
    assert!(net.host.is_empty() && net.host.answer(&offer, Net::host_addr(), net.now).is_ok());
}
