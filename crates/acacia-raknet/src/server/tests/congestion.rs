use super::*;

/// A connected client on port 50000 with its events drained, and the server's view of it.
fn connected() -> (Net, usize) {
    let mut net = Net::new(config());
    let client = net.join(50000);
    net.run(6);
    net.client_events(client);
    (net, client)
}

fn stats(net: &Net) -> PeerStats {
    net.server.stats(addr(50000)).unwrap()
}

#[test]
fn a_large_message_leaves_under_the_window_and_arrives() {
    let (mut net, client) = connected();
    let big = Bytes::from(vec![9u8; 2_000_000]);
    assert!(net.server.send(addr(50000), big.clone(), Reliability::ReliableOrdered));
    assert_eq!((stats(&net).queued_bytes, stats(&net).window), (2_000_000, 10));

    net.run(1);
    let after_one = stats(&net);
    assert_eq!(after_one.in_flight, 10, "one window, not two megabytes at once");
    assert!(after_one.queued_bytes > 1_900_000);

    net.run(12);
    assert_eq!(net.client_events(client), [Event::Message(big)]);
    let done = stats(&net);
    assert_eq!((done.queued_bytes, done.in_flight, done.resent), (0, 0, 0));
    assert!(done.window > 500, "slow start opened the window: {}", done.window);
}

#[test]
fn a_lossy_link_still_delivers_and_shrinks_the_window() {
    let (mut net, client) = connected();
    net.lose_every = Some(7);
    let big = Bytes::from(vec![9u8; 300_000]);
    assert!(net.server.send(addr(50000), big.clone(), Reliability::ReliableOrdered));
    net.run(400);
    assert_eq!(net.client_events(client), [Event::Message(big)]);
    let done = stats(&net);
    assert!(done.resent > 20 && done.window < 40, "{done:?}");
    assert_eq!((done.queued_bytes, done.in_flight), (0, 0));
    assert_eq!(net.events, [joined(50000)], "the client stayed connected");
}
