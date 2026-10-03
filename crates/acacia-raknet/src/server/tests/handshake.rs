use super::*;

#[test]
fn requests_below_the_mtu_floor_are_ignored() {
    let mut net = Net::new(config());
    let from = addr(50000);
    assert!(net.raw(from, request_1(100)).is_empty());
    for mtu in [0, 20, 44, o::MIN_MTU - 1] {
        let cookie = Some(net.cookie(from));
        assert!(net.raw(from, request_2(cookie, mtu, 2)).is_empty(), "mtu {mtu}");
    }
    assert!(net.server.peers.is_empty());
}

#[test]
fn request_2_needs_the_cookie_from_reply_1() {
    let mut net = Net::new(config());
    let (from, other) = (addr(50000), addr(50001));
    let reply_1 = net.raw(from, request_1(1400));
    let cookie = o::parse_reply_1(&reply_1[0]).unwrap().cookie.expect("reply 1 carries a cookie");

    assert!(net.raw(from, request_2(Some(cookie ^ 1), 1400, 2)).is_empty());
    assert!(net.raw(from, request_2(None, 1400, 2)).is_empty(), "no cookie at all");
    assert!(net.raw(other, request_2(Some(cookie), 1400, 2)).is_empty(), "a cookie is good for one address");
    assert!(net.server.peers.is_empty());

    let reply_2 = net.raw(from, request_2(Some(cookie), 1400, 2));
    assert_eq!(o::parse_reply_2(&reply_2[0]).unwrap(), o::Reply2 { server_guid: 1, client_addr: from, mtu: 1400 });
    let again = net.raw(from, request_2(Some(cookie), 1400, 2));
    assert_eq!(again, reply_2, "a lost reply 2 can be asked for again");
}

#[test]
fn spoofed_offline_datagrams_cannot_reset_a_connected_peer() {
    let mut net = Net::new(config());
    let client = net.join(50000);
    net.run(6);
    let victim = addr(50000);

    assert!(net.raw(victim, request_2(Some(0x1234_5678), 1400, 50000)).is_empty(), "a guessed cookie");
    assert!(net.raw(victim, request_2(None, 1400, 50000)).is_empty());
    assert!(net.raw(victim, BytesMut::from(&[o::ID_UNCONNECTED_PING][..])).is_empty(), "a truncated ping");
    let cookie = Some(net.cookie(victim));
    let refused = net.raw(victim, request_2(cookie, 1400, 50000));
    assert_eq!(refused[0][0], o::ID_ALREADY_CONNECTED, "even the address owner cannot replace a live connection");

    let hello = Bytes::from_static(b"\xfehello");
    assert!(net.server.send(victim, hello.clone(), Reliability::ReliableOrdered));
    net.run(2);
    assert_eq!(net.client_events(client).last(), Some(&Event::Message(hello)));
    assert_eq!(net.events, [ServerEvent::Connected(victim)]);
}

#[test]
fn cookies_can_be_turned_off() {
    let mut net = Net::new(ServerConfig { cookies: false, ..config() });
    let reply_1 = net.raw(addr(50001), request_1(1400));
    assert_eq!(o::parse_reply_1(&reply_1[0]).unwrap().cookie, None);
    net.join(50000);
    net.run(6);
    assert_eq!(net.events, [ServerEvent::Connected(addr(50000))]);
}
