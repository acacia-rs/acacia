use super::*;

#[test]
fn client_connects_and_messages_flow_both_ways() {
    let mut net = Net::new(config());
    let client = net.join(50000);
    net.run(6);
    assert_eq!(net.events, [joined(50000)]);
    assert!(matches!(net.client_events(client)[..], [Event::Connected { mtu: 1492 }]));
    assert_eq!(net.server.peers().collect::<Vec<_>>(), [addr(50000)]);
    let stats = net.server.stats(addr(50000)).unwrap();
    assert_eq!((stats.guid, stats.mtu, stats.rtt), (50000, 1492, Some(TICK)), "the accepted reply was ACKed a tick later");
    assert_eq!(net.server.stats(addr(50001)), None);

    let (big, hello) = (Bytes::from(vec![7u8; 5000]), Bytes::from_static(b"\xfehello"));
    assert!(net.clients[client].1.send(big.clone(), Reliability::ReliableOrdered));
    assert!(net.server.send(addr(50000), hello.clone(), Reliability::ReliableOrdered));
    net.run(3);
    assert_eq!(net.events[1..], [ServerEvent::Message(addr(50000), big)]);
    assert_eq!(net.client_events(client), [Event::Message(hello)]);

    let now = net.now;
    net.clients[client].1.close(now);
    net.run(1);
    assert_eq!(net.events.last(), Some(&ServerEvent::Disconnected(addr(50000), DisconnectReason::ClientClosed)));
}

#[test]
fn client_survives_a_tiny_mtu_in_the_replies() {
    let now = Instant::now();
    let mut client = Client::new(Config::new(2), addr(SERVER_PORT), now);
    let (mut reply_1, mut reply_2) = (BytesMut::new(), BytesMut::new());
    o::reply_1(&mut reply_1, 1, None, 0);
    o::reply_2(&mut reply_2, 1, addr(50000), 0);
    client.handle_datagram(now, reply_1.freeze());
    client.handle_datagram(now, reply_2.freeze());
    let sent: Vec<Bytes> = std::iter::from_fn(|| client.poll_transmit(now)).collect();
    assert!(sent.len() >= 3, "requests 1 and 2, then the connection request");
}
