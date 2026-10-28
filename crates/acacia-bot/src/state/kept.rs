use std::sync::Mutex;

use acacia_client::proto::RawPacket;
use bytes::Bytes;

/// `packet` with a body of its own, shared with the last identical one kept. A received body slices the
/// whole decompressed batch (megabytes at join), and a swarm's bots all receive the same join packets.
pub(crate) fn keep(packet: &RawPacket) -> RawPacket {
    static LAST: Mutex<Vec<(u32, Bytes)>> = Mutex::new(Vec::new());
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let body = match last.iter_mut().find(|(id, _)| *id == packet.id) {
        Some((_, body)) if *body == packet.body => body.clone(),
        Some((_, body)) => {
            *body = Bytes::copy_from_slice(&packet.body);
            body.clone()
        }
        None => {
            let body = Bytes::copy_from_slice(&packet.body);
            last.push((packet.id, body.clone()));
            body
        }
    };
    RawPacket { id: packet.id, sender_subclient: packet.sender_subclient, target_subclient: packet.target_subclient, body }
}
