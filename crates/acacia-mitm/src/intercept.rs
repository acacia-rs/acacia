//! Packet interception: per-player [`Interceptor`]s see each packet lazily as a [`RawPacket`] and
//! return a [`Verdict`]; an [`Injector`] sends new packets either way at any time.
//!
//! The proxy owns Login, NetworkSettings and ServerToClientHandshake (it re-signs, switches codecs
//! on, or replaces them), so those never reach interceptors. Injected packets bypass interceptors
//! and the capture, and wait until the direction's handshake is done: to the game after our
//! ServerToClientHandshake, to the server after the game's ClientToServerHandshake.

use std::net::SocketAddr;

use acacia_proto::{encode_packet, DecodeError, Packet, RawPacket};
use bytes::{Bytes, BytesMut};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    ToGame,
    ToServer,
}

/// What happens to an intercepted packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Forward,
    Drop,
    /// Sent in the packet's place, in order, in the same batch. Each is a full packet (header and
    /// body), as [`encode`] makes.
    Replace(Vec<Bytes>),
}

impl Verdict {
    pub fn replace<T: Packet>(packet: &T) -> Self {
        Self::Replace(vec![encode(packet)])
    }
}

/// One proxied player's hooks, one instance per player. Decode with `packet.decode::<T>()` only
/// for the ids you handle: the rest pass through without being parsed.
pub trait Interceptor: Send + 'static {
    fn on_game_packet(&mut self, _packet: &RawPacket) -> Verdict {
        Verdict::Forward
    }

    fn on_server_packet(&mut self, _packet: &RawPacket) -> Verdict {
        Verdict::Forward
    }
}

/// A packet with its header (both subclients 0), ready for [`Verdict::Replace`] or an [`Injector`].
pub fn encode<T: Packet>(packet: &T) -> Bytes {
    let mut buf = BytesMut::new();
    encode_packet(packet, &mut buf);
    buf.freeze()
}

/// One proxied player, as handed to interceptor factories.
pub struct Session {
    /// The game's address as the proxy sees it.
    pub game: SocketAddr,
    pub injector: Injector,
}

pub(crate) struct Injection {
    pub game: SocketAddr,
    pub dir: Direction,
    pub packet: Bytes,
}

/// Sends packets into one player's connection. Cheap to clone. Sends fail (`false`) once the
/// proxy has stopped; packets for a player who has left are dropped.
#[derive(Clone)]
pub struct Injector {
    game: SocketAddr,
    tx: mpsc::UnboundedSender<Injection>,
}

impl Injector {
    pub(crate) fn new(game: SocketAddr, tx: mpsc::UnboundedSender<Injection>) -> Self {
        Self { game, tx }
    }

    pub fn to_game<T: Packet>(&self, packet: &T) -> bool {
        self.raw(Direction::ToGame, encode(packet))
    }

    pub fn to_server<T: Packet>(&self, packet: &T) -> bool {
        self.raw(Direction::ToServer, encode(packet))
    }

    /// `packet` is a full packet, header included.
    pub fn raw(&self, dir: Direction, packet: Bytes) -> bool {
        self.tx.send(Injection { game: self.game, dir, packet }).is_ok()
    }
}

/// The player's interceptors in the order they were added: each sees what the previous one let
/// through or put in its place.
#[derive(Default)]
pub(crate) struct Chain(Vec<Box<dyn Interceptor>>);

impl Chain {
    pub fn new(interceptors: Vec<Box<dyn Interceptor>>) -> Self {
        Self(interceptors)
    }

    /// Runs `packet` (already parsed as `raw`) through the chain, appending what survives to `out`.
    pub fn run(&mut self, from_game: bool, packet: Bytes, raw: &RawPacket, out: &mut Vec<Bytes>) -> Result<(), DecodeError> {
        let Some((first, rest)) = self.0.split_first_mut() else {
            out.push(packet);
            return Ok(());
        };
        let mut current = apply(first.as_mut(), from_game, packet, raw);
        for interceptor in rest {
            let mut next = Vec::with_capacity(current.len());
            for p in current {
                let raw = RawPacket::parse(p.clone())?;
                next.extend(apply(interceptor.as_mut(), from_game, p, &raw));
            }
            current = next;
        }
        out.extend(current);
        Ok(())
    }
}

fn apply(interceptor: &mut dyn Interceptor, from_game: bool, packet: Bytes, raw: &RawPacket) -> Vec<Bytes> {
    let verdict = if from_game { interceptor.on_game_packet(raw) } else { interceptor.on_server_packet(raw) };
    match verdict {
        Verdict::Forward => vec![packet],
        Verdict::Drop => Vec::new(),
        Verdict::Replace(packets) => packets,
    }
}
