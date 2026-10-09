use glam::{DVec3, IVec3, Vec3};

use super::collide::tests::Floor;
use super::sprite::Behaviour;
use super::*;

fn ticks(p: &mut Particles, blocks: &Floor, n: usize, player: Option<IVec3>) {
    for _ in 0..n {
        p.tick(blocks, player);
    }
}

#[test]
fn chips_fly_fall_and_expire() {
    let mut p = Particles::default();
    p.break_block(IVec3::new(0, 64, 0), 7);
    assert_eq!(p.chips.len(), 64);
    assert!(p.chips.iter().all(|c| c.layer == 7 && c.piece.iter().all(|&o| o <= 8) && (0.05..=0.1).contains(&c.size)));
    let floor = Floor::new();
    ticks(&mut p, &floor, 5, None);
    assert!(p.chips.iter().all(|c| c.position.y >= 64.0), "nothing falls through the floor");
    ticks(&mut p, &floor, 60, None);
    assert!(p.chips.is_empty(), "every chip expires within 40 ticks");
}

#[test]
fn java_smoke_rises_and_fades_out() {
    let mut p = Particles { style: Style::Java, ..Default::default() };
    let at = DVec3::new(0.5, 65.0, 0.5);
    p.spawn(Kind::Smoke, at, Vec3::ZERO);
    let floor = Floor::new();
    ticks(&mut p, &floor, 4, None);
    assert!(p.sprites[0].position.y > at.y, "smoke rises");
    ticks(&mut p, &floor, 80, None);
    assert!(p.sprites.is_empty());
}

#[test]
fn a_drip_hangs_falls_and_splashes() {
    for style in [Style::Java, Style::Bedrock] {
        let mut p = Particles { style, ..Default::default() };
        p.spawn(Kind::DrippingWater, DVec3::new(0.5, 66.0, 0.5), Vec3::ZERO);
        let floor = Floor::new();
        ticks(&mut p, &floor, 30, None);
        assert!(matches!(p.sprites[0].behaviour, Behaviour::Hang { .. }) && p.sprites[0].position.y > 65.9, "{style:?}: still hanging");
        ticks(&mut p, &floor, 12, None);
        assert!(matches!(p.sprites[0].behaviour, Behaviour::Fall { .. }), "{style:?}: falling");
        ticks(&mut p, &floor, 30, None);
        assert!(p.sprites.iter().all(|s| s.behaviour == Behaviour::Drop), "{style:?}: landed as a splash");
    }
}

#[test]
fn a_torch_gives_off_flame_and_smoke_at_its_wick() {
    let mut floor = Floor::new();
    let torch = IVec3::new(2, 64, 3);
    floor.place(torch, "minecraft:torch", "torch_facing_direction=top");
    let mut p = Particles { style: Style::Java, ..Default::default() };
    ticks(&mut p, &floor, 200, Some(IVec3::new(0, 65, 0)));
    let near = |s: &&Sprite| (s.previous - DVec3::new(2.5, 64.7, 3.5)).length() < 0.5;
    assert!(p.sprites.iter().filter(near).any(|s| s.set == Set::Flame), "a flame");
    assert!(p.sprites.iter().filter(near).any(|s| s.set == Set::Generic), "smoke");
}

#[test]
fn a_lit_campfire_smokes_once_found() {
    let mut floor = Floor::new();
    floor.place(IVec3::new(1, 64, 1), "minecraft:campfire", "extinguished=0,minecraft:cardinal_direction=east");
    let mut p = Particles { style: Style::Bedrock, ..Default::default() };
    ticks(&mut p, &floor, 400, Some(IVec3::new(0, 65, 0)));
    assert!(p.sprites.iter().any(|s| s.set == Set::BigSmoke), "campfire smoke");
}

#[test]
fn bedrock_flames_are_full_bright_and_shrink() {
    let mut p = Particles::default();
    p.spawn(Kind::Flame, DVec3::new(0.5, 65.0, 0.5), Vec3::ZERO);
    let s = p.sprites[0];
    assert_eq!((s.light, s.set), (Light::Full, Set::Flame));
    let floor = Floor::new();
    let size = s.quad_size();
    ticks(&mut p, &floor, 10, None);
    assert!(p.sprites[0].quad_size() < size);
}

#[test]
fn a_crit_burst_scatters_crits() {
    let mut p = Particles { style: Style::Java, ..Default::default() };
    p.spawn(Kind::CritBurst, DVec3::new(0.0, 66.0, 0.0), Vec3::ZERO);
    assert_eq!(p.sprites.len(), 48);
    assert!(p.sprites.iter().all(|s| s.set == Set::Crit));
}
