//! Our client [`Connection`] against an in-process str0m answerer, datagrams passed by hand on a
//! virtual clock: signaling, ICE, DTLS, both channels, fragmentation and backpressure.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytes::Bytes;
use p384::ecdsa::SigningKey;
use str0m::change::SdpOffer;
use str0m::channel::ChannelId;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event as RtcEvent, Input, Output, Rtc};

use crate::frame::{fragments, Reassembler};
use crate::identity::IDENTITY;
use crate::server_identity::sign_as_server;
use crate::{Connection, Event, Identity, LocalCandidate};

const CLIENT: &str = "10.0.0.1:5000";
const SERVER: &str = "10.0.0.2:19132";

struct Server {
    rtc: Box<Rtc>,
    channels: Vec<(ChannelId, String)>,
    reassembler: Reassembler,
    messages: Vec<Bytes>,
}

impl Server {
    /// Answers the offer the way BDS does: strip the client assertion, add a signed one.
    fn answer(offer: &str, now: Instant) -> (Self, String) {
        let mut rtc = Box::new(Rtc::builder().build(now));
        rtc.add_local_candidate(Candidate::host(SERVER.parse().unwrap(), "udp").unwrap());
        let stripped: String = offer.lines().filter(|l| !l.starts_with(IDENTITY)).flat_map(|l| [l, "\r\n"]).collect();
        let offer = SdpOffer::from_sdp_string(&stripped).unwrap();
        let answer = rtc.sdp_api().accept_offer(offer).unwrap().to_sdp_string();
        let server = Self { rtc, channels: vec![], reassembler: Reassembler::default(), messages: vec![] };
        (server, sign_as_server(&answer, &SigningKey::from_slice(&[9; 48]).unwrap(), i64::MAX))
    }

    /// Runs until str0m waits; returns its next deadline and queues datagrams for the client.
    fn poll(&mut self, to_client: &mut Vec<Vec<u8>>) -> Instant {
        loop {
            match self.rtc.poll_output().unwrap() {
                Output::Timeout(t) => return t,
                Output::Transmit(t) => to_client.push(t.contents.to_vec()),
                Output::Event(RtcEvent::ChannelOpen(id, label)) => self.channels.push((id, label)),
                Output::Event(RtcEvent::ChannelData(d)) => {
                    if let Some(msg) = self.reassembler.push(&d.data).unwrap() {
                        self.messages.push(msg);
                    }
                }
                Output::Event(_) => {}
            }
        }
    }
}

/// Steps both sides until `done` holds or the virtual clock passes 30 s.
fn run(client: &mut Connection, server: &mut Server, now: &mut Instant, mut done: impl FnMut(&mut Connection, &mut Server) -> bool) {
    let (client_addr, server_addr): (SocketAddr, SocketAddr) = (CLIENT.parse().unwrap(), SERVER.parse().unwrap());
    let end = *now + Duration::from_secs(30);
    while *now < end {
        // Trade packets until both sides go quiet, then move the clock to the next deadline.
        // Each side is polled after every datagram, as a real driver does.
        let mut to_client = vec![];
        let server_deadline = loop {
            let mut deadline = server.poll(&mut to_client);
            let mut moved = !to_client.is_empty();
            for d in std::mem::take(&mut to_client) {
                client.handle_datagram(*now, server_addr, &d);
            }
            while let Some(t) = client.poll_transmit() {
                moved = true;
                assert_eq!((t.source, t.destination), (client_addr, server_addr));
                let receive = Receive::new(Protocol::Udp, client_addr, server_addr, &t.contents).unwrap();
                server.rtc.handle_input(Input::Receive(*now, receive)).unwrap();
                deadline = server.poll(&mut to_client);
            }
            if !moved && to_client.is_empty() {
                break deadline;
            }
        };
        if done(client, server) {
            return;
        }
        let next = client.poll_timeout().into_iter().chain([server_deadline]).min().unwrap();
        *now = next.max(*now + Duration::from_millis(1));
        client.handle_timeout(*now);
        server.rtc.handle_input(Input::Timeout(*now)).unwrap();
    }
    panic!(
        "loopback did not finish: client open={} server channels={:?} server messages={:?} client events={:?}",
        client.is_open(),
        server.channels,
        server.messages.iter().map(Bytes::len).collect::<Vec<_>>(),
        std::iter::from_fn(|| client.poll_event()).collect::<Vec<_>>(),
    );
}

#[test]
fn connects_with_trickled_candidates_both_ways() {
    let mut now = Instant::now();
    let identity = Identity::multiplayer(SigningKey::from_slice(&[5; 48]).unwrap(), "token".into());
    let (mut client, offer) = Connection::trickle_offer(&identity, now).unwrap();
    assert!(!offer.contains("a=candidate") && client.host_candidate().is_none());
    let (mut server, answer) = Server::answer(&offer, now);

    // The server trickles its candidate before the answer arrives; the client must hold it.
    let (candidates, answer): (Vec<&str>, Vec<&str>) = answer.split_inclusive("\r\n").partition(|l| l.starts_with("a=candidate"));
    assert_eq!(candidates.len(), 1);
    client.add_remote_candidate(candidates[0], now).unwrap();
    client.accept_answer(&answer.concat(), now).unwrap();

    let line = client.add_local_candidate(LocalCandidate::Host(CLIENT.parse().unwrap()), now).unwrap();
    assert!(line.contains(" 10.0.0.1 5000 typ host generation 0 ufrag "), "{line}");
    server.rtc.add_remote_candidate(Candidate::from_sdp_string(&line).unwrap());

    run(&mut client, &mut server, &mut now, |c, s| c.is_open() && s.channels.len() == 2);
}

#[test]
fn connects_and_exchanges_fragmented_messages() {
    // Unoptimized str0m/SCTP needs more than the 2 MiB test-thread stack for a 600 KB transfer.
    let test = std::thread::Builder::new().stack_size(32 << 20).spawn(exchange_fragmented_messages).unwrap();
    test.join().unwrap();
}

fn exchange_fragmented_messages() {
    let mut now = Instant::now();
    let identity = Identity::multiplayer(SigningKey::from_slice(&[5; 48]).unwrap(), "token".into());
    let (mut client, offer) = Connection::offer(CLIENT.parse().unwrap(), &identity, now).unwrap();
    let (mut server, answer) = Server::answer(&offer, now);
    client.accept_answer(&answer, now).unwrap();
    assert!(client.server_key().is_some());

    let big: Vec<u8> = (0..600_000u32).map(|i| i as u8).collect();
    client.send(Bytes::from_static(b"\x06\xc1\x01\x00\x00\x08\x91"), now);
    client.send(Bytes::from(big.clone()), now);
    run(&mut client, &mut server, &mut now, |_, s| s.messages.len() == 2);

    assert_eq!(client.poll_event(), Some(Event::Open));
    let labels: Vec<&str> = server.channels.iter().map(|(_, l)| l.as_str()).collect();
    assert_eq!(labels, ["ReliableDataChannel", "UnreliableDataChannel"]);
    assert_eq!(&server.messages[0][..], b"\x06\xc1\x01\x00\x00\x08\x91");
    assert_eq!(server.messages[1], big);

    let reliable = server.channels[0].0;
    let reply: Vec<u8> = vec![7; 300_000];
    let frames: Vec<Vec<u8>> = fragments(&reply, 65_536).unwrap().collect();
    let mut sent = 0;
    run(&mut client, &mut server, &mut now, |c, s| {
        while sent < frames.len() && s.rtc.channel(reliable).unwrap().write(true, &frames[sent]).unwrap() {
            sent += 1;
        }
        c.poll_message().is_some_and(|m| m == reply)
    });
}
