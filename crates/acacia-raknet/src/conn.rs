use std::time::{Duration, Instant};

use bytes::Bytes;

use crate::reliability::{RecvLimits, RecvState, SendQueue};
use crate::types::DisconnectReason;
use crate::wire::datagram::{decode_acks, Frame, Reliability, FLAG_ACK, FLAG_NACK};
use crate::wire::{connected as c, offline as o, Reader};

/// ACKs and NACKs leave on this tick, like RakNet's update loop, not per datagram: vanilla covers
/// ~4.6 datagrams per ACK after a 5 ms median wait (2026-10-03 capture; acacia-capdiff `pacing`).
const ACK_TICK: Duration = Duration::from_millis(10);

pub(crate) struct ConnConfig {
    pub mtu: u16,
    pub idle_timeout: Duration,
    pub ping_interval: Duration,
    pub recv_limits: RecvLimits,
    /// Send under a congestion window. Off for the client, whose pacing is part of how it looks.
    pub congestion_window: bool,
}

/// The connected half of a RakNet connection, shared by client and server: reliability, ACKs,
/// answering pings, sending our own and the idle timeout.
pub(crate) struct Conn {
    epoch: Instant,
    mtu: u16,
    idle_timeout: Duration,
    ping_interval: Duration,
    send: SendQueue,
    recv: RecvState,
    last_recv: Instant,
    last_ping: Instant,
    /// The tick that flushes pending ACKs and NACKs.
    ack_at: Option<Instant>,
}

impl Conn {
    pub fn new(epoch: Instant, cfg: ConnConfig, now: Instant) -> Self {
        Self {
            epoch,
            // The peer names the MTU; below the floor the payload arithmetic would underflow.
            mtu: cfg.mtu.max(o::MIN_MTU),
            idle_timeout: cfg.idle_timeout,
            ping_interval: cfg.ping_interval,
            send: SendQueue::new(cfg.congestion_window),
            recv: RecvState::new(cfg.recv_limits),
            last_recv: now,
            last_ping: now,
            ack_at: None,
        }
    }

    /// The first [`ACK_TICK`] boundary after `now`, counted from the connection's epoch.
    fn next_ack_tick(&self, now: Instant) -> Instant {
        let ticks = now.saturating_duration_since(self.epoch).as_nanos() / ACK_TICK.as_nanos() + 1;
        self.epoch + ACK_TICK * ticks as u32
    }

    pub fn mtu(&self) -> u16 {
        self.mtu
    }

    pub fn rtt(&self) -> Option<Duration> {
        self.send.rtt()
    }

    pub fn send_queue(&self) -> &SendQueue {
        &self.send
    }

    pub fn time(&self, now: Instant) -> i64 {
        now.saturating_duration_since(self.epoch).as_millis() as i64
    }

    fn max_payload(&self) -> usize {
        usize::from(self.mtu - o::UDP_OVERHEAD)
    }

    pub fn queue(&mut self, data: Bytes, reliability: Reliability) {
        let max = self.max_payload();
        self.send.push(data, reliability, 0, max);
    }

    /// Handles a datagram with the valid flag; reassembled messages wait in [`Conn::pop_message`].
    pub fn handle_datagram(&mut self, now: Instant, data: &Bytes) -> Result<(), DisconnectReason> {
        self.last_recv = now;
        let protocol = |e: &dyn std::fmt::Display| DisconnectReason::Protocol(e.to_string());
        let flags = data[0];
        if flags & FLAG_ACK != 0 {
            return decode_acks(data, |a, b| self.send.on_ack(now, a, b)).map_err(|e| protocol(&e));
        }
        if flags & FLAG_NACK != 0 {
            return decode_acks(data, |a, b| self.send.on_nack(a, b)).map_err(|e| protocol(&e));
        }
        let mut r = Reader::new(data);
        r.u8().map_err(|e| protocol(&e))?;
        self.recv.on_datagram(r.u24_le().map_err(|e| protocol(&e))?);
        if self.ack_at.is_none() {
            self.ack_at = Some(self.next_ack_tick(now));
        }
        while r.remaining() > 0 {
            let frame = Frame::decode(&mut r, data).map_err(|e| protocol(&e))?;
            self.recv.on_frame(frame, data.len()).map_err(|e| protocol(&e))?;
        }
        Ok(())
    }

    /// The next reassembled message, after answering connected pings and dropping pongs.
    pub fn pop_message(&mut self, now: Instant) -> Option<Bytes> {
        while let Some(msg) = self.recv.ready.pop_front() {
            match msg.first() {
                Some(&c::ID_CONNECTED_PING) => {
                    if let Ok(time) = c::parse_timestamp(&msg) {
                        self.send.push_control(c::connected_pong(time, self.time(now)));
                    }
                }
                Some(&c::ID_CONNECTED_PONG) | None => {}
                Some(_) => return Some(msg),
            }
        }
        None
    }

    pub fn poll_transmit(&mut self, now: Instant) -> Option<Bytes> {
        let max = self.max_payload();
        if self.ack_at.is_some_and(|t| t <= now) {
            if let Some(ack) = self.recv.poll_ack(max) {
                return Some(ack);
            }
            self.ack_at = None;
        }
        self.send.pack(now, max)
    }

    pub fn poll_timeout(&self) -> Instant {
        let idle = self.last_recv + self.idle_timeout;
        let ping = self.last_ping + self.ping_interval;
        let ack = self.ack_at.unwrap_or(idle);
        idle.min(ping).min(ack).min(self.send.next_resend().unwrap_or(idle))
    }

    pub fn handle_timeout(&mut self, now: Instant) -> Result<(), DisconnectReason> {
        if now >= self.last_recv + self.idle_timeout {
            return Err(DisconnectReason::Timeout);
        }
        self.send.on_timeout(now);
        if now >= self.last_ping + self.ping_interval {
            self.last_ping = now;
            self.send.push_control(c::connected_ping(self.time(now)));
        }
        Ok(())
    }

    /// Queues a disconnection notification and packs everything still queued into datagrams.
    pub fn close(&mut self, now: Instant) -> Vec<Bytes> {
        self.queue(c::disconnection_notification(), Reliability::ReliableOrdered);
        self.send.flush(now, self.max_payload())
    }
}

#[cfg(test)]
mod tests {
    use bytes::BytesMut;

    use super::*;
    use crate::wire::datagram::{put_datagram_header, Split};

    fn conn_with_mtu(now: Instant, mtu: u16) -> Conn {
        let (idle_timeout, ping_interval) = (Duration::from_secs(10), Duration::from_secs(5));
        let cfg = ConnConfig { mtu, idle_timeout, ping_interval, recv_limits: RecvLimits::SERVER, congestion_window: true };
        Conn::new(now, cfg, now)
    }

    fn conn(now: Instant) -> Conn {
        conn_with_mtu(now, 1400)
    }

    /// `n` reliable datagrams from a peer, one message each.
    fn datagrams(now: Instant, n: u8) -> Vec<Bytes> {
        let mut peer = conn(now);
        (0..n)
            .map(|i| {
                peer.queue(Bytes::from(vec![0xfe, i]), Reliability::ReliableOrdered);
                peer.send.pack(now, 1000).expect("one datagram per message")
            })
            .collect()
    }

    fn ack_ranges(datagram: &Bytes) -> Vec<(u32, u32)> {
        let mut ranges = Vec::new();
        decode_acks(datagram, |a, b| ranges.push((a, b))).unwrap();
        ranges
    }

    #[test]
    fn mtu_below_the_floor_is_raised() {
        let now = Instant::now();
        let mut c = conn_with_mtu(now, 20);
        c.queue(Bytes::from(vec![1u8; 5000]), Reliability::ReliableOrdered);
        let sent: Vec<Bytes> = std::iter::from_fn(|| c.poll_transmit(now)).collect();
        let max = usize::from(o::MIN_MTU - o::UDP_OVERHEAD);
        assert!(sent.len() > 5000 / max && sent.iter().all(|d| d.len() <= max), "{} datagrams", sent.len());
    }

    #[test]
    fn split_abuse_ends_the_connection() {
        let t0 = Instant::now();
        let mut c = conn(t0);
        let split = Some(Split { count: 100_000, id: 0, index: 0 });
        let frame = Frame { reliability: Reliability::Reliable, reliable_index: 0, sequence_index: 0, order_index: 0, order_channel: 0, split, body: Bytes::from_static(b"x") };
        let mut d = BytesMut::new();
        put_datagram_header(&mut d, 0);
        frame.encode(&mut d);
        assert_eq!(c.handle_datagram(t0, &d.freeze()), Err(DisconnectReason::Protocol("BadSplit".into())));
    }

    #[test]
    fn acks_wait_for_the_tick_and_cover_everything_since() {
        let t0 = Instant::now();
        let mut c = conn(t0);
        for d in datagrams(t0, 5) {
            c.handle_datagram(t0, &d).unwrap();
        }
        let early = c.poll_transmit(t0);
        assert_eq!(early, None, "no ACK before the tick; got {:02x?}", early.as_deref().map(|d| &d[..d.len().min(12)]));
        let tick = c.poll_timeout();
        assert!(tick > t0 && tick <= t0 + ACK_TICK, "{:?}", tick - t0);
        let ack = c.poll_transmit(tick).expect("one ACK at the tick");
        assert_eq!(ack[0] & FLAG_ACK, FLAG_ACK);
        assert_eq!(ack_ranges(&ack), [(0, 4)]);
        assert_eq!(c.poll_transmit(tick), None);
    }

    #[test]
    fn reordered_datagrams_are_not_nacked() {
        let t0 = Instant::now();
        let mut c = conn(t0);
        let d = datagrams(t0, 3);
        for i in [0, 2, 1] {
            c.handle_datagram(t0, &d[i]).unwrap();
        }
        let tick = c.poll_timeout();
        let sent: Vec<Bytes> = std::iter::from_fn(|| c.poll_transmit(tick)).collect();
        assert!(sent.iter().all(|d| d[0] & FLAG_NACK == 0), "datagram 1 arrived before the tick");
        assert_eq!(sent.iter().flat_map(ack_ranges).collect::<Vec<_>>(), [(0, 2)]);
    }
}
