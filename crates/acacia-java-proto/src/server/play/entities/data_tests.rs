use super::Optional;
use super::data::*;
use super::data_types::*;
use super::particle::*;
use super::profile::*;
use crate::actions::{BlockPos, Direction};
use crate::item::ItemStack;
use crate::registry::PARTICLE_TYPE;
use crate::server::round_trip;
use crate::text::Component;
use crate::types::{GlobalPos, Vec3};
use crate::{VarInt, Wire};

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

#[test]
fn entity_data_ends_with_ff_and_names_the_serializer() {
    let data = EntityData(vec![
        DataItem::new(0, DataValue::Byte(0x20)),
        DataItem::new(9, DataValue::Float(1.5)),
        DataItem::new(6, DataValue::Pose(Pose::Crouching)),
        DataItem::new(2, DataValue::OptionalComponent(None)),
    ]);
    let bytes = round_trip(data);
    assert_eq!(bytes, cat(&[&[0, 0, 0x20], &[9, 3], &1.5f32.to_be_bytes(), &[6, 20, 5], &[2, 6, 0], &[0xFF]]));
    assert_eq!(round_trip(SetEntityData { entity: VarInt(3), items: EntityData::default() }), [3, 0xFF]);
}

fn one_of_each() -> Vec<DataValue> {
    let rot = Rotations { x: 1.0, y: 2.0, z: 3.0 };
    let mut values = vec![
        DataValue::Byte(1),
        DataValue::Int(-5),
        DataValue::Long(1 << 40),
        DataValue::Float(0.25),
        DataValue::String("s".into()),
        DataValue::Component(Component::text("c")),
        DataValue::OptionalComponent(Some(Component::text("n"))),
        DataValue::ItemStack(Some(ItemStack::new(7, 3))),
        DataValue::Boolean(true),
        DataValue::Rotations(rot.clone()),
        DataValue::BlockPos(BlockPos([1, -2, 3])),
        DataValue::OptionalBlockPos(Some(BlockPos([1, -2, 3]))),
        DataValue::Direction(Direction::East),
        DataValue::OptionalLivingEntityReference(Some(9)),
        DataValue::BlockState(1234),
        DataValue::OptionalBlockState(Some(1234)),
        DataValue::Particle(Particle::of("heart", ParticleOptions::None).unwrap()),
        DataValue::Particles(vec![Particle::of("entity_effect", ParticleOptions::Int(-1)).unwrap()]),
        DataValue::VillagerData(VillagerData { villager_type: VarInt(1), profession: VarInt(2), level: VarInt(3) }),
        DataValue::OptionalUnsignedInt(Some(77)),
        DataValue::Pose(Pose::Inhaling),
    ];
    values.extend(VARIANTS.iter().map(|&kind| DataValue::Variant(kind, 4)));
    values.extend([
        DataValue::OptionalGlobalPos(Some(GlobalPos { dimension: "minecraft:overworld".into(), pos: BlockPos([1, 2, 3]) })),
        DataValue::PaintingVariant(PaintingHolder::Registered(5)),
        DataValue::SnifferState(1),
        DataValue::ArmadilloState(2),
        DataValue::CopperGolemState(3),
        DataValue::WeatheringCopperState(2),
        DataValue::Vector3(rot),
        DataValue::Quaternion([0.0, 0.0, 0.0, 1.0]),
        DataValue::ResolvableProfile(ResolvableProfile::default()),
        DataValue::HumanoidArm(1),
        DataValue::DyeColor(15),
    ]);
    values
}

#[test]
fn every_serializer_has_its_registration_id_and_round_trips() {
    let values = one_of_each();
    assert_eq!(values.len(), 44);
    for (id, value) in values.into_iter().enumerate() {
        assert_eq!(usize::from(value.serializer_id()), id, "{value:?}");
        round_trip(EntityData(vec![DataItem::new(1, value)]));
    }
}

#[test]
fn nullable_values_use_zero_or_a_flag() {
    let bytes = |v: DataValue| {
        let mut out = Vec::new();
        EntityData(vec![DataItem::new(1, v)]).write(&mut out);
        out[2..out.len() - 1].to_vec()
    };
    assert_eq!(bytes(DataValue::OptionalUnsignedInt(None)), [0]);
    assert_eq!(bytes(DataValue::OptionalUnsignedInt(Some(0))), [1]);
    assert_eq!(bytes(DataValue::OptionalBlockState(None)), [0]);
    assert_eq!(bytes(DataValue::ItemStack(None)), [0]);
    assert_eq!(bytes(DataValue::OptionalBlockPos(None)), [0]);
    assert_eq!(bytes(DataValue::Direction(Direction::Up)), [1]);
    assert_eq!(bytes(DataValue::Long(300)), [0xAC, 0x02]);
    assert_eq!(bytes(DataValue::PaintingVariant(PaintingHolder::Registered(5))), [6]);
}

#[test]
fn painting_variants_can_be_inline() {
    let inline = PaintingHolder::Inline { width: 2, height: 1, asset_id: "a:b".into(), title: Optional(Some(Component::text("t"))), author: Optional(None) };
    let mut out = Vec::new();
    EntityData(vec![DataItem::new(9, DataValue::PaintingVariant(inline))]).write(&mut out);
    assert_eq!(out[..6], [9, 34, 0, 2, 1, 3]);
    round_trip(EntityData::read(&mut &out[..]).unwrap());
}

#[test]
fn unknown_serializers_and_particles_are_errors() {
    assert!(EntityData::read(&mut &[0, 44, 0, 0xFF][..]).is_err());
    assert!(EntityData::read(&mut &[0, 3][..]).is_err());
    assert!(Particle::read(&mut &[0xFF, 0x7F][..]).is_err());
}

fn particle_bytes(name: &str, options: ParticleOptions) -> Vec<u8> {
    let id = PARTICLE_TYPE.require(name).unwrap();
    let bytes = round_trip(Particle::of(name, options).unwrap());
    let mut head = Vec::new();
    crate::write_varint(id, &mut head);
    assert_eq!(bytes[..head.len()], head[..], "{name}");
    bytes[head.len()..].to_vec()
}

#[test]
fn particle_options_follow_the_option_classes() {
    use ParticleOptions as O;
    assert_eq!(particle_bytes("heart", O::None), Vec::<u8>::new());
    assert_eq!(particle_bytes("block", O::BlockState(130)), [0x82, 0x01]);
    assert_eq!(particle_bytes("dragon_breath", O::Float(1.0)), 1.0f32.to_be_bytes());
    assert_eq!(particle_bytes("flash", O::Int(-1)), [0xFF; 4]);
    assert_eq!(particle_bytes("effect", O::IntFloat(-1, 1.0)), cat(&[&[0xFF; 4], &1.0f32.to_be_bytes()]));
    assert_eq!(particle_bytes("dust", O::IntFloat(0xFF0000, 1.0)).len(), 8);
    assert_eq!(particle_bytes("dust_color_transition", O::DustTransition { from: 1, to: 2, scale: 1.0 }).len(), 12);
    assert_eq!(particle_bytes("geyser", O::Int(3)), [0, 0, 0, 3]);
    assert_eq!(particle_bytes("geyser_base", O::IntFloat(3, 0.5)).len(), 8);
    assert_eq!(particle_bytes("sculk_charge", O::Float(0.5)), 0.5f32.to_be_bytes());
    assert_eq!(particle_bytes("shriek", O::Shriek(5)), [5]);
    assert_eq!(particle_bytes("trail", O::Trail { target: Vec3::default(), color: -1, duration: 20 }).len(), 24 + 4 + 1);
}

#[test]
fn item_particles_write_item_before_count() {
    let item = ItemTemplate { item: 7, count: 3, ..ItemTemplate::default() };
    assert_eq!(particle_bytes("item", ParticleOptions::Item(item)), [7, 3, 0, 0]);
}

#[test]
fn vibration_names_a_position_source_then_the_arrival_time() {
    let block = ParticleOptions::Vibration { source: PositionSource::Block(BlockPos([0, 0, 0])), arrival_ticks: 40 };
    assert_eq!(particle_bytes("vibration", block), [0, 0, 0, 0, 0, 0, 0, 0, 0, 40]);
    let entity = ParticleOptions::Vibration { source: PositionSource::Entity { id: 5, y_offset: 1.0 }, arrival_ticks: 2 };
    assert_eq!(particle_bytes("vibration", entity), cat(&[&[1, 5], &1.0f32.to_be_bytes(), &[2]]));
}

#[test]
fn resolvable_profiles_are_either_full_or_partial_plus_a_skin_patch() {
    let full = ResolvableProfile { source: ProfileSource::Full(crate::server::login::GameProfile { uuid: 1, name: "A".into(), properties: vec![] }), ..Default::default() };
    let bytes = round_trip(full);
    assert_eq!(bytes[0], 1);
    assert_eq!(bytes.len(), 1 + 16 + 2 + 1 + 4);
    let partial = ResolvableProfile {
        source: ProfileSource::Partial(PartialProfile { name: Optional(Some("N".into())), ..Default::default() }),
        skin: SkinPatch { slim: Optional(Some(true)), ..Default::default() },
    };
    assert_eq!(round_trip(partial), [0, 1, 1, b'N', 0, 0, 0, 0, 0, 1, 1]);
}
