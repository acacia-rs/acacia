//! The proxy's answer to RakNet pings, so the game's server list shows it.

use std::net::SocketAddr;
use std::time::Duration;

use bytes::BytesMut;
use tokio::net::UdpSocket;
use tokio::sync::watch;

const INTERVAL: Duration = Duration::from_secs(3);

/// Mirrors the server's ping answer with our GUID and port. Until the server answers (BDS 1.26.52
/// ignores pings), a generic one: the game won't join a blank status.
pub fn watch_status(server: SocketAddr, guid: u64, port: u16) -> watch::Receiver<String> {
    let (protocol, version) = (acacia_proto::PROTOCOL_VERSION, acacia_proto::GAME_VERSION);
    let fallback = format!("MCPE;acacia-mitm;{protocol};{version};0;10;{guid};acacia-mitm;Survival;1;{port};{port};0;");
    let (tx, rx) = watch::channel(fallback);
    tokio::spawn(async move {
        let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await else { return };
        let mut buf = vec![0u8; 2048];
        while !tx.is_closed() {
            let mut ping = BytesMut::new();
            acacia_raknet::unconnected_ping(&mut ping, 0, guid);
            let _ = socket.send_to(&ping, server).await;
            if let Ok(Ok((n, _))) = tokio::time::timeout(INTERVAL, socket.recv_from(&mut buf)).await
                && let Ok(pong) = acacia_raknet::parse_pong(&buf[..n])
            {
                let mut fields: Vec<String> = pong.motd.split(';').map(str::to_owned).collect();
                for (i, value) in [(6, guid.to_string()), (10, port.to_string()), (11, port.to_string())] {
                    if let Some(f) = fields.get_mut(i) {
                        *f = value;
                    }
                }
                let motd = fields.join(";");
                tx.send_if_modified(|m| m != &motd && { *m = motd; true });
            }
            tokio::time::sleep(INTERVAL).await;
        }
    });
    rx
}
