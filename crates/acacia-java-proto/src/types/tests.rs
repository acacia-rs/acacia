use super::*;

fn round_trip<T: Wire + PartialEq + std::fmt::Debug>(value: T) -> Vec<u8> {
    let mut out = Vec::new();
    value.write(&mut out);
    let mut input = &out[..];
    assert_eq!(T::read(&mut input), Ok(value));
    assert!(input.is_empty());
    out
}

#[test]
fn game_modes_round_trip_and_reject_unknown_ids() {
    for mode in [GameMode::Survival, GameMode::Creative, GameMode::Adventure, GameMode::Spectator] {
        assert_eq!(round_trip(mode), [mode.id()]);
        assert_eq!(round_trip(OptionalGameMode(Some(mode))), [mode.id() + 1]);
    }
    assert_eq!(round_trip(OptionalGameMode(None)), [0]);
    assert_eq!(GameMode::read(&mut &[4][..]), Err(Error::Invalid("game mode")));
    assert_eq!(GameMode::Spectator.name(), "spectator");
}

#[test]
fn colour_writes_argb_and_reads_back_rgb() {
    assert_eq!(round_trip(Rgb::new(1, 2, 3)), [0xFF, 1, 2, 3]);
    assert_eq!(Rgb::read(&mut &[0x12, 0xAB, 0xCD, 0xEF][..]), Ok(Rgb(0xABCDEF)));
}

#[test]
fn positions_pack_like_vanilla() {
    assert_eq!(round_trip(ChunkPos { x: -1, z: 2 }), [0, 0, 0, 2, 0xFF, 0xFF, 0xFF, 0xFF]);
    // SectionPos.asLong(1, 2, 3) = 1 << 42 | 2 | 3 << 20
    assert_eq!(round_trip(SectionPos { x: 1, y: 2, z: 3 }), (1i64 << 42 | 2 | 3 << 20).to_be_bytes());
    round_trip(SectionPos { x: -33_000, y: -4, z: 33_000 });
    round_trip(GlobalPos { dimension: "minecraft:overworld".into(), pos: BlockPos([-5, 70, 12]) });
    round_trip(Vec3 { x: 1.5, y: -2.0, z: 3.25 });
}

#[test]
fn section_relative_positions_and_filterables() {
    assert_eq!(round_trip(SectionBlockPos::of(BlockPos([-1, 17, 18]))), [0x0F, 0x21]);
    assert_eq!(SectionBlockPos { x: 1, y: 2, z: 3 }.packed(), 0x132);
    assert_eq!(round_trip(Filterable { raw: "a".to_string(), filtered: None }), [1, b'a', 0]);
    assert_eq!(round_trip(Filterable { raw: "a".to_string(), filtered: Some("b".to_string()) }), [1, b'a', 1, 1, b'b']);
}

#[test]
fn id_enums_use_their_vanilla_ids() {
    assert_eq!(round_trip(HeightmapKind::MotionBlockingNoLeaves), [5]);
    assert_eq!(round_trip(HeightmapKind::OceanFloor), [3]);
    assert_eq!(round_trip(AttributeModifierOperation::AddMultipliedTotal), [2]);
    assert_eq!(round_trip(ObjectiveRenderType::Hearts), [1]);
    assert_eq!(HeightmapKind::read(&mut &[6][..]), Err(Error::Invalid("heightmap type")));
}
