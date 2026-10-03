use std::time::{Duration, Instant};

use bytes::Bytes;

use crate::reliability::{RecvState, SendQueue};
use crate::types::DisconnectReason;
use crate::wire::datagram::{decode_acks, Frame, Reliability, FLAG_ACK, FLAG_NACK};
use crate::wire::{connected as c, offline as o, Reader};

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
}

impl Conn {
    pub fn new(epoch: Instant, mtu: u16, idle_timeout: Duration, ping_interval: Duration, now: Instant) -> Self {
        Self { epoch, mtu, idle_timeout, ping_interval, send: SendQueue::new(), recv: RecvState::default(), last_recv: now, last_ping: now }
    }

    pub fn rtt(&self) -> Option<Duration> {
        self.send.rtt()
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
        while r.remaining() > 0 {
            let frame = Frame::decode(&mut r, data).map_err(|e| protocol(&e))?;
            self.recv.on_frame(frame).map_err(|e| protocol(&e))?;
        }
        Ok(())
    }

    /// The next reassembled message, after answering connected pings and dropping pongs.
    pub fn pop_message(&mut self, now: Instant) -> Option<Bytes> {
        while let Some(msg) = self.recv.ready.pop_front() {
            match msg.first() {
                Some(&c::ID_CONNECTED_PING) => {
                    if let Ok(time) = c::parse_timestamp(&msg) {
                        self.queue(c::connected_pong(time, self.time(now)), Reliability::Unreliable);
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
        self.recv.poll_ack(max).or_else(|| self.send.pack(now, max))
    }

    pub fn poll_timeout(&self) -> Instant {
        let idle = self.last_recv + self.idle_timeout;
        let ping = self.last_ping + self.ping_interval;
        idle.min(ping).min(self.send.next_resend().unwrap_or(idle))
    }

    pub fn handle_timeout(&mut self, now: Instant) -> Result<(), DisconnectReason> {
        if now >= self.last_recv + self.idle_timeout {
            return Err(DisconnectReason::Timeout);
        }
        self.send.on_timeout(now);
        if now >= self.last_ping + self.ping_interval {
            self.last_ping = now;
            self.queue(c::connected_ping(self.time(now)), Reliability::Unreliable);
        }
        Ok(())
    }

    /// Queues a disconnection notification and packs everything still queued into datagrams.
    pub fn close(&mut self, now: Instant) -> Vec<Bytes> {
        self.queue(c::disconnection_notification(), Reliability::ReliableOrdered);
        let max = self.max_payload();
        std::iter::from_fn(|| self.send.pack(now, max)).collect()
    }
}
