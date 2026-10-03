//! One player proxied over NetherNet: the game's connection to us and ours to the server, each on
//! its own UDP socket, with the relay (relay.rs) between them.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use acacia_nethernet::{Connection, Event};
use serde_json::json;
use tokio::net::UdpSocket;

use crate::record::Recorder;
use crate::relay::Relay;

pub struct Link {
    pub game: Connection,
    pub game_udp: UdpSocket,
    pub up: Connection,
    pub up_udp: UdpSocket,
    pub relay: Relay,
}

fn lock(rec: &Mutex<Recorder>) -> MutexGuard<'_, Recorder> {
    rec.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Link {
    /// Relays until either side closes, then closes the other.
    pub async fn run(mut self, rec: Arc<Mutex<Recorder>>) {
        let (mut game_buf, mut up_buf) = (vec![0u8; 2048], vec![0u8; 2048]);
        let why = loop {
            let ended = self.pump(&rec);
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
                _ = tokio::time::sleep_until(deadline.into()) => {
                    let now = Instant::now();
                    self.game.handle_timeout(now);
                    self.up.handle_timeout(now);
                }
            }
        };
        println!("NetherNet player left: {why}");
        lock(&rec).write(json!({ "event": "closed" }));
    }

    /// Moves messages across. Once a side closes, starts closing the other and returns why.
    fn pump(&mut self, rec: &Mutex<Recorder>) -> Option<String> {
        let now = Instant::now();
        while let Some(msg) = self.game.poll_message() {
            match self.relay.on_game_message(&msg, &mut lock(rec)) {
                Ok(batch) => self.up.send(batch, now),
                Err(e) => return Some(self.close_both(e, now)),
            }
        }
        while let Some(msg) = self.up.poll_message() {
            match self.relay.on_server_message(&msg, &mut lock(rec)) {
                Ok(batches) => batches.into_iter().for_each(|b| self.game.send(b, now)),
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
