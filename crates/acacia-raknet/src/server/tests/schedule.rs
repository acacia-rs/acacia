use super::*;

#[test]
fn peers_with_output_take_turns() {
    let mut net = Net::new(config());
    net.join(50000);
    net.join(50001);
    net.run(6);
    for port in [50000, 50001] {
        assert!(net.server.send(addr(port), Bytes::from(vec![1u8; 20_000]), Reliability::ReliableOrdered));
    }
    let now = net.now;
    let order: Vec<SocketAddr> = std::iter::from_fn(|| net.server.poll_transmit(now)).map(|(to, _)| to).collect();
    assert_eq!(order.len(), 20, "one window of 10 for each");
    assert!(order.chunks(2).all(|pair| pair[0] != pair[1]), "{order:?}");
}

#[test]
fn a_silent_peer_times_out_and_leaves_no_timers() {
    let mut net = Net::new(config());
    net.join(50000);
    net.run(6);
    net.clients.clear();
    net.run(995);
    assert_eq!(net.events, [joined(50000)], "not yet 10 s of silence");
    net.run(5);
    assert_eq!(net.events.last(), Some(&ServerEvent::Disconnected(addr(50000), DisconnectReason::Timeout)));
    net.run(400);
    assert_eq!((net.server.poll_timeout(), net.server.schedule.timers()), (None, 0));
}

#[test]
fn a_busy_peer_keeps_a_handful_of_timers() {
    let mut net = Net::new(config());
    let client = net.join(50000);
    net.run(6);
    for _ in 0..200 {
        assert!(net.server.send(addr(50000), Bytes::from(vec![1u8; 3000]), Reliability::ReliableOrdered));
        assert!(net.clients[client].1.send(Bytes::from_static(b"\xfehi"), Reliability::ReliableOrdered));
        net.run(5);
        assert!(net.server.schedule.timers() <= 4, "{} timers", net.server.schedule.timers());
    }
    assert_eq!(net.events.len(), 201, "connected, then 200 messages");
    assert!(net.server.poll_timeout().is_some_and(|t| t > net.now));
}
