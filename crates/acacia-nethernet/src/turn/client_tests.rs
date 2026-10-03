use std::net::SocketAddr;
use std::time::{Duration, Instant};

use super::stun::{self, Attr, Class, Message, Method};
use super::{TurnClient, TurnEvent};

const USER: &str = "user";
const PASS: &str = "pass";
const REALM: &str = "turn.example";

fn addr(s: &str) -> SocketAddr {
    s.parse().unwrap()
}

/// Scripted TURN server: answers whatever the test picks, signed with the long-term key.
struct FakeServer {
    key: [u8; 16],
    nonce: String,
}

impl FakeServer {
    fn new() -> Self {
        Self { key: stun::long_term_key(USER, REALM, PASS), nonce: "nonce-1".into() }
    }

    fn next(&self, client: &mut TurnClient) -> Message {
        self.check(&client.poll_transmit().expect("client sent nothing"))
    }

    fn check(&self, raw: &[u8]) -> Message {
        let msg = Message::decode(raw).unwrap();
        if msg.class == Class::Request && msg.nonce().is_some() {
            assert!(stun::verify_integrity(raw, &self.key), "{:?} not signed", msg.method);
            assert_eq!(msg.nonce(), Some(self.nonce.clone()));
        }
        msg
    }

    fn reply(&self, req: &Message, attrs: Vec<Attr>) -> Vec<u8> {
        let msg = Message { class: Class::Success, method: req.method, transaction_id: req.transaction_id, attrs };
        msg.encode(Some(&self.key), true)
    }

    fn challenge(&self, req: &Message, code: u16) -> Vec<u8> {
        Message::new(Class::Error, req.method, req.transaction_id)
            .with(Attr::ErrorCode { code, reason: "challenge".into() })
            .with(Attr::Realm(REALM.into()))
            .with(Attr::Nonce(self.nonce.clone()))
            .encode(None, true)
    }

    /// Answers every queued request with success, returning their methods.
    fn answer_all(&self, client: &mut TurnClient, now: Instant) -> Vec<Method> {
        let raws: Vec<_> = std::iter::from_fn(|| client.poll_transmit()).collect();
        let mut methods: Vec<_> = raws
            .iter()
            .map(|raw| {
                let req = self.check(raw);
                assert!(client.handle_datagram(now, &self.reply(&req, vec![Attr::Lifetime(600)])).is_none());
                req.method
            })
            .collect();
        methods.sort_by_key(|m| *m as u16);
        methods
    }
}

#[test]
fn allocates_binds_relays_and_refreshes() {
    let (f, t0) = (FakeServer::new(), Instant::now());
    let (peer, relayed, mapped) = (addr("198.51.100.20:5000"), addr("192.0.2.10:49152"), addr("203.0.113.5:40000"));
    let mut c = TurnClient::new(addr("192.0.2.10:3478"), USER, PASS, t0);

    let alloc = f.next(&mut c);
    assert_eq!((alloc.method, &alloc.attrs[..]), (Method::Allocate, &[Attr::RequestedTransport(17)][..]));
    assert!(c.handle_datagram(t0, &f.challenge(&alloc, 401)).is_none());
    let alloc = f.next(&mut c);
    assert_eq!(alloc.attrs[1], Attr::Username(USER.into()));
    let ok = f.reply(&alloc, vec![Attr::XorRelayedAddress(relayed), Attr::XorMappedAddress(mapped), Attr::Lifetime(600)]);
    c.handle_datagram(t0, &ok);
    assert_eq!(c.poll_event(), Some(TurnEvent::Allocated { relayed, mapped: Some(mapped) }));

    c.create_permission(peer, t0);
    let perm = f.next(&mut c);
    assert_eq!((perm.method, perm.xor_peer()), (Method::CreatePermission, Some(peer)));
    c.handle_datagram(t0, &f.reply(&perm, vec![]));

    c.send_to(peer, b"hello", t0);
    let send = f.next(&mut c);
    assert_eq!((send.class, send.method, send.xor_peer()), (Class::Indication, Method::Send, Some(peer)));
    assert_eq!(send.into_data().as_deref(), Some(&b"hello"[..]));
    let bind = f.next(&mut c);
    assert_eq!(bind.attrs[..2], [Attr::ChannelNumber(0x4000), Attr::XorPeerAddress(peer)]);
    c.handle_datagram(t0, &f.reply(&bind, vec![]));
    c.send_to(peer, b"again", t0);
    assert_eq!(c.poll_transmit(), Some(stun::channel_data(0x4000, b"again")));

    let data = Message::new(Class::Indication, Method::Data, stun::transaction_id())
        .with(Attr::XorPeerAddress(peer))
        .with(Attr::Data(b"from peer".to_vec()));
    assert_eq!(c.handle_datagram(t0, &data.encode(None, false)), Some((peer, b"from peer".to_vec())));
    assert_eq!(c.handle_datagram(t0, &stun::channel_data(0x4000, b"cd")), Some((peer, b"cd".to_vec())));
    assert_eq!(c.handle_datagram(t0, &stun::channel_data(0x4001, b"cd")), None);
    assert!(c.poll_transmit().is_none());

    let secs = |s| t0 + Duration::from_secs(s);
    for at in [240, 480] {
        assert_eq!(c.poll_timeout(), Some(secs(at)));
        c.handle_timeout(secs(at));
        assert_eq!(f.answer_all(&mut c, secs(at)), [Method::CreatePermission, Method::ChannelBind]);
    }
    assert_eq!(c.poll_timeout(), Some(secs(540)));
    c.handle_timeout(secs(540));
    let refresh = f.next(&mut c);
    assert_eq!((refresh.method, refresh.lifetime()), (Method::Refresh, None));
    let f = FakeServer { nonce: "nonce-2".into(), ..f };
    c.handle_datagram(secs(540), &f.challenge(&refresh, 438));
    let refresh = f.next(&mut c);
    c.handle_datagram(secs(540), &f.reply(&refresh, vec![Attr::Lifetime(600)]));
    assert_eq!(c.poll_timeout(), Some(secs(720)));
    assert_eq!(c.relayed(), Some(relayed));

    c.close(secs(600));
    let release = f.next(&mut c);
    assert_eq!((release.method, release.lifetime()), (Method::Refresh, Some(0)));
    c.handle_datagram(secs(600), &f.reply(&release, vec![]));
    assert!(c.is_done() && c.poll_timeout().is_none() && c.poll_event().is_none());
}

#[test]
fn retransmits_then_fails() {
    let mut c = TurnClient::new(addr("192.0.2.10:3478"), USER, PASS, Instant::now());
    let first = c.poll_transmit().unwrap();
    let mut sends = 1;
    while let Some(at) = c.poll_timeout() {
        c.handle_timeout(at);
        while let Some(again) = c.poll_transmit() {
            assert_eq!(again, first);
            sends += 1;
        }
    }
    assert_eq!(sends, 9);
    assert!(matches!(c.poll_event(), Some(TurnEvent::Failed(_))));
}

#[test]
fn rejects_forged_success_and_fails_on_second_401() {
    let (f, t0) = (FakeServer::new(), Instant::now());
    let peer = addr("198.51.100.20:5000");
    let mut c = TurnClient::new(addr("192.0.2.10:3478"), USER, PASS, t0);
    c.create_permission(peer, t0);
    let alloc = f.next(&mut c);
    assert!(c.poll_transmit().is_none(), "no permission before the allocation");
    c.handle_datagram(t0, &f.challenge(&alloc, 401));
    let alloc = f.next(&mut c);
    let forged = FakeServer { key: [0; 16], ..FakeServer::new() };
    c.handle_datagram(t0, &forged.reply(&alloc, vec![Attr::XorRelayedAddress(peer)]));
    assert_eq!(c.poll_event(), None);
    c.handle_datagram(t0, &f.challenge(&alloc, 401));
    assert!(matches!(c.poll_event(), Some(TurnEvent::Failed(e)) if e.contains("401")));
    assert!(c.is_done());
}

#[test]
fn queued_peers_get_permissions_once_allocated() {
    let (f, t0) = (FakeServer::new(), Instant::now());
    let mut c = TurnClient::new(addr("192.0.2.10:3478"), USER, PASS, t0);
    c.create_permission(addr("198.51.100.20:5000"), t0);
    let alloc = f.next(&mut c);
    c.handle_datagram(t0, &f.reply(&alloc, vec![Attr::XorRelayedAddress(addr("192.0.2.10:49152"))]));
    assert!(matches!(c.poll_event(), Some(TurnEvent::Allocated { mapped: None, .. })));
    assert_eq!(f.next(&mut c).method, Method::CreatePermission);
}
