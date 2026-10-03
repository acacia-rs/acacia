//! Region latencies for a realm join's `pingRegions`. PlayFab QoS beacons (UDP 3075) echo a datagram
//! that starts with `0xFFFF` back with `0x0000` in its place. Vanilla's shape: docs/research/qos-lan-capture-2026-10-03.md.

use std::time::{Duration, Instant};

use acacia_auth::{PingRegion, QosBeacon};

use crate::socks5::Socks5Proxy;
use crate::transport::{resolve, Transport};
use crate::ConnectError;

const PORT: u16 = 3075;
const PAYLOAD_LEN: usize = 10;
const SEND_SPACING: Duration = Duration::from_millis(15);
/// TODO: vanilla's give-up time is unknown; its slowest echo (australiaSoutheast) took 0.29 s.
const TIMEOUT: Duration = Duration::from_secs(2);

/// Pings every beacon once (through `proxy` when given, so latencies match the bot's egress) and
/// returns each region's round trip in `beacons` order. Regions that never answer are left out.
pub async fn measure_ping_regions(beacons: &[QosBeacon], proxy: Option<&Socks5Proxy>) -> Result<Vec<PingRegion>, ConnectError> {
    let mut targets = Vec::with_capacity(beacons.len());
    for (i, b) in beacons.iter().enumerate() {
        match resolve(&format!("{}:{PORT}", b.host)).await {
            Ok(addr) => targets.push((i, addr)),
            Err(e) => tracing::debug!(host = %b.host, "QoS beacon unresolved: {e}"),
        }
    }
    let Some(&(_, first)) = targets.first() else { return Ok(vec![]) };
    let transport = Transport::bind(first, proxy).await?;

    let mut sent_at: Vec<Option<Instant>> = vec![None; beacons.len()];
    let mut rtts: Vec<Option<Duration>> = vec![None; beacons.len()];
    let mut queue = targets.into_iter();
    let mut next_send = Instant::now();
    let mut deadline = None;
    let mut buf = [0u8; 256];
    loop {
        if deadline.is_none() && Instant::now() >= next_send {
            match queue.next() {
                Some((i, addr)) => {
                    sent_at[i] = Some(Instant::now());
                    if let Err(e) = transport.send_to(&ping_payload(i), addr).await {
                        tracing::trace!(%addr, "QoS send failed: {e}");
                    }
                    next_send = Instant::now() + SEND_SPACING;
                }
                None => deadline = Some(Instant::now() + TIMEOUT),
            }
        }
        let pending = sent_at.iter().zip(&rtts).any(|(s, r)| s.is_some() && r.is_none());
        if deadline.is_some_and(|d| !pending || Instant::now() >= d) {
            break;
        }
        let wake = deadline.unwrap_or(next_send);
        tokio::select! {
            r = transport.recv_from(&mut buf) => {
                let (range, source) = r?;
                let Some(i) = echo_index(&buf[range]).filter(|&i| i < beacons.len() && rtts[i].is_none()) else { continue };
                if let Some(at) = sent_at[i] {
                    tracing::trace!(%source, region = %beacons[i].region, "QoS echo");
                    rtts[i] = Some(at.elapsed());
                }
            }
            _ = tokio::time::sleep_until(wake.into()) => {}
        }
    }
    Ok(beacons
        .iter()
        .zip(rtts)
        .filter_map(|(b, rtt)| Some(PingRegion { latency_ms: rtt?.as_millis() as u32, region: b.region.clone() }))
        .collect())
}

/// `FFFF <id> 10 000000000000`, id counting regions from 1.
fn ping_payload(index: usize) -> [u8; PAYLOAD_LEN] {
    let id = u8::try_from(index + 1).expect("fewer than 255 QoS regions");
    [0xFF, 0xFF, id, 0x10, 0, 0, 0, 0, 0, 0]
}

/// The beacon index an echo answers, if `data` is one.
fn echo_index(data: &[u8]) -> Option<usize> {
    (data.len() == PAYLOAD_LEN && data[..2] == [0, 0] && data[2] > 0).then(|| usize::from(data[2]) - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echoes_match_their_ping() {
        let payload = ping_payload(0);
        assert_eq!(payload, [0xFF, 0xFF, 0x01, 0x10, 0, 0, 0, 0, 0, 0]);
        let mut echo = ping_payload(27);
        echo[..2].copy_from_slice(&[0, 0]);
        assert_eq!(echo_index(&echo), Some(27));
        assert_eq!(echo_index(&payload), None);
    }
}
