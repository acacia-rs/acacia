use super::*;
use crate::write_varint;

const DIAMOND_PICKAXE: i32 = 1052;
const TEXT: &[u8] = &[8, 0, 2, b'h', b'i'];
const COMPOUND: &[u8] = &[10, 8, 0, 1, b'k', 0, 1, b'v', 0];
const F32: &[u8] = &[0x3f, 0x80, 0, 0];
const F64: &[u8] = &[0x3f, 0xf0, 0, 0, 0, 0, 0, 0];
const SOUND_REF: &[u8] = &[5];
const HOLDER_SET_IDS: &[u8] = &[3, 7, 9];
const TRAILER: u8 = 0xee;

fn v(n: i32) -> Vec<u8> {
    let mut out = Vec::new();
    write_varint(n, &mut out);
    out
}

fn s(text: &str) -> Vec<u8> {
    let mut out = v(text.len() as i32);
    out.extend_from_slice(text.as_bytes());
    out
}

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

fn sound_direct() -> Vec<u8> {
    cat(&[&[0], &s("a:b"), &[1], F32])
}

fn holder_set_tag() -> Vec<u8> {
    cat(&[&[0], &s("minecraft:logs")])
}

/// `ItemStackTemplate`: item 5, count 1, damage 10.
fn template() -> Vec<u8> {
    cat(&[&v(5), &v(1), &[1, 0, 3, 10]])
}

/// A `MobEffectInstance` with one hidden effect behind it.
fn mob_effect() -> Vec<u8> {
    cat(&[&v(2), &v(1), &v(200), &[0, 1, 1], &[1], &v(0), &v(100), &[0, 0, 0], &[0]])
}

fn texts(n: usize) -> Vec<u8> {
    TEXT.repeat(n)
}

fn block_predicate() -> Vec<u8> {
    let properties = cat(&[&[2], &s("facing"), &[1], &s("north"), &s("age"), &[0], &[1], &s("1"), &[0]]);
    let exact = cat(&[&[1], &v(DAMAGE), &v(4)]);
    let partial = cat(&[&[1], &[1], &v(0), COMPOUND]);
    cat(&[&[1], HOLDER_SET_IDS, &[1], &properties, &[1], COMPOUND, &exact, &partial])
}

/// A sample value for every component the server can send, built from the 26.3 stream codecs.
fn samples() -> Vec<(i32, Vec<u8>)> {
    let consume_effects = cat(&[
        &[5],
        &v(0),
        &[1],
        &mob_effect(),
        F32,
        &v(1),
        &holder_set_tag(),
        &v(2),
        &v(3),
        F32,
        &[1],
        &v(4),
        SOUND_REF,
    ]);
    let attribute = |display: &[u8]| cat(&[&v(5), &s("minecraft:base"), F64, &v(0), &v(1), display]);
    let explosion = cat(&[&v(1), &[1], &[0, 0, 0, 1], &[0], &[1, 0]]);
    let mut samples = vec![
        (1, v(64)),
        (2, v(1561)),
        (3, v(10)),
        (4, vec![]),
        (5, cat(&[&[1, 0], F32])),
        (6, TEXT.to_vec()),
        (7, F32.to_vec()),
        (8, v(3)),
        (9, cat(&[&[10], &[8, 0, 4], b"text", &[0, 1, b'x'], &[0]])),
        (10, s("minecraft:stick")),
        (11, cat(&[&[2], TEXT, TEXT])),
        (12, vec![1]),
        (13, vec![1, 7, 5]),
        (14, cat(&[&[1], &block_predicate()])),
        (15, vec![0]),
        (16, cat(&[&[2], &attribute(&cat(&[&v(2), TEXT])), &attribute(&v(0))])),
        (17, cat(&[&[1], F32, &[2, 1, 0], &[1], &s("x"), &[1], &[0, 0, 0, 7]])),
        (18, vec![1, 2, 3, 6]),
        (19, v(2)),
        (20, vec![]),
        (21, vec![1]),
        (23, cat(&[&v(4), F32, &[0]])),
        (24, cat(&[F32, &v(1), &sound_direct(), &[1], &consume_effects])),
        (25, template()),
        (26, cat(&[F32, &[1], &s("a:b")])),
        (27, holder_set_tag()),
        (28, cat(&[&[1], HOLDER_SET_IDS, &[1], F32, &[1, 1], F32, &v(1), &[1]])),
        (29, cat(&[&v(1), F32])),
        (30, F32.repeat(6)),
        (31, v(15)),
        (32, cat(&[&v(4), SOUND_REF, &[1], &s("minecraft:diamond"), &[0], &[1], HOLDER_SET_IDS, &[1, 1, 1, 0, 0], &sound_direct()])),
        (33, HOLDER_SET_IDS.to_vec()),
        (34, vec![]),
        (35, s("a:b")),
        (36, cat(&[&[1], &v(2)])),
        (37, cat(&[F32, F32, &[1], F32, &[1], &holder_set_tag(), F32, F32, &F32.repeat(3), &[0], &[1], SOUND_REF, &[0]])),
        (38, cat(&[&[1, 0], &[1], SOUND_REF, &[0]])),
        (39, cat(&[&v(10), &v(5), &[1], &v(1), F32, F32, &[0, 0], F32, F32, &[0], &[1], &sound_direct()])),
        (40, cat(&[&v(1), &v(6)])),
        (41, cat(&[&v(2), &v(6)])),
        (42, v(3)),
        (43, v(0)),
        (44, v(4)),
        (45, vec![0]),
        (46, v(14)),
        (47, vec![0, 0x11, 0x22, 0x33]),
        (48, v(42)),
        (50, v(1)),
        (51, cat(&[&[1], &template()])),
        (52, cat(&[&[2], &template(), &template()])),
        (53, cat(&[&[1], &v(9), &[1], &[0, 0, 0, 1], &[1], &mob_effect(), &[1], &s("x")])),
        (54, F32.to_vec()),
        (55, cat(&[&[1], &v(3), &v(160)])),
        (56, cat(&[&[2], &s("p1"), &[0], &s("p2"), &[1], &s("f")])),
        (57, cat(&[&s("t"), &[0], &s("me"), &v(0), &[1], TEXT, &[1], TEXT, &[1]])),
        (58, cat(&[&v(0), &s("a:b"), TEXT, &v(0), &s("c:d"), TEXT, &[1]])),
        (58, cat(&[&v(3), &v(4)])),
        (60, cat(&[&v(12), COMPOUND])),
        (61, COMPOUND.to_vec()),
        (62, cat(&[&v(3), COMPOUND])),
        (63, cat(&[&[0], SOUND_REF, F32, F32, &v(1), TEXT])),
        (64, v(2)),
        (64, cat(&[&[0], &s("a:b"), TEXT])),
        (65, v(2)),
        (66, cat(&[&[0], &sound_direct(), TEXT, F32, &v(15)])),
        (67, holder_set_tag()),
        (69, cat(&[&[1], &s("minecraft:overworld"), &[0; 8], &[1]])),
        (70, explosion.clone()),
        (71, cat(&[&v(2), &[1], &explosion])),
        (72, cat(&[&[1], &[0; 16], &s("Notch"), &[1], &s("textures"), &s("v"), &[1], &s("sig"), &[1], &s("a:b"), &[0, 0], &[1, 1]])),
        (72, cat(&[&[0], &[1], &s("Notch"), &[0], &[0], &[0, 0, 0], &[0]])),
        (73, s("a:b")),
        (74, cat(&[&[2], &v(0), &s("a:b"), &s("k"), &v(3), &v(7), &v(1)])),
        (75, v(1)),
        (76, cat(&[&[1], &template(), &[0, 0, 1], &template()])),
        (77, cat(&[&[3, 1], &template(), &[0, 1], &template()])),
        (78, cat(&[&[1], &s("facing"), &s("north")])),
        (79, cat(&[&[1], &v(5), COMPOUND, &v(100), &v(600)])),
        (80, template()),
        (83, SOUND_REF.to_vec()),
        (83, sound_direct()),
        (84, cat(&[&[1], &[0, 0, 0, 1]])),
        (85, cat(&[&[0], &s("a:b"), &[1], F32])),
        (86, cat(&[&[1], &[0, 0, 0, 1], &[0], &s("c:d")])),
        (87, cat(&[HOLDER_SET_IDS, F32])),
        (109, cat(&[&[0], &v(2), &v(1), &s("a:b"), &[1], TEXT, &[0]])),
        (118, cat(&[&texts(4), &[1], &texts(4), &v(0), &[1]])),
        (119, cat(&[&texts(4), &[0], &v(15), &[0]])),
        (120, vec![]),
    ];
    samples.extend((88..=108).chain(110..=117).chain([121]).map(|id| (id, v(1))));
    samples
}

const NOT_SYNCED: [i32; 7] = [0, 22, 49, 59, 68, 81, 82];

fn stack_with(id: i32, value: &[u8]) -> Vec<u8> {
    cat(&[&[1], &v(1), &[1, 0], &v(id), value])
}

#[test]
fn every_synced_component_skips_exactly_its_bytes() {
    for (id, value) in samples() {
        let bytes = cat(&[&stack_with(id, &value), &[TRAILER]]);
        let mut input = &bytes[..];
        assert!(<Option<ItemStack>>::read(&mut input).is_ok(), "component {id}");
        assert_eq!(input, [TRAILER], "component {id} left bytes");
        // Same path on every prefix, so each one must run out.
        for cut in 0..bytes.len() - 1 {
            assert!(<Option<ItemStack>>::read(&mut &bytes[..cut]).is_err(), "component {id} cut at {cut}");
        }
    }
}

#[test]
fn samples_cover_every_component_type() {
    let mut ids: Vec<i32> = samples().into_iter().map(|(id, _)| id).chain(NOT_SYNCED).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids, (0..components::COUNT).collect::<Vec<_>>());
}

#[test]
fn unsynced_and_unknown_components_are_refused() {
    for id in NOT_SYNCED {
        let err = <Option<ItemStack>>::read(&mut &stack_with(id, &[])[..]).unwrap_err();
        assert!(matches!(err, Error::Invalid(name) if name.starts_with("synced component")), "{id}: {err}");
    }
    let unknown = <Option<ItemStack>>::read(&mut &stack_with(components::COUNT, &[])[..]);
    assert_eq!(unknown, Err(Error::Invalid("data component type")));
    let removed_unknown = cat(&[&[1, 1, 0, 1], &v(components::COUNT)]);
    assert_eq!(<Option<ItemStack>>::read(&mut &removed_unknown[..]), Err(Error::Invalid("data component type")));
}

#[test]
fn damaged_named_efficiency_pickaxe_is_byte_exact() {
    // Enchantment ids are the session's synced registry order; 8 stands in for efficiency.
    #[rustfmt::skip]
    let wire = [
        0x01, 0x9c, 0x08, 0x03, 0x00,
        0x03, 0x64,
        0x0d, 0x01, 0x08, 0x05,
        0x06, 0x08, 0x00, 0x05, b'M', b'i', b'n', b'e', b'r',
    ];
    let stack = <Option<ItemStack>>::read(&mut &wire[..]).unwrap().unwrap();
    assert_eq!((stack.item, stack.count), (DIAMOND_PICKAXE, 1));
    assert_eq!(stack.components.damage, Some(100));
    assert_eq!(stack.components.enchantments, [(8, 5)]);
    assert_eq!(stack.components.custom_name_text().as_deref(), Some("Miner"));

    let mut out = Vec::new();
    Some(stack).write(&mut out);
    let in_type_order = cat(&[&wire[..5], &wire[5..7], &wire[11..], &wire[7..11]]);
    assert_eq!(out, in_type_order);
}

#[test]
fn empty_stacks_and_patches() {
    assert_eq!(<Option<ItemStack>>::read(&mut &[0][..]), Ok(None));
    let mut out = Vec::new();
    Some(ItemStack::new(DIAMOND_PICKAXE, 0)).write(&mut out);
    assert_eq!(out, [0]);
    out.clear();
    Some(ItemStack::new(1, 64)).write(&mut out);
    assert_eq!(out, [64, 1, 0, 0]);
}

#[test]
fn decoded_components_round_trip() {
    let components = Components {
        max_stack_size: Some(16),
        max_damage: Some(250),
        damage: Some(3),
        unbreakable: true,
        custom_name: Some(TEXT.to_vec()),
        enchantments: vec![(1, 2), (300, 1)],
        map_id: Some(7),
        removed: vec![12, 18],
    };
    let mut out = Vec::new();
    components.write(&mut out);
    assert_eq!(Components::read(&mut &out[..]), Ok(components));
}

#[test]
fn nested_templates_carry_their_own_patches() {
    // A bundle holding a bundle holding a damaged item; the decoded outer patch keeps only its own damage.
    let inner = cat(&[&v(1153), &v(1), &[1, 0], &v(52), &[1], &template()]);
    let value = cat(&[&[1], &inner]);
    let bytes = cat(&[&[1], &v(1153), &[2, 0], &v(52), &value, &v(DAMAGE), &v(7)]);
    let stack = <Option<ItemStack>>::read(&mut &bytes[..]).unwrap().unwrap();
    assert_eq!(stack.components.damage, Some(7));
}

#[test]
fn hostile_nesting_is_cut_off() {
    let mut value = Vec::new();
    for _ in 0..MAX_DEPTH + 4 {
        value.extend(cat(&[&[1], &v(1153), &v(1), &[1, 0], &v(52)]));
    }
    let bytes = cat(&[&[1], &v(1153), &[1, 0], &v(52), &value]);
    assert_eq!(<Option<ItemStack>>::read(&mut &bytes[..]), Err(Error::TooDeep(MAX_DEPTH)));
}
