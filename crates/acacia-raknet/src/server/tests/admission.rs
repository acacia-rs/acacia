use super::*;

fn connected(events: Vec<Event>) -> bool {
    matches!(events[..], [Event::Connected { .. }])
}

#[test]
fn a_full_server_turns_request_2_away() {
    let mut net = Net::new(ServerConfig { max_peers: 1, ..config() });
    let first = net.join(50000);
    net.run(6);
    let second = net.join(50001);
    net.run(3);
    assert_eq!(net.client_events(second), [Event::Disconnected(DisconnectReason::ServerFull)]);

    let now = net.now;
    net.clients[first].1.close(now);
    net.run(1);
    let third = net.join(50002);
    net.run(6);
    assert!(connected(net.client_events(third)));
}

#[test]
fn banned_addresses_are_refused_until_unbanned() {
    let mut net = Net::new(config());
    let ip = addr(0).ip();
    net.server.ban(ip);
    let refused = net.join(50000);
    net.run(2);
    assert_eq!(net.client_events(refused), [Event::Disconnected(DisconnectReason::Banned)]);
    let cookie = Some(net.cookie(addr(50001)));
    assert_eq!(net.raw(addr(50001), request_2(cookie, 1400, 3))[0][0], o::ID_CONNECTION_BANNED);

    net.server.unban(ip);
    let welcome = net.join(50002);
    net.run(6);
    assert!(connected(net.client_events(welcome)));
}

#[test]
fn a_handshake_that_never_finishes_is_dropped() {
    let mut net = Net::new(ServerConfig { handshake_timeout: Duration::from_secs(1), ..config() });
    let from = addr(50000);
    let cookie = Some(net.cookie(from));
    assert_eq!(net.raw(from, request_2(cookie, 1400, 2)).len(), 1);
    assert_eq!(net.server.poll_timeout(), Some(net.now + Duration::from_secs(1)));
    net.run(99);
    assert_eq!(net.server.peers.len(), 1);
    net.run(1);
    assert!(net.server.peers.is_empty() && net.events.is_empty());
}
