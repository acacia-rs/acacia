//! `animateTick` rules on Bedrock states: what each block asks the level to add, drawn from
//! seeded randoms the way Java draws them.

use glam::{DVec3, IVec3};

use super::Scene;
use crate::particles::collide::Blocks;
use crate::particles::java::blocks_more::{offset_xz, wire_color};
use crate::particles::java::classify::{Animate, Tint, classify};
use crate::particles::java::level;
use crate::particles::java::{Engine, JavaRandom, Options, Request, Type};

const AT: IVec3 = IVec3::new(2, 64, 3);

fn state(scene: &mut Scene, name: &str, properties: &str) -> Animate {
    scene.place(AT, name, properties);
    classify(scene.block(AT).unwrap())
}

/// What one `animateTick` of the block at [`AT`] adds, with the animate random seeded `seed`.
fn animate(scene: &Scene, seed: i64) -> Vec<Request> {
    let mut engine = Engine::seeded(1);
    engine.animate = JavaRandom::new(seed);
    engine.level = JavaRandom::new(seed + 1);
    engine.requested = Some(Vec::new());
    engine.animate_cell(scene, AT);
    engine.requested.unwrap()
}

fn types(requests: &[Request]) -> Vec<Type> {
    requests.iter().map(|(o, ..)| o.ty).collect()
}

#[test]
fn bedrock_states_classify_as_their_java_blocks() {
    let mut s = Scene::new();
    assert_eq!(state(&mut s, "minecraft:torch", "torch_facing_direction=top"), Animate::Torch(Type::Flame, None));
    assert_eq!(state(&mut s, "minecraft:soul_torch", "torch_facing_direction=east"), Animate::Torch(Type::SoulFireFlame, Some(IVec3::X)));
    assert_eq!(state(&mut s, "minecraft:lit_furnace", "minecraft:cardinal_direction=north"), Animate::Furnace { facing: IVec3::NEG_Z, blast: false });
    assert_eq!(state(&mut s, "minecraft:furnace", "minecraft:cardinal_direction=north"), Animate::None);
    assert_eq!(state(&mut s, "minecraft:candle", "candles=2,lit=1"), Animate::Candles(3));
    assert_eq!(state(&mut s, "minecraft:azalea_leaves", "persistent_bit=0,update_bit=0"), Animate::Leaves(Some((Type::TintedLeaves, 0.01, Tint::Fixed(0x70_922D)))));
    assert_eq!(state(&mut s, "minecraft:end_rod", "facing_direction=2"), Animate::EndRod(IVec3::Z));
    assert_eq!(state(&mut s, "minecraft:lever", "lever_direction=east,open_bit=1"), Animate::Lever { facing: IVec3::NEG_X, attached: IVec3::NEG_X });
    assert_eq!(state(&mut s, "minecraft:lever", "lever_direction=up_north_south,open_bit=0"), Animate::Lever { facing: IVec3::NEG_Z, attached: IVec3::NEG_Y });
    assert_eq!(state(&mut s, "minecraft:redstone_wire", "redstone_signal=0"), Animate::None);
}

#[test]
fn a_torch_smokes_and_flames_at_its_wick() {
    let mut s = Scene::new();
    s.place(AT, "minecraft:torch", "torch_facing_direction=top");
    let wick = AT.as_dvec3() + DVec3::new(0.5, 0.7, 0.5);
    assert_eq!(animate(&s, 1), [(Options::simple(Type::Smoke), wick, DVec3::ZERO), (Options::simple(Type::Flame), wick, DVec3::ZERO)]);
    s.place(AT, "minecraft:torch", "torch_facing_direction=west");
    assert_eq!(animate(&s, 1)[0].1, wick + DVec3::new(-0.27, 0.22, 0.0));
}

#[test]
fn a_furnace_draws_its_crackle_then_its_spot() {
    let mut s = Scene::new();
    s.place(AT, "minecraft:lit_furnace", "minecraft:cardinal_direction=east");
    let mut r = JavaRandom::new(77);
    r.next_double();
    let ss = r.next_double() * 0.6 - 0.3;
    let dy = r.next_double() * 6.0 / 16.0;
    let p = AT.as_dvec3() + DVec3::new(0.5, 0.0, 0.5) + DVec3::new(0.52, dy, ss);
    assert_eq!(types(&animate(&s, 77)), [Type::Smoke, Type::Flame]);
    assert_eq!(animate(&s, 77)[0].1, p);
}

#[test]
fn candles_smoke_under_three_tenths_and_always_flame() {
    let mut s = Scene::new();
    s.place(AT, "minecraft:candle", "candles=3,lit=1");
    for seed in 0..20 {
        let requests = animate(&s, seed);
        assert_eq!(requests.iter().filter(|(o, ..)| o.ty == Type::SmallFlame).count(), 4);
        let mut r = JavaRandom::new(seed);
        let first = r.next_float();
        assert_eq!(requests[0].0.ty == Type::Smoke, first < 0.3, "seed {seed}");
    }
}

#[test]
fn a_campfire_rerolls_its_lava_count_each_pass() {
    let mut s = Scene::new();
    s.place(AT, "minecraft:campfire", "extinguished=0,minecraft:cardinal_direction=east");
    let spits = (0..200).map(|seed| animate(&s, seed)).filter(|r| r.iter().any(|(o, ..)| o.ty == Type::Lava)).count();
    assert!((20..=60).contains(&spits), "one tick in five: {spits}");
}

#[test]
fn lava_pops_only_under_open_air() {
    let mut s = Scene::new();
    s.flood(AT, "minecraft:lava", 0);
    let pops = (0..1000).filter(|&seed| types(&animate(&s, seed)).contains(&Type::Lava)).count();
    assert!((3..=25).contains(&pops), "one in a hundred: {pops}");
    s.place(AT + IVec3::Y, "minecraft:stone", "");
    assert!((0..1000).all(|seed| !types(&animate(&s, seed)).contains(&Type::Lava)));
}

#[test]
fn water_drips_through_a_ceiling_from_the_levels_random() {
    let mut s = Scene::new();
    let water = IVec3::new(0, 80, 0);
    s.place(water + IVec3::NEG_Y, "minecraft:stone", "").flood(water, "minecraft:water", 0);
    let mut engine = Engine::seeded(1);
    engine.requested = Some(Vec::new());
    for _ in 0..200 {
        engine.animate_cell(&s, water);
    }
    let drips: Vec<_> = engine.requested.unwrap().into_iter().filter(|(o, ..)| o.ty == Type::DrippingWater).collect();
    assert!(!drips.is_empty() && drips.iter().all(|(_, p, _)| (p.y - (79.0 - 0.05)).abs() < 1e-9), "{drips:?}");
}

#[test]
fn glass_holds_water_up() {
    let mut s = Scene::new();
    let water = IVec3::new(0, 80, 0);
    s.place(water + IVec3::NEG_Y, "minecraft:glass", "").flood(water, "minecraft:water", 0);
    let mut engine = Engine::seeded(1);
    engine.requested = Some(Vec::new());
    for _ in 0..200 {
        engine.animate_cell(&s, water);
    }
    assert!(engine.requested.unwrap().iter().all(|(o, ..)| o.ty != Type::DrippingWater));
}

#[test]
fn tinted_leaves_fall_in_the_foliage_colour() {
    let mut s = Scene::new();
    s.place(AT + IVec3::Y * 4, "minecraft:oak_leaves", "persistent_bit=0,update_bit=0");
    let mut engine = Engine::seeded(1);
    engine.foliage = 0x11_2233;
    engine.requested = Some(Vec::new());
    for _ in 0..2000 {
        engine.animate_cell(&s, AT + IVec3::Y * 4);
    }
    let leaves = engine.requested.unwrap();
    assert!(!leaves.is_empty() && leaves.iter().all(|(o, ..)| o.ty == Type::TintedLeaves && o.color[0] == 0x11 as f32 / 255.0), "{leaves:?}");
}

#[test]
fn redstone_wire_colours_match_the_wire_table() {
    let c = wire_color(15);
    assert!((c[0] - 1.0).abs() < 1e-6 && (c[1] - 0.2).abs() < 1e-6 && c[2] == 0.0);
    let mut s = Scene::new();
    s.place(AT, "minecraft:redstone_wire", "redstone_signal=15");
    // Java floors 0.19999999 * 255 to 50 when it packs the colour.
    assert!((0..50).flat_map(|seed| animate(&s, seed)).all(|(o, p, _)| o.ty == Type::Dust && o.color[1] == 50.0 / 255.0 && p.y < 64.1));
}

#[test]
fn block_offsets_and_seeds_match_java() {
    assert_eq!(level::seed(1, 0, 2), -98_563_738_688_972);
    assert_eq!(level::seed(-700, 0, 1234), -103_676_343_830_122);
    let o = offset_xz(IVec3::new(1, 0, 2), 0.125);
    assert_eq!((o.x, o.z), (-0.11666665971279144, -0.125));
    let o = offset_xz(IVec3::new(-700, 0, 1234), 0.125);
    assert_eq!((o.x, o.z), (-0.04999999701976776, 0.050000011920928955));
}

#[test]
fn a_campfire_found_by_sampling_smokes_from_the_level_random() {
    let mut s = Scene::new();
    s.place(AT, "minecraft:campfire", "extinguished=0,minecraft:cardinal_direction=east");
    s.place(AT - IVec3::Y, "minecraft:hay_block", "");
    let mut engine = Engine::seeded(1);
    engine.requested = Some(Vec::new());
    engine.animate_cell(&s, AT);
    engine.requested = Some(Vec::new());
    for _ in 0..100 {
        engine.campfires_tick(&s, AT);
    }
    let smoke = engine.requested.unwrap();
    assert!(!smoke.is_empty() && smoke.iter().all(|(o, _, v)| o.ty == Type::CampfireSignalSmoke && v.y == 0.07));
}
