use acacia_java_proto::chat::{LAST_SEEN, LastSeenUpdate, MessageSignature, PackedSignature, SIGNATURE_BYTES};

use super::*;
use crate::last_seen::checksum;

fn sig(byte: u8) -> MessageSignature {
    MessageSignature([byte; SIGNATURE_BYTES])
}

#[test]
fn checksums_match_java_hash_codes() {
    let up = MessageSignature(std::array::from_fn(|i| i as u8));
    let down = MessageSignature(std::array::from_fn(|i| 255 - i as u8));
    // Java: (byte) (31 * (31 + Arrays.hashCode(up)) + Arrays.hashCode(down)), etc.
    assert_eq!(checksum(&[up.clone(), down]), 225);
    assert_eq!(checksum(&[up]), 160);
    assert_eq!(checksum(&[]), 1);
}

#[test]
fn tracker_acknowledges_the_window_oldest_first_and_clears_the_offset() {
    let mut tracker = Tracker::default();
    assert!(tracker.add_pending(sig(1), true));
    assert!(!tracker.add_pending(sig(1), true), "a repeat of the last message is not tracked");
    assert!(tracker.add_pending(sig(2), false));
    assert!(tracker.add_pending(sig(3), true));
    let (seen, update) = tracker.update();
    assert_eq!(seen, [sig(1), sig(3)]);
    assert_eq!(update, LastSeenUpdate { offset: 3, acknowledged: 1 << 17 | 1 << 19, checksum: checksum(&seen) });
    let (_, again) = tracker.update();
    assert_eq!((again.offset, again.acknowledged), (0, update.acknowledged));
}

#[test]
fn tracker_window_drops_messages_older_than_twenty() {
    let mut tracker = Tracker::default();
    for i in 0..=LAST_SEEN as u8 {
        tracker.add_pending(sig(i), true);
    }
    tracker.ignore_pending(&sig(LAST_SEEN as u8));
    let (seen, update) = tracker.update();
    assert_eq!(seen.first(), Some(&sig(1)));
    assert_eq!(seen.len(), LAST_SEEN - 1);
    assert_eq!(update.acknowledged, (1 << (LAST_SEEN - 1)) - 1);
    assert_eq!(update.offset, LAST_SEEN as i32 + 1);
}

#[test]
fn cache_puts_newest_first_and_keeps_displaced_entries() {
    let mut cache = SignatureCache::default();
    cache.push(Vec::new(), Some(sig(1)));
    cache.push(vec![sig(1)], Some(sig(2)));
    assert_eq!(cache.unpack(&PackedSignature::Cached(0)), Some(sig(2)));
    assert_eq!(cache.unpack(&PackedSignature::Cached(1)), Some(sig(1)));
    cache.push(Vec::new(), Some(sig(3)));
    let order: Vec<_> = (0..4).map(|i| cache.unpack(&PackedSignature::Cached(i))).collect();
    assert_eq!(order, [Some(sig(3)), Some(sig(2)), Some(sig(1)), None]);
}

#[test]
fn signed_command_encodes_like_its_codec() {
    let packet = SignedChatCommand {
        command: "msg Bob hi".into(),
        timestamp_millis: 1,
        salt: 2,
        arguments: vec![ArgumentSignature { name: "message".into(), signature: sig(9) }],
        last_seen: LastSeenUpdate { offset: 1, acknowledged: 1 << 19, checksum: 7 },
    };
    let mut out = Vec::new();
    packet.write(&mut out);
    let mut expected = vec![10];
    expected.extend(b"msg Bob hi");
    expected.extend(1i64.to_be_bytes());
    expected.extend(2i64.to_be_bytes());
    expected.extend([1, 7]);
    expected.extend(b"message");
    expected.extend([9; SIGNATURE_BYTES]);
    expected.extend([1, 0, 0, 0x08, 7]);
    assert_eq!(out, expected);
}
