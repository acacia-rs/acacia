//! The world a LAN host advertises in its discovery response (version 7; go-nethernet
//! `discovery/server_data.go`, MIT). Strings are varuint32-prefixed, ints zigzag varints.

use crate::Error;

const VERSION: u8 = 7;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerData {
    pub server_name: String,
    pub protocol: i32,
    pub version: String,
    pub level_name: String,
    pub player_count: i32,
    pub max_player_count: i32,
    /// 0 survival, 1 creative, 2 adventure.
    pub game_type: i32,
    pub editor_world: bool,
    pub hardcore: bool,
    pub accepts_online_auth: bool,
    pub accepts_self_signed_auth: bool,
    pub nonce: String,
    /// Vanilla hosts send 4.
    pub connection_type: i32,
}

impl ServerData {
    pub fn decode(data: &[u8]) -> Result<Self, Error> {
        let mut r = data;
        let version = byte(&mut r)?;
        if version != VERSION {
            return Err(Error::Lan(format!("ServerData version {version}, want {VERSION}")));
        }
        let d = Self {
            server_name: string(&mut r)?,
            protocol: varint(&mut r)?,
            version: string(&mut r)?,
            level_name: string(&mut r)?,
            player_count: varint(&mut r)?,
            max_player_count: varint(&mut r)?,
            game_type: varint(&mut r)?,
            editor_world: byte(&mut r)? != 0,
            hardcore: byte(&mut r)? != 0,
            accepts_online_auth: byte(&mut r)? != 0,
            accepts_self_signed_auth: byte(&mut r)? != 0,
            nonce: string(&mut r)?,
            connection_type: varint(&mut r)?,
        };
        if !r.is_empty() {
            return Err(Error::Lan("trailing ServerData bytes".into()));
        }
        Ok(d)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut b = vec![VERSION];
        put_string(&mut b, &self.server_name);
        put_varint(&mut b, self.protocol);
        put_string(&mut b, &self.version);
        put_string(&mut b, &self.level_name);
        put_varint(&mut b, self.player_count);
        put_varint(&mut b, self.max_player_count);
        put_varint(&mut b, self.game_type);
        b.extend([self.editor_world, self.hardcore, self.accepts_online_auth, self.accepts_self_signed_auth].map(u8::from));
        put_string(&mut b, &self.nonce);
        put_varint(&mut b, self.connection_type);
        b
    }
}

fn byte(r: &mut &[u8]) -> Result<u8, Error> {
    let (&b, rest) = r.split_first().ok_or_else(|| Error::Lan("truncated ServerData".into()))?;
    *r = rest;
    Ok(b)
}

fn varuint(r: &mut &[u8]) -> Result<u32, Error> {
    let mut v = 0u32;
    for shift in (0..35).step_by(7) {
        let b = byte(r)?;
        v |= u32::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err(Error::Lan("varint too long".into()))
}

fn varint(r: &mut &[u8]) -> Result<i32, Error> {
    let u = varuint(r)?;
    Ok((u >> 1) as i32 ^ -((u & 1) as i32))
}

fn string(r: &mut &[u8]) -> Result<String, Error> {
    let n = varuint(r)? as usize;
    if n > r.len() {
        return Err(Error::Lan("truncated ServerData string".into()));
    }
    let (s, rest) = r.split_at(n);
    *r = rest;
    String::from_utf8(s.to_vec()).map_err(|_| Error::Lan("ServerData string is not UTF-8".into()))
}

fn put_varuint(b: &mut Vec<u8>, mut v: u32) {
    while v >= 0x80 {
        b.push(v as u8 | 0x80);
        v >>= 7;
    }
    b.push(v as u8);
}

fn put_varint(b: &mut Vec<u8>, v: i32) {
    put_varuint(b, ((v << 1) ^ (v >> 31)) as u32);
}

fn put_string(b: &mut Vec<u8>, s: &str) {
    put_varuint(b, s.len() as u32);
    b.extend_from_slice(s.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// go-nethernet `server_data_test.go` vector.
    const GN: &[u8] = &[
        0x07, 0x06, b's', b'e', b'r', b'v', b'e', b'r', 0x8A, 0x22, 0x07, b'1', b'.', b'2', b'6', b'.', b'5', b'0', 0x05,
        b'w', b'o', b'r', b'l', b'd', 0x02, 0x10, 0x04, 0x00, 0x01, 0x01, 0x01, 0x05, b'n', b'o', b'n', b'c', b'e', 0x08,
    ];

    #[test]
    fn matches_go_nethernet_vector() {
        let d = ServerData::decode(GN).unwrap();
        assert_eq!((d.server_name.as_str(), d.protocol, d.version.as_str(), d.level_name.as_str()), ("server", 2181, "1.26.50", "world"));
        assert_eq!((d.player_count, d.max_player_count, d.game_type, d.connection_type), (1, 8, 2, 4));
        assert_eq!((d.editor_world, d.hardcore, d.accepts_online_auth, d.accepts_self_signed_auth), (false, true, true, true));
        assert_eq!(d.encode(), GN);
        let neg = ServerData { protocol: -3, ..d };
        assert_eq!(ServerData::decode(&neg.encode()).unwrap(), neg);
    }
}
