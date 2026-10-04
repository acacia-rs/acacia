use std::net::SocketAddr;

use bytes::{BufMut, BytesMut};

use super::{Reader, Result, WireError, WriteExt};

pub const ID_UNCONNECTED_PING: u8 = 0x01;
pub const ID_OPEN_CONNECTION_REQUEST_1: u8 = 0x05;
pub const ID_OPEN_CONNECTION_REPLY_1: u8 = 0x06;
pub const ID_OPEN_CONNECTION_REQUEST_2: u8 = 0x07;
pub const ID_OPEN_CONNECTION_REPLY_2: u8 = 0x08;
pub const ID_ALREADY_CONNECTED: u8 = 0x12;
pub const ID_NO_FREE_INCOMING_CONNECTIONS: u8 = 0x14;
pub const ID_CONNECTION_BANNED: u8 = 0x17;
pub const ID_INCOMPATIBLE_PROTOCOL_VERSION: u8 = 0x19;
pub const ID_IP_RECENTLY_CONNECTED: u8 = 0x1a;
pub const ID_UNCONNECTED_PONG: u8 = 0x1c;

/// IPv4 + UDP header bytes that the MTU includes but a datagram payload cannot use.
pub const UDP_OVERHEAD: u16 = 28;
/// The smallest MTU RakNet clients try, and the smallest any IPv4 host must accept.
pub const MIN_MTU: u16 = 576;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pong {
    pub time: i64,
    pub server_guid: u64,
    pub motd: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reply1 {
    pub server_guid: u64,
    pub cookie: Option<u32>,
    pub mtu: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request2 {
    pub cookie: Option<u32>,
    pub mtu: u16,
    pub client_guid: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reply2 {
    pub server_guid: u64,
    pub client_addr: SocketAddr,
    pub mtu: u16,
}

fn expect_id(r: &mut Reader, id: u8) -> Result<()> {
    if r.u8()? == id { Ok(()) } else { Err(WireError::Malformed("unexpected packet id")) }
}

pub fn unconnected_ping(out: &mut BytesMut, time: i64, client_guid: u64) {
    out.put_u8(ID_UNCONNECTED_PING);
    out.put_i64(time);
    out.put_magic();
    out.put_u64(client_guid);
}

pub fn parse_pong(data: &[u8]) -> Result<Pong> {
    let mut r = Reader::new(data);
    expect_id(&mut r, ID_UNCONNECTED_PONG)?;
    let time = r.i64_be()?;
    let server_guid = r.u64_be()?;
    r.magic()?;
    let len = r.u16_be()?;
    let motd = String::from_utf8_lossy(r.take(len.into())?).into_owned();
    Ok(Pong { time, server_guid, motd })
}

/// Padded so the datagram plus IP/UDP headers equals `mtu`: the server infers our MTU from its size.
pub fn open_connection_request_1(out: &mut BytesMut, protocol: u8, mtu: u16) {
    let total = usize::from(mtu.saturating_sub(UDP_OVERHEAD));
    out.put_u8(ID_OPEN_CONNECTION_REQUEST_1);
    out.put_magic();
    out.put_u8(protocol);
    out.put_bytes(0, total.saturating_sub(out.len()));
}

pub fn parse_reply_1(data: &[u8]) -> Result<Reply1> {
    let mut r = Reader::new(data);
    expect_id(&mut r, ID_OPEN_CONNECTION_REPLY_1)?;
    r.magic()?;
    let server_guid = r.u64_be()?;
    let cookie = if r.bool()? { Some(r.u32_be()?) } else { None };
    let mtu = r.u16_be()?;
    Ok(Reply1 { server_guid, cookie, mtu })
}

pub fn open_connection_request_2(
    out: &mut BytesMut,
    cookie: Option<u32>,
    server: SocketAddr,
    mtu: u16,
    client_guid: u64,
) {
    out.put_u8(ID_OPEN_CONNECTION_REQUEST_2);
    out.put_magic();
    if let Some(cookie) = cookie {
        out.put_u32(cookie);
        out.put_u8(0); // client did not solve a security challenge
    }
    out.put_addr(server);
    out.put_u16(mtu);
    out.put_u64(client_guid);
}

pub fn parse_reply_2(data: &[u8]) -> Result<Reply2> {
    let mut r = Reader::new(data);
    expect_id(&mut r, ID_OPEN_CONNECTION_REPLY_2)?;
    r.magic()?;
    let server_guid = r.u64_be()?;
    let client_addr = r.addr()?;
    let mtu = r.u16_be()?;
    Ok(Reply2 { server_guid, client_addr, mtu })
}

/// Both ping IDs: 0x02 ("open connections only") is answered the same way.
pub fn is_unconnected_ping(id: u8) -> bool {
    matches!(id, ID_UNCONNECTED_PING | 0x02)
}

/// The ping's timestamp, echoed in the pong.
pub fn parse_unconnected_ping(data: &[u8]) -> Result<i64> {
    let mut r = Reader::new(data);
    r.u8()?;
    r.i64_be()
}

pub fn unconnected_pong(out: &mut BytesMut, time: i64, server_guid: u64, motd: &str) {
    out.put_u8(ID_UNCONNECTED_PONG);
    out.put_i64(time);
    out.put_u64(server_guid);
    out.put_magic();
    out.put_u16(motd.len() as u16);
    out.put_slice(motd.as_bytes());
}

/// Returns `(protocol, mtu)`: the client pads the request to its MTU minus IP/UDP headers.
pub fn parse_request_1(data: &[u8]) -> Result<(u8, u16)> {
    let mut r = Reader::new(data);
    expect_id(&mut r, ID_OPEN_CONNECTION_REQUEST_1)?;
    r.magic()?;
    let protocol = r.u8()?;
    Ok((protocol, (data.len() as u16).saturating_add(UDP_OVERHEAD)))
}

/// With a cookie the reply says "has security", and request 2 must echo the cookie.
pub fn reply_1(out: &mut BytesMut, server_guid: u64, cookie: Option<u32>, mtu: u16) {
    out.put_u8(ID_OPEN_CONNECTION_REPLY_1);
    out.put_magic();
    out.put_u64(server_guid);
    out.put_u8(u8::from(cookie.is_some()));
    if let Some(cookie) = cookie {
        out.put_u32(cookie);
    }
    out.put_u16(mtu);
}

/// A refusal such as [`ID_ALREADY_CONNECTED`] or [`ID_NO_FREE_INCOMING_CONNECTIONS`].
pub fn refusal(out: &mut BytesMut, id: u8, server_guid: u64) {
    out.put_u8(id);
    out.put_magic();
    out.put_u64(server_guid);
}

pub fn incompatible_protocol(out: &mut BytesMut, protocol: u8, server_guid: u64) {
    out.put_u8(ID_INCOMPATIBLE_PROTOCOL_VERSION);
    out.put_u8(protocol);
    out.put_magic();
    out.put_u64(server_guid);
}

/// The request carries a cookie only if reply 1 did, and nothing in it says so: pass `with_cookie`
/// as the server sent it.
pub fn parse_request_2(data: &[u8], with_cookie: bool) -> Result<Request2> {
    let mut r = Reader::new(data);
    expect_id(&mut r, ID_OPEN_CONNECTION_REQUEST_2)?;
    r.magic()?;
    let cookie = if with_cookie {
        let cookie = r.u32_be()?;
        r.u8()?; // whether the client solved a security challenge
        Some(cookie)
    } else {
        None
    };
    r.addr()?;
    Ok(Request2 { cookie, mtu: r.u16_be()?, client_guid: r.u64_be()? })
}

pub fn reply_2(out: &mut BytesMut, server_guid: u64, client: SocketAddr, mtu: u16) {
    out.put_u8(ID_OPEN_CONNECTION_REPLY_2);
    out.put_magic();
    out.put_u64(server_guid);
    out.put_addr(client);
    out.put_u16(mtu);
    out.put_u8(0); // no encryption
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_replies_parse_as_the_client_reads_them() {
        let mut buf = BytesMut::new();
        open_connection_request_1(&mut buf, 11, 1400);
        assert_eq!(parse_request_1(&buf).unwrap(), (11, 1400));
        let mut buf = BytesMut::new();
        reply_1(&mut buf, 9, None, 1400);
        assert_eq!(parse_reply_1(&buf).unwrap(), Reply1 { server_guid: 9, cookie: None, mtu: 1400 });
        let mut buf = BytesMut::new();
        reply_1(&mut buf, 9, Some(0xdead_beef), 1400);
        assert_eq!(parse_reply_1(&buf).unwrap(), Reply1 { server_guid: 9, cookie: Some(0xdead_beef), mtu: 1400 });

        let client: SocketAddr = "10.0.0.2:5000".parse().unwrap();
        for cookie in [None, Some(0xdead_beef)] {
            let mut buf = BytesMut::new();
            open_connection_request_2(&mut buf, cookie, "10.0.0.1:19132".parse().unwrap(), 1400, 77);
            assert_eq!(parse_request_2(&buf, cookie.is_some()).unwrap(), Request2 { cookie, mtu: 1400, client_guid: 77 });
        }
        let mut buf = BytesMut::new();
        reply_2(&mut buf, 9, client, 1400);
        assert_eq!(parse_reply_2(&buf).unwrap(), Reply2 { server_guid: 9, client_addr: client, mtu: 1400 });

        let mut buf = BytesMut::new();
        unconnected_ping(&mut buf, 123, 5);
        assert_eq!(parse_unconnected_ping(&buf).unwrap(), 123);
        let mut buf = BytesMut::new();
        unconnected_pong(&mut buf, 123, 9, "MCPE;x");
        assert_eq!(parse_pong(&buf).unwrap(), Pong { time: 123, server_guid: 9, motd: "MCPE;x".into() });
    }

    #[test]
    fn request_1_is_padded_to_mtu() {
        let mut buf = BytesMut::new();
        open_connection_request_1(&mut buf, 11, 1400);
        assert_eq!(buf.len(), 1400 - 28);
        assert_eq!(buf[17], 11);
    }

    #[test]
    fn reply_1_with_and_without_cookie() {
        let mut buf = BytesMut::new();
        buf.put_u8(ID_OPEN_CONNECTION_REPLY_1);
        buf.put_magic();
        buf.put_u64(42);
        buf.put_u8(1);
        buf.put_u32(0xdead_beef);
        buf.put_u16(1400);
        assert_eq!(
            parse_reply_1(&buf).unwrap(),
            Reply1 { server_guid: 42, cookie: Some(0xdead_beef), mtu: 1400 }
        );

        let mut buf = BytesMut::new();
        buf.put_u8(ID_OPEN_CONNECTION_REPLY_1);
        buf.put_magic();
        buf.put_u64(42);
        buf.put_u8(0);
        buf.put_u16(1200);
        assert_eq!(parse_reply_1(&buf).unwrap().cookie, None);
    }
}
