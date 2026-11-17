//! Status state. `StatusRequest` is an empty body ([`super::Empty`]); the response is the `ServerStatus` JSON as a
//! string (`ByteBufCodecs.lenientJson(262144)`), the ping and pong a single long (`ping` package).

use crate::{Error, Wire};
use crate::packets::packet;

pub use crate::ids::status::{clientbound, serverbound};

pub const MAX_JSON: usize = 262_144;

packet!(StatusResponse { json: String });
packet!(PingRequest { time: i64 });
packet!(PongResponse { time: i64 });

impl PongResponse {
    pub fn answering(ping: &PingRequest) -> Self {
        Self { time: ping.time }
    }
}

/// The `ServerStatus` fields a loopback server reports; the description is a plain-text component.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerStatus {
    pub description: String,
    pub version_name: String,
    pub protocol: i32,
    pub max_players: i32,
    pub online_players: i32,
    pub enforces_secure_chat: bool,
}

impl ServerStatus {
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"description":{},"players":{{"max":{},"online":{}}},"version":{{"name":{},"protocol":{}}},"enforcesSecureChat":{}}}"#,
            json_string(&self.description),
            self.max_players,
            self.online_players,
            json_string(&self.version_name),
            self.protocol,
            self.enforces_secure_chat,
        )
    }

    pub fn response(&self) -> StatusResponse {
        StatusResponse { json: self.to_json() }
    }
}

pub(crate) fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::round_trip;

    #[test]
    fn ids_match_the_26_3_reports() {
        assert_eq!((clientbound::STATUS_RESPONSE, clientbound::PONG_RESPONSE), (0, 1));
        assert_eq!((serverbound::STATUS_REQUEST, serverbound::PING_REQUEST), (0, 1));
    }

    #[test]
    fn status_response_carries_escaped_json() {
        let status = ServerStatus {
            description: "a \"b\"\n".into(),
            version_name: "26.3".into(),
            protocol: 777,
            max_players: 1,
            online_players: 0,
            enforces_secure_chat: false,
        };
        round_trip(status.response());
        assert!(status.to_json().starts_with(r#"{"description":"a \"b\"\u000a","players":{"max":1,"online":0}"#));
    }

    #[test]
    fn ping_and_pong_are_big_endian_longs() {
        let ping = PingRequest { time: 0x0102_0304_0506_0708 };
        assert_eq!(round_trip(ping.clone()), [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(round_trip(PongResponse::answering(&ping)), [1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
