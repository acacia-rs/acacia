//! Login state. `LoginAcknowledged` is an empty body ([`super::Empty`]); `Hello` and `LoginCompression` are
//! [`super::LoginHello`] and [`super::LoginCompression`].

use crate::server::read_rest;
use crate::server::status::json_string;
use crate::{Error, VarInt, Wire};

pub use crate::ids::login::{clientbound, serverbound};

/// `GameProfile.properties` entry: name (64), value (32767), nullable signature (1024).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProfileProperty {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

impl Wire for ProfileProperty {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let (name, value) = (String::read(input)?, String::read(input)?);
        let signature = if bool::read(input)? { Some(String::read(input)?) } else { None };
        Ok(Self { name, value, signature })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.name.write(out);
        self.value.write(out);
        self.signature.is_some().write(out);
        if let Some(signature) = &self.signature {
            signature.write(out);
        }
    }
}

/// `ByteBufCodecs.GAME_PROFILE`: uuid, name (16), then the properties list (at most 16).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GameProfile {
    pub uuid: u128,
    pub name: String,
    pub properties: Vec<ProfileProperty>,
}

impl Wire for GameProfile {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { uuid: u128::read(input)?, name: String::read(input)?, properties: Vec::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.uuid.write(out);
        self.name.write(out);
        self.properties.write(out);
    }
}

/// `ClientboundLoginFinishedPacket`: the profile, then the session id. The client-side
/// [`crate::packets::LoginFinished`] stops after the name.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LoginFinishedPacket {
    pub profile: GameProfile,
    pub session_id: u128,
}

impl Wire for LoginFinishedPacket {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { profile: GameProfile::read(input)?, session_id: u128::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.profile.write(out);
        self.session_id.write(out);
    }
}

/// `ClientboundLoginDisconnectPacket`: the reason as a JSON component string (`lenientJson(262144)`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LoginDisconnect {
    pub reason_json: String,
}

impl LoginDisconnect {
    pub fn text(message: &str) -> Self {
        Self { reason_json: json_string(message) }
    }
}

impl Wire for LoginDisconnect {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { reason_json: String::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.reason_json.write(out);
    }
}

/// `ClientboundCustomQueryPacket`: transaction, channel, then the payload to the end of the frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CustomQuery {
    pub transaction: VarInt,
    pub channel: String,
    pub data: Vec<u8>,
}

impl Wire for CustomQuery {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { transaction: VarInt::read(input)?, channel: String::read(input)?, data: read_rest(input) })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.transaction.write(out);
        self.channel.write(out);
        out.extend_from_slice(&self.data);
    }
}

/// `ServerboundCustomQueryAnswerPacket`: transaction, then a presence flag and the payload to the end of the
/// frame. `None` is a client that did not understand the query.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CustomQueryAnswer {
    pub transaction: VarInt,
    pub data: Option<Vec<u8>>,
}

impl Wire for CustomQueryAnswer {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let transaction = VarInt::read(input)?;
        let data = if bool::read(input)? { Some(read_rest(input)) } else { None };
        Ok(Self { transaction, data })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.transaction.write(out);
        self.data.is_some().write(out);
        if let Some(data) = &self.data {
            out.extend_from_slice(data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{Empty, LoginCompression, LoginHello, round_trip};

    #[test]
    fn ids_match_the_26_3_reports() {
        assert_eq!(
            [clientbound::LOGIN_DISCONNECT, clientbound::HELLO, clientbound::LOGIN_FINISHED, clientbound::LOGIN_COMPRESSION, clientbound::CUSTOM_QUERY],
            [0, 1, 2, 3, 4]
        );
        assert_eq!([serverbound::HELLO, serverbound::KEY, serverbound::CUSTOM_QUERY_ANSWER, serverbound::LOGIN_ACKNOWLEDGED], [0, 1, 2, 3]);
    }

    #[test]
    fn hello_and_compression_round_trip() {
        let bytes = round_trip(LoginHello { name: "Bob".into(), uuid: 1 });
        assert_eq!(bytes.len(), 1 + 3 + 16);
        assert_eq!(round_trip(LoginCompression { threshold: VarInt(256) }), [0x80, 0x02]);
        assert!(round_trip(Empty {}).is_empty());
    }

    #[test]
    fn login_finished_writes_uuid_name_properties_then_session() {
        let packet = LoginFinishedPacket {
            profile: GameProfile {
                uuid: 7,
                name: "Bob".into(),
                properties: vec![
                    ProfileProperty { name: "textures".into(), value: "v".into(), signature: Some("s".into()) },
                    ProfileProperty { name: "x".into(), value: "y".into(), signature: None },
                ],
            },
            session_id: 9,
        };
        let bytes = round_trip(packet);
        assert_eq!(&bytes[16..21], &[3, b'B', b'o', b'b', 2]);
        assert_eq!(bytes.len(), 16 + 4 + 1 + (1 + 8 + 1 + 1 + 1 + 1 + 1) + (1 + 1 + 1 + 1 + 1 + 0) + 16);
    }

    #[test]
    fn login_finished_is_prefix_compatible_with_the_client_packet() {
        let mut bytes = Vec::new();
        LoginFinishedPacket { profile: GameProfile { uuid: 5, name: "Al".into(), properties: Vec::new() }, session_id: 1 }.write(&mut bytes);
        let seen = crate::packets::LoginFinished::read(&mut &bytes[..]).unwrap();
        assert_eq!((seen.uuid, seen.name.as_str()), (5, "Al"));
    }

    #[test]
    fn disconnect_is_a_json_string() {
        let bytes = round_trip(LoginDisconnect::text("no \"way\""));
        assert_eq!(&bytes[1..], br#""no \"way\"""#);
    }

    #[test]
    fn custom_query_and_answer_carry_raw_tails() {
        let query = CustomQuery { transaction: VarInt(1), channel: "acacia:auth".into(), data: b"25565".to_vec() };
        let bytes = round_trip(query);
        assert_eq!(bytes[0], 1);
        assert!(bytes.ends_with(b"25565"));
        assert_eq!(round_trip(CustomQueryAnswer { transaction: VarInt(1), data: Some(vec![1, 2, 3]) }), [1, 1, 1, 2, 3]);
        assert_eq!(round_trip(CustomQueryAnswer { transaction: VarInt(1), data: None }), [1, 0]);
    }
}
