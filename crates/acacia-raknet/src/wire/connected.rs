use std::net::{Ipv4Addr, SocketAddr};

use bytes::{BufMut, Bytes, BytesMut};

use super::{Reader, Result, WriteExt};

pub const ID_CONNECTED_PING: u8 = 0x00;
pub const ID_CONNECTED_PONG: u8 = 0x03;
pub const ID_CONNECTION_REQUEST: u8 = 0x09;
pub const ID_CONNECTION_REQUEST_ACCEPTED: u8 = 0x10;
pub const ID_NEW_INCOMING_CONNECTION: u8 = 0x13;
pub const ID_DISCONNECTION_NOTIFICATION: u8 = 0x15;

const INTERNAL_ADDRESS_COUNT: usize = 20;

pub fn connection_request(client_guid: u64, time: i64) -> Bytes {
    let mut out = BytesMut::with_capacity(18);
    out.put_u8(ID_CONNECTION_REQUEST);
    out.put_u64(client_guid);
    out.put_i64(time);
    out.put_u8(0); // no security
    out.freeze()
}

/// Returns the server's timestamp. The accepted packet holds a variable number of internal
/// addresses, so we skip straight to the two trailing timestamps.
pub fn parse_request_accepted(data: &[u8]) -> Result<i64> {
    let mut r = Reader::new(data);
    r.u8()?;
    r.addr()?;
    r.u16_be()?;
    while r.remaining() > 16 {
        r.addr()?;
    }
    r.i64_be()?;
    r.i64_be()
}

/// Returns the client's timestamp, echoed in the acceptance.
pub fn parse_connection_request(data: &[u8]) -> Result<i64> {
    let mut r = Reader::new(data);
    r.u8()?;
    r.u64_be()?;
    r.i64_be()
}

pub fn connection_request_accepted(client: SocketAddr, request_time: i64, time: i64) -> Bytes {
    let mut out = BytesMut::with_capacity(8 + 7 * (INTERNAL_ADDRESS_COUNT + 1) + 18);
    out.put_u8(ID_CONNECTION_REQUEST_ACCEPTED);
    out.put_addr(client);
    out.put_u16(0); // system index
    let unspecified = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
    for _ in 0..INTERNAL_ADDRESS_COUNT {
        out.put_addr(unspecified);
    }
    out.put_i64(request_time);
    out.put_i64(time);
    out.freeze()
}

pub fn new_incoming_connection(server: SocketAddr, ping_time: i64, pong_time: i64) -> Bytes {
    let mut out = BytesMut::with_capacity(8 + 7 * (INTERNAL_ADDRESS_COUNT + 1) + 16);
    out.put_u8(ID_NEW_INCOMING_CONNECTION);
    out.put_addr(server);
    let unspecified = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
    for _ in 0..INTERNAL_ADDRESS_COUNT {
        out.put_addr(unspecified);
    }
    out.put_i64(ping_time);
    out.put_i64(pong_time);
    out.freeze()
}

pub fn connected_ping(time: i64) -> Bytes {
    let mut out = BytesMut::with_capacity(9);
    out.put_u8(ID_CONNECTED_PING);
    out.put_i64(time);
    out.freeze()
}

pub fn connected_pong(ping_time: i64, pong_time: i64) -> Bytes {
    let mut out = BytesMut::with_capacity(17);
    out.put_u8(ID_CONNECTED_PONG);
    out.put_i64(ping_time);
    out.put_i64(pong_time);
    out.freeze()
}

/// Returns the timestamp of a connected ping, or the echoed ping time of a pong.
pub fn parse_timestamp(data: &[u8]) -> Result<i64> {
    let mut r = Reader::new(data);
    r.u8()?;
    r.i64_be()
}

pub fn disconnection_notification() -> Bytes {
    Bytes::from_static(&[ID_DISCONNECTION_NOTIFICATION])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_accepted_skips_variable_address_list() {
        let mut out = BytesMut::new();
        out.put_u8(ID_CONNECTION_REQUEST_ACCEPTED);
        out.put_addr("1.2.3.4:5".parse().unwrap());
        out.put_u16(0);
        for _ in 0..10 {
            out.put_addr("0.0.0.0:0".parse().unwrap());
        }
        out.put_i64(111);
        out.put_i64(222);
        assert_eq!(parse_request_accepted(&out).unwrap(), 222);
    }

    #[test]
    fn accepted_roundtrips_with_the_client_parser() {
        let request = connection_request(5, 111);
        let echoed = parse_connection_request(&request).unwrap();
        let accepted = connection_request_accepted("1.2.3.4:5".parse().unwrap(), echoed, 222);
        assert_eq!(parse_request_accepted(&accepted).unwrap(), 222);
    }
}
