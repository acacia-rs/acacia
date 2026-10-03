use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};

use super::*;

/// Minimal SOCKS5 proxy: user/pass auth, one UDP association relaying to the requested target.
async fn mock_proxy(require_auth: bool) -> Socks5Proxy {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut tcp, _) = listener.accept().await.unwrap();
        let mut greet = [0u8; 2];
        tcp.read_exact(&mut greet).await.unwrap();
        let mut methods = vec![0u8; greet[1].into()];
        tcp.read_exact(&mut methods).await.unwrap();
        if require_auth {
            tcp.write_all(&[5, 2]).await.unwrap();
            let mut ver_len = [0u8; 2];
            tcp.read_exact(&mut ver_len).await.unwrap();
            let mut user = vec![0u8; ver_len[1].into()];
            tcp.read_exact(&mut user).await.unwrap();
            let mut pass = vec![0u8; tcp.read_u8().await.unwrap().into()];
            tcp.read_exact(&mut pass).await.unwrap();
            let ok = user == b"bot" && pass == b"secret";
            tcp.write_all(&[1, if ok { 0 } else { 1 }]).await.unwrap();
            if !ok {
                return;
            }
        } else {
            tcp.write_all(&[5, 0]).await.unwrap();
        }
        let mut req = [0u8; 10];
        tcp.read_exact(&mut req).await.unwrap();
        let relay = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mut reply = vec![5, 0, 0];
        put_addr(&mut reply, (std::net::Ipv4Addr::UNSPECIFIED, relay.local_addr().unwrap().port()).into());
        tcp.write_all(&reply).await.unwrap();

        let mut buf = [0u8; 2048];
        let mut client = None;
        loop {
            let (n, from) = relay.recv_from(&mut buf).await.unwrap();
            if let Some(c) = client.filter(|&c| c != from) {
                // From the target: wrap and forward to the client.
                let mut out = vec![0, 0, 0];
                put_addr(&mut out, from);
                out.extend_from_slice(&buf[..n]);
                relay.send_to(&out, c).await.unwrap();
            } else {
                client = Some(from);
                let target = SocketAddr::new(std::net::Ipv4Addr::new(buf[4], buf[5], buf[6], buf[7]).into(), u16::from_be_bytes([buf[8], buf[9]]));
                relay.send_to(&buf[10..n], target).await.unwrap();
            }
        }
    });
    Socks5Proxy::new("127.0.0.1", addr.port())
}

async fn echo_server() -> SocketAddr {
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = sock.local_addr().unwrap();
    tokio::spawn(async move {
        let mut buf = [0u8; 2048];
        loop {
            let (n, from) = sock.recv_from(&mut buf).await.unwrap();
            buf[..n].reverse();
            sock.send_to(&buf[..n], from).await.unwrap();
        }
    });
    addr
}

#[tokio::test]
async fn relays_datagrams_both_ways() {
    for auth in [false, true] {
        let target = echo_server().await;
        let mut proxy = mock_proxy(auth).await;
        if auth {
            proxy = proxy.login("bot", "secret");
        }
        let udp = Socks5Udp::associate(&proxy, target).await.unwrap();
        udp.send(b"hello").await.unwrap();
        let mut buf = [0u8; 2048];
        let (range, source) = udp.recv_from(&mut buf).await.unwrap();
        assert_eq!((&buf[range], source), (&b"olleh"[..], target));
    }
}

#[tokio::test]
async fn connect_tunnels_tcp() {
    let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = echo.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut s, _) = echo.accept().await.unwrap();
        let mut buf = [0u8; 4];
        s.read_exact(&mut buf).await.unwrap();
        s.write_all(&buf).await.unwrap();
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy = Socks5Proxy::new("127.0.0.1", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let (mut tcp, _) = listener.accept().await.unwrap();
        let mut greet = [0u8; 3];
        tcp.read_exact(&mut greet).await.unwrap();
        tcp.write_all(&[5, 0]).await.unwrap();
        let mut req = [0u8; 10];
        tcp.read_exact(&mut req).await.unwrap();
        assert_eq!(req[1], 1, "CONNECT");
        let mut upstream = tokio::net::TcpStream::connect(target).await.unwrap();
        let mut reply = vec![5, 0, 0];
        put_addr(&mut reply, target);
        tcp.write_all(&reply).await.unwrap();
        let _ = tokio::io::copy_bidirectional(&mut tcp, &mut upstream).await;
    });
    let mut stream = connect_tcp(&proxy, target).await.unwrap();
    stream.write_all(b"ping").await.unwrap();
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"ping");
}

#[tokio::test]
async fn wrong_credentials_are_rejected() {
    let proxy = mock_proxy(true).await.login("bot", "wrong");
    let err = Socks5Udp::associate(&proxy, "127.0.0.1:1".parse().unwrap()).await.err().unwrap();
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
}
