use glam::{DVec3, IVec3, Vec3};

use super::collide::tests::Floor;
use super::java::Type;
use super::sprite::Behaviour;
use super::*;

fn ticks(p: &mut Particles, blocks: &Floor, n: usize, player: Option<IVec3>) {
    for _ in 0..n {
        p.tick(blocks, player, DVec3::new(0.0, 65.0, 0.0));
    }
}

/// The camera starts where `ticks` keeps it; Java drops spawns over 32 blocks away.
fn styled(style: Style) -> Particles {
    let mut p = Particles { style, ..Default::default() };
    p.java.camera = DVec3::new(0.0, 65.0, 0.0);
    p
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
fn java_smoke_rises_and_goes_out() {
    let mut p = styled(Style::Java);
    let at = DVec3::new(0.5, 65.0, 0.5);
    p.spawn(Kind::Smoke, at, Vec3::ZERO);
    let floor = Floor::new();
    ticks(&mut p, &floor, 5, None);
    assert!(p.java.particles[0].position.y > at.y, "smoke rises");
    ticks(&mut p, &floor, 80, None);
    assert!(p.java.particles.is_empty());
}

#[test]
fn a_bedrock_drip_hangs_falls_and_splashes() {
    let mut p = styled(Style::Bedrock);
    p.spawn(Kind::DrippingWater, DVec3::new(0.5, 66.0, 0.5), Vec3::ZERO);
    let floor = Floor::new();
    ticks(&mut p, &floor, 30, None);
    assert!(matches!(p.sprites[0].behaviour, Behaviour::Hang { .. }) && p.sprites[0].position.y > 65.9, "still hanging");
    ticks(&mut p, &floor, 12, None);
    assert!(matches!(p.sprites[0].behaviour, Behaviour::Fall { .. }), "falling");
    ticks(&mut p, &floor, 30, None);
    assert!(p.sprites.iter().all(|s| s.behaviour == Behaviour::Drop), "landed as a splash");
}

#[test]
fn a_java_drip_hangs_falls_and_splashes() {
    let mut p = styled(Style::Java);
    p.spawn(Kind::DrippingWater, DVec3::new(0.5, 66.0, 0.5), Vec3::ZERO);
    let floor = Floor::new();
    let types = |p: &Particles| p.java.particles.iter().map(|q| q.ty).collect::<Vec<_>>();
    ticks(&mut p, &floor, 30, None);
    assert_eq!(types(&p), [Type::DrippingWater]);
    ticks(&mut p, &floor, 12, None);
    assert_eq!(types(&p), [Type::FallingWater]);
    ticks(&mut p, &floor, 30, None);
    assert!(types(&p).iter().all(|&t| t == Type::Splash), "{:?}", types(&p));
}

#[test]
fn a_torch_gives_off_flame_and_smoke_at_its_wick() {
    let mut floor = Floor::new();
    floor.place(IVec3::new(2, 64, 3), "minecraft:torch", "torch_facing_direction=top");
    let mut p = styled(Style::Java);
    ticks(&mut p, &floor, 200, Some(IVec3::new(0, 65, 0)));
    let near: Vec<_> = p.java.particles.iter().filter(|q| (q.previous - DVec3::new(2.5, 64.7, 3.5)).length() < 0.5).map(|q| q.ty).collect();
    assert!(near.contains(&Type::Flame) && near.contains(&Type::Smoke), "{near:?}");
}

#[test]
fn a_lit_campfire_smokes_once_found() {
    let mut floor = Floor::new();
    floor.place(IVec3::new(1, 64, 1), "minecraft:campfire", "extinguished=0,minecraft:cardinal_direction=east");
    let mut p = styled(Style::Bedrock);
    ticks(&mut p, &floor, 400, Some(IVec3::new(0, 65, 0)));
    assert!(p.sprites.iter().any(|s| s.set == Set::BigSmoke), "Bedrock campfire smoke");
    let mut p = styled(Style::Java);
    ticks(&mut p, &floor, 400, Some(IVec3::new(0, 65, 0)));
    assert!(p.java.particles.iter().any(|q| q.ty == Type::CampfireCosySmoke), "Java campfire smoke");
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
fn a_java_crit_burst_emits_for_three_ticks() {
    let mut p = styled(Style::Java);
    p.spawn(Kind::CritBurst, DVec3::new(0.0, 66.0, 0.0), Vec3::ZERO);
    let floor = Floor::new();
    ticks(&mut p, &floor, 1, None);
    let first = p.java.particles.len();
    assert!((4..=16).contains(&first), "about half of 16 tries land in the ball: {first}");
    ticks(&mut p, &floor, 3, None);
    assert!(p.java.particles.len() > first && p.java.particles.iter().all(|q| q.ty == Type::Crit));
}

#[test]
fn switching_style_drops_the_other_styles_particles() {
    let mut p = styled(Style::Java);
    p.spawn(Kind::Flame, DVec3::new(0.5, 65.0, 0.5), Vec3::ZERO);
    p.tick(&Floor::new(), None, DVec3::new(0.0, 65.0, 0.0));
    assert_eq!(p.java.particles.len(), 1);
    p.set_style(Style::Bedrock);
    assert!(p.java.particles.is_empty() && p.style() == Style::Bedrock);
}
