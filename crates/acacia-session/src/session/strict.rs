//! Strict mode: reports what the server sends that a schema-exact, in-order peer would not
//! (docs/testing.md, "Strict mode").

use std::fmt;

use acacia_proto::packets::{
    Disconnect, JigsawStructureData, NetworkSettings, PlayStatus, PlayStatusStatus, ResourcePackStack, ServerToClientHandshake, StartGame,
};
use acacia_proto::{DecodeError, Packet, RawPacket, packet_name};

use super::{Event, Session};

#[derive(Debug, Clone, PartialEq)]
pub struct Violation {
    /// Id of the packet at fault.
    pub packet: u32,
    pub reason: Reason,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reason {
    /// The strict decoder rejected the packet (`acacia_proto::strict`).
    Decode(DecodeError),
    /// A join-order rule, by its text in docs/testing.md.
    Order(&'static str),
    /// The packet decodes but its payload is unusable (layers above the session report these).
    Content(String),
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = packet_name(self.packet).unwrap_or("unknown packet");
        match &self.reason {
            Reason::Decode(e) => write!(f, "{e}"),
            Reason::Order(rule) => write!(f, "{name}: {rule}"),
            Reason::Content(what) => write!(f, "{name}: {what}"),
        }
    }
}

/// What the join has reached, for the order rules.
#[derive(Default)]
pub(super) struct JoinOrder {
    settings: bool,
    handshake: bool,
    logged_in: bool,
    pack_stack: bool,
    jigsaw: bool,
    start_game: bool,
}

impl JoinOrder {
    /// Reports every order rule `raw` breaks.
    fn observe(&mut self, raw: &RawPacket, mut report: impl FnMut(&'static str)) {
        let seen = |flag: &mut bool| std::mem::replace(flag, true);
        match raw.id {
            NetworkSettings::ID if seen(&mut self.settings) => report("second NetworkSettings"),
            ServerToClientHandshake::ID if seen(&mut self.handshake) => report("second ServerToClientHandshake"),
            NetworkSettings::ID | ServerToClientHandshake::ID | Disconnect::ID => {}
            PlayStatus::ID => match raw.decode::<PlayStatus>().map(|p| p.status) {
                Ok(PlayStatusStatus::LoginSuccess) => self.logged_in = true,
                Ok(PlayStatusStatus::PlayerSpawn) if !self.start_game => report("PlayerSpawn before StartGame"),
                _ => {}
            },
            _ if !self.logged_in => report("sent before PlayStatus(LoginSuccess)"),
            ResourcePackStack::ID => self.pack_stack = true,
            JigsawStructureData::ID => self.jigsaw = true,
            StartGame::ID => {
                if seen(&mut self.start_game) {
                    return report("second StartGame");
                }
                if !self.pack_stack {
                    report("StartGame before ResourcePackStack");
                }
                if !self.jigsaw {
                    report("StartGame without JigsawStructureData before it");
                }
            }
            _ => {}
        }
    }
}

impl Session {
    /// Strict mode only: queues an [`Event::Violation`] for everything wrong with `raw`.
    pub(super) fn audit(&mut self, raw: &RawPacket) {
        let Some(order) = &mut self.strict else { return };
        let events = &mut self.events;
        let mut report = |reason| {
            let violation = Violation { packet: raw.id, reason };
            tracing::warn!(%violation, "strict mode");
            events.push_back(Event::Violation(violation));
        };
        if let Err(e) = acacia_proto::strict::check(raw) {
            report(Reason::Decode(e));
        }
        order.observe(raw, |rule| report(Reason::Order(rule)));
    }
}

#[cfg(test)]
mod tests {
    use acacia_proto::encode_packet;
    use bytes::BytesMut;

    use super::*;

    fn raw<T: Packet>(packet: &T) -> RawPacket {
        let mut buf = BytesMut::new();
        encode_packet(packet, &mut buf);
        RawPacket::parse(buf.freeze()).unwrap()
    }

    fn bare(id: u32) -> RawPacket {
        RawPacket { id, sender_subclient: 0, target_subclient: 0, body: Default::default() }
    }

    fn rules(order: &mut JoinOrder, raw: &RawPacket) -> Vec<&'static str> {
        let mut out = Vec::new();
        order.observe(raw, |rule| out.push(rule));
        out
    }

    #[test]
    fn a_join_in_order_breaks_no_rule() {
        let mut order = JoinOrder::default();
        let login = raw(&PlayStatus { status: PlayStatusStatus::LoginSuccess });
        let spawn = raw(&PlayStatus { status: PlayStatusStatus::PlayerSpawn });
        let join = [bare(NetworkSettings::ID), bare(ServerToClientHandshake::ID), login, bare(ResourcePackStack::ID), bare(JigsawStructureData::ID), bare(StartGame::ID), spawn];
        for packet in &join {
            assert_eq!(rules(&mut order, packet), Vec::<&str>::new(), "packet {}", packet.id);
        }
    }

    #[test]
    fn start_game_reports_each_missing_step() {
        let mut order = JoinOrder::default();
        assert_eq!(rules(&mut order, &bare(StartGame::ID)), ["sent before PlayStatus(LoginSuccess)"]);
        rules(&mut order, &raw(&PlayStatus { status: PlayStatusStatus::LoginSuccess }));
        assert_eq!(
            rules(&mut order, &bare(StartGame::ID)),
            ["StartGame before ResourcePackStack", "StartGame without JigsawStructureData before it"]
        );
        assert_eq!(rules(&mut order, &bare(StartGame::ID)), ["second StartGame"]);
    }

    #[test]
    fn repeats_and_an_early_spawn_are_reported() {
        let mut order = JoinOrder::default();
        rules(&mut order, &bare(NetworkSettings::ID));
        assert_eq!(rules(&mut order, &bare(NetworkSettings::ID)), ["second NetworkSettings"]);
        let spawn = raw(&PlayStatus { status: PlayStatusStatus::PlayerSpawn });
        assert_eq!(rules(&mut order, &spawn), ["PlayerSpawn before StartGame"]);
    }
}
