use super::*;

#[test]
fn reads_own_actor_flags() {
    // BDS 1.26 to a vanilla client (runtime id 1): flags only, not sprinting.
    let body = Bytes::from_static(&[1, 1, 0, 7, 7, 0x80, 0x80, 0xc0, 0x80, 0x80, 0x82, 0x80, 3, 0, 0, 0]);
    let packet = RawPacket { id: SetEntityData::ID, sender_subclient: 0, target_subclient: 0, body };
    assert!(about(&packet, 1) && !about(&packet, 2));
    let flags = actor_flags(&packet.decode().unwrap()).unwrap();
    assert_eq!(flags.0 & MetadataFlags1::SPRINTING.0, 0);
}

#[test]
fn events_round_trip() {
    let path = std::env::temp_dir().join(format!("btrc-{}.bin", std::process::id()));
    let events = vec![
        Event::Mark("drill".into()),
        Event::Start { feet: [1.5, -60.0, 2.5], yaw: -90.0, pitch: 0.0 },
        Event::Packet(RawPacket { id: 7, sender_subclient: 0, target_subclient: 0, body: Bytes::from_static(b"abc") }),
        Event::Input(Bytes::from_static(b"xyz")),
        Event::Ride(Ride::Boat { feet: [1.0, 2.0, 3.0], yaw: 90.0 }),
        Event::Ride(Ride::Horse { feet: [1.0, 2.0, 3.0], yaw: 4.0, speed: 0.2, jump_strength: 0.7 }),
        Event::Ride(Ride::Leave { feet: [5.0, 6.0, 7.0] }),
        Event::Ride(Ride::Seat),
        Event::GlideBoost,
        Event::Equipment(Equipment { elytra: true, elytra_uses: Some(3), ..Equipment::default() }),
        Event::Equipment(Equipment { elytra: true, ..Equipment::default() }),
        Event::Solids(vec![Aabb { min: [0.0, 1.0, 2.0], max: [3.0, 4.0, 5.0] }]),
        Event::Riptide(3),
    ];
    let mut rec = Recorder::create(&path).unwrap();
    events.iter().for_each(|e| rec.write(e));
    drop(rec);
    assert_eq!(read(&path).unwrap(), events);
    std::fs::remove_file(path).unwrap();
}
