//! One player proxied over NetherNet: the game's connection to us and ours to the server, each on
//! its own UDP socket, with the relay (relay.rs) between them.

use std::time::{Duration, Instant};

use acacia_nethernet::{Connection, Event};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::intercept::Injection;
use crate::relay::{Out, Relay};

pub struct Link {
    pub game: Connection,
    pub game_udp: UdpSocket,
    pub up: Connection,
    pub up_udp: UdpSocket,
    pub relay: Relay,
    pub injections: mpsc::UnboundedReceiver<Injection>,
}

impl Link {
    /// Relays until either side closes, then closes the other.
    pub async fn run(mut self) {
        let (mut game_buf, mut up_buf) = (vec![0u8; 2048], vec![0u8; 2048]);
        let why = loop {
            let ended = self.pump();
            self.flush().await;
            if let Some(why) = ended {
                break why;
            }
            let now = Instant::now();
            let deadline = self.game.poll_timeout().into_iter().chain(self.up.poll_timeout()).min().unwrap_or(now + Duration::from_secs(1));
            // Receive errors are Windows' ICMP port-unreachable resets; the peers' ICE copes.
            tokio::select! {
                r = self.game_udp.recv_from(&mut game_buf) => if let Ok((n, from)) = r {
                    self.game.handle_datagram(Instant::now(), from, &game_buf[..n]);
                },
                r = self.up_udp.recv_from(&mut up_buf) => if let Ok((n, from)) = r {
                    self.up.handle_datagram(Instant::now(), from, &up_buf[..n]);
                },
                Some(first) = self.injections.recv() => {
                    let rest = std::iter::from_fn(|| self.injections.try_recv().ok());
                    let packets: Vec<_> = std::iter::once(first).chain(rest).map(|i| (i.dir, i.packet)).collect();
                    let out = self.relay.inject(packets);
                    self.route(out, Instant::now());
                },
                _ = tokio::time::sleep_until(deadline.into()) => {
                    let now = Instant::now();
                    self.game.handle_timeout(now);
                    self.up.handle_timeout(now);
                }
            }
        };
        println!("NetherNet player left: {why}");
        self.relay.close();
    }

    fn route(&mut self, out: Out, now: Instant) {
        out.to_server.into_iter().for_each(|b| self.up.send(b, now));
        out.to_game.into_iter().for_each(|b| self.game.send(b, now));
    }

    /// Moves messages across. Once a side closes, starts closing the other and returns why.
    fn pump(&mut self) -> Option<String> {
        let now = Instant::now();
        while let Some(msg) = self.game.poll_message() {
            match self.relay.on_game_message(&msg) {
                Ok(out) => self.route(out, now),
                Err(e) => return Some(self.close_both(e, now)),
            }
        }
        while let Some(msg) = self.up.poll_message() {
            match self.relay.on_server_message(&msg) {
                Ok(out) => self.route(out, now),
                Err(e) => return Some(self.close_both(e, now)),
            }
        }
        while let Some(event) = self.game.poll_event() {
            if let Event::Closed(why) = event {
                if let Some(bye) = self.relay.goodbye() {
                    self.up.send(bye, now);
                }
                self.up.close(now);
                return Some(format!("game side: {why}"));
            }
        }
        while let Some(event) = self.up.poll_event() {
            if let Event::Closed(why) = event {
                self.game.close(now);
                return Some(format!("server side: {why}"));
            }
        }
        None
    }

    fn close_both(&mut self, why: String, now: Instant) -> String {
        self.up.close(now);
        self.game.close(now);
        why
    }

    async fn flush(&mut self) {
        while let Some(t) = self.game.poll_transmit() {
            let _ = self.game_udp.send_to(&t.contents, t.destination).await;
        }
        while let Some(t) = self.up.poll_transmit() {
            let _ = self.up_udp.send_to(&t.contents, t.destination).await;
        }
    }
}
