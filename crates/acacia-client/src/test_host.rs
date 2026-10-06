//! A str0m answerer on a loopback UDP socket, standing in for a NetherNet host in dial tests. It
//! trickles its candidate after the CONNECTRESPONSE and signs the answer like BDS, or not, like a
//! game-hosted LAN world.

use std::time::{Duration, Instant};

use acacia_nethernet::{sign_answer, Signal, SignalKind};
use p384::ecdsa::SigningKey;
use str0m::change::SdpOffer;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Input, Output, Rtc};
use tokio::net::UdpSocket;

pub(crate) struct RtcHost {
    udp: UdpSocket,
    rtc: Option<Box<Rtc>>,
    buf: Vec<u8>,
    signs: bool,
}

impl RtcHost {
    pub async fn bind(signs: bool) -> Self {
        Self { udp: UdpSocket::bind("127.0.0.1:0").await.unwrap(), rtc: None, buf: vec![0; 2048], signs }
    }

    /// Handles a signal from the client; returns the signals to send back.
    pub fn on_signal(&mut self, signal: &Signal) -> Vec<Signal> {
        match signal.kind {
            SignalKind::ConnectRequest => self.answer(signal),
            SignalKind::CandidateAdd => {
                let rtc = self.rtc.as_mut().expect("candidate after the offer");
                rtc.add_remote_candidate(Candidate::from_sdp_string(&signal.data).unwrap());
                vec![]
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    fn answer(&mut self, offer: &Signal) -> Vec<Signal> {
        let mut rtc = Box::new(Rtc::builder().build(Instant::now()));
        rtc.add_local_candidate(Candidate::host(self.udp.local_addr().unwrap(), "udp").unwrap());
        let stripped: String = offer.data.lines().filter(|l| !l.starts_with("a=identity")).flat_map(|l| [l, "\r\n"]).collect();
        let answer = rtc.sdp_api().accept_offer(SdpOffer::from_sdp_string(&stripped).unwrap()).unwrap().to_sdp_string();
        let signed = match self.signs {
            true => {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
                sign_answer(&answer, &SigningKey::from_slice(&[9; 48]).unwrap(), now)
            }
            false => answer,
        };
        let (candidates, answer): (Vec<&str>, Vec<&str>) = signed.split_inclusive("\r\n").partition(|l| l.starts_with("a=candidate"));
        self.rtc = Some(rtc);
        let id = offer.connection_id;
        std::iter::once(Signal::new(SignalKind::ConnectResponse, id, answer.concat()))
            .chain(candidates.iter().map(|c| Signal::new(SignalKind::CandidateAdd, id, c.trim_end().trim_start_matches("a="))))
            .collect()
    }

    /// Sends str0m's output, then waits for one datagram or its next deadline.
    pub async fn step(&mut self) {
        let deadline = self.flush().await;
        tokio::select! {
            r = self.udp.recv_from(&mut self.buf) => {
                let (n, source) = r.unwrap();
                if let Some(rtc) = self.rtc.as_mut() {
                    let receive = Receive::new(Protocol::Udp, source, self.udp.local_addr().unwrap(), &self.buf[..n]).unwrap();
                    rtc.handle_input(Input::Receive(Instant::now(), receive)).unwrap();
                }
            }
            _ = tokio::time::sleep_until(deadline.into()) => {
                if let Some(rtc) = self.rtc.as_mut() {
                    rtc.handle_input(Input::Timeout(Instant::now())).unwrap();
                }
            }
        }
    }

    async fn flush(&mut self) -> Instant {
        let Some(rtc) = self.rtc.as_mut() else { return Instant::now() + Duration::from_secs(1) };
        loop {
            match rtc.poll_output().unwrap() {
                Output::Timeout(t) => return t,
                Output::Transmit(t) => {
                    self.udp.send_to(&t.contents, t.destination).await.unwrap();
                }
                // Echoes what the client sends: enough for a test to see its batches cross the link.
                Output::Event(str0m::Event::ChannelData(data)) => {
                    if let Some(mut channel) = rtc.channel(data.id) {
                        channel.write(data.binary, &data.data).unwrap();
                    }
                }
                Output::Event(_) => {}
            }
        }
    }
}
