//! The particle rules one at a time: constructors, providers and ticks with seeded randoms.

mod animate;

use acacia_world::{BlockRegistry, BlockState};
use glam::{DVec3, IVec3};

use super::particle::{Glow, Size};
use super::*;
use crate::particles::collide::Blocks;

/// Stone below y 64, air above, placed blocks and liquids on top.
pub struct Scene {
    registry: &'static BlockRegistry,
    placed: Vec<(IVec3, u32)>,
    liquids: Vec<(IVec3, u32)>,
}

impl Scene {
    pub fn new() -> Scene {
        Scene { registry: BlockRegistry::vanilla(), placed: Vec::new(), liquids: Vec::new() }
    }

    /// Empty `properties` takes the block's first state.
    fn id(&self, name: &str, properties: &str) -> u32 {
        let id = if properties.is_empty() { self.registry.states_of(name).next().map(|(id, _)| id) } else { self.registry.find(name, properties) };
        id.unwrap_or_else(|| panic!("vanilla state {name}[{properties}]"))
    }

    pub fn place(&mut self, cell: IVec3, name: &str, properties: &str) -> &mut Scene {
        let id = self.id(name, properties);
        self.placed.push((cell, id));
        self
    }

    /// A liquid layer over whatever block is there.
    pub fn flood(&mut self, cell: IVec3, name: &str, depth: u8) -> &mut Scene {
        let id = self.id(name, &format!("liquid_depth={depth}"));
        self.liquids.push((cell, id));
        self
    }
}

impl Blocks for Scene {
    fn block(&self, cell: IVec3) -> Option<&BlockState> {
        let placed = self.placed.iter().rev().find(|(c, _)| *c == cell).map(|(_, id)| *id);
        let id = placed.unwrap_or_else(|| self.id(if cell.y < 64 { "minecraft:stone" } else { "minecraft:air" }, ""));
        self.registry.get(id)
    }

    fn liquid(&self, cell: IVec3) -> Option<&BlockState> {
        self.liquids.iter().rev().find(|(c, _)| *c == cell).and_then(|(_, id)| self.registry.get(*id))
    }
}

fn made(ty: Type, p: DVec3, v: DVec3, seed: i64) -> Particle {
    Engine::seeded(1).probe_create(&Options::simple(ty), p, v, seed, &Scene::new()).expect("a particle")
}

const UP: DVec3 = DVec3::new(0.5, 65.0, 0.5);

#[test]
fn the_base_constructor_draws_lifetime_then_size() {
    let mut r = JavaRandom::new(5);
    let q = Particle::at(&mut r, Type::Flame, UP);
    let mut check = JavaRandom::new(5);
    assert_eq!(q.lifetime, (4.0 / (check.next_float() * 0.9 + 0.1)) as i32);
    assert_eq!(q.size, 0.1 * (check.next_float() * 0.5 + 0.5) * 2.0);
    assert_eq!(r.next_long(), check.next_long(), "nothing else drawn");
}

#[test]
fn a_moving_particle_is_jittered_to_a_random_speed_and_lifted() {
    let mut r = JavaRandom::new(9);
    let q = Particle::base_moving(&mut r, Type::Smoke, UP, DVec3::ZERO);
    let horizontal = q.velocity.with_y(0.0).length();
    assert!(horizontal <= 0.15 * 3.0 * 0.4 + 1e-9 && q.velocity.y > 0.0, "{}", q.velocity);
}

#[test]
fn flames_jitter_their_spot_but_not_their_box() {
    let q = made(Type::Flame, UP, DVec3::ZERO, 3);
    assert!((q.position - UP).abs().max_element() <= 0.05 && q.position != UP);
    assert!((12..=44).contains(&q.lifetime), "{}", q.lifetime);
    let mut q = q;
    q.base_tick(&Scene::new());
    let centre = DVec3::new((q.bb_min.x + q.bb_max.x) / 2.0, q.bb_min.y, (q.bb_min.z + q.bb_max.z) / 2.0);
    assert_eq!(q.position, centre, "the first move snaps it back onto its box");
    assert_eq!((q.size_curve, q.glow), (Size::Flame, Glow::AgeEmission));
}

#[test]
fn a_small_flame_is_half_a_flame() {
    let (big, small) = (made(Type::Flame, UP, DVec3::ZERO, 7), made(Type::SmallFlame, UP, DVec3::ZERO, 7));
    assert_eq!(small.size, big.size * 0.5);
    assert_eq!(small.bb_width, 0.1);
}

#[test]
fn smoke_rises_slows_and_steps_its_sprites_by_age() {
    let scene = Scene::new();
    let mut engine = Engine::seeded(1);
    let mut q = engine.probe_create(&Options::simple(Type::Smoke), UP, DVec3::ZERO, 11, &scene).unwrap();
    assert_eq!(q.sprite, Some("generic_7"));
    let y = q.position.y;
    for _ in 0..q.lifetime {
        engine.probe_tick(&mut q, &scene);
    }
    assert!(q.position.y > y && !q.removed);
    assert_eq!(q.sprite, Some("generic_0"), "the last frame at the end of its life");
    engine.probe_tick(&mut q, &scene);
    assert!(q.removed);
}

#[test]
fn quad_sizes_follow_each_classes_curve() {
    let mut q = made(Type::Lava, UP, DVec3::ZERO, 1);
    q.lifetime = 10;
    q.age = 5;
    assert!((q.quad_size(0.0) - q.size * 0.75).abs() < 1e-6, "lava: 1 - t²");
    q.size_curve = Size::Flame;
    assert!((q.quad_size(0.0) - q.size * 0.875).abs() < 1e-6, "flame: 1 - t²/2");
    q.size_curve = Size::Portal;
    assert!((q.quad_size(0.0) - q.size * 0.75).abs() < 1e-6, "portal: 1 - (1 - t)²");
    q.size_curve = Size::GrowIn;
    q.age = 0;
    assert!((q.quad_size(0.16) - q.size * 0.512).abs() < 1e-6, "full within a 32nd of its life");
}

#[test]
fn flame_light_rises_to_full_block_light_over_its_life() {
    let mut q = made(Type::Flame, UP, DVec3::ZERO, 2);
    q.lifetime = 20;
    q.age = 10;
    assert_eq!(q.light(0.0, [4.0, 9.0]), [11.5, 9.0]);
    q.age = 20;
    assert_eq!(q.light(0.0, [4.0, 9.0]), [15.0, 9.0]);
    q.glow = Glow::Block15;
    assert_eq!(q.light(0.0, [4.0, 9.0]), [15.0, 9.0]);
}

#[test]
fn set_sprite_from_age_spans_the_set() {
    let mut q = made(Type::Explosion, UP, DVec3::ZERO, 4);
    q.lifetime = 8;
    q.age = 4;
    q.sprite_from_age();
    assert_eq!(q.sprite, Some("explosion_7"), "4 * 15 / 8");
}

#[test]
fn lava_leaves_smoke_more_early_on() {
    let scene = Scene::new();
    let mut engine = Engine::seeded(3);
    engine.requested = Some(Vec::new());
    let mut q = engine.probe_create(&Options::simple(Type::Lava), UP, DVec3::ZERO, 21, &scene).unwrap();
    for _ in 0..4 {
        engine.probe_tick(&mut q, &scene);
    }
    let smoke = engine.requested.as_ref().unwrap();
    assert!(!smoke.is_empty() && smoke.iter().all(|(o, ..)| o.ty == Type::Smoke));
}

#[test]
fn a_drip_lands_in_the_fluid_it_dripped_from() {
    let mut scene = Scene::new();
    scene.flood(IVec3::new(0, 64, 0), "minecraft:water", 0);
    let mut engine = Engine::seeded(1);
    let mut q = engine.probe_create(&Options::simple(Type::FallingWater), DVec3::new(0.5, 65.5, 0.5), DVec3::ZERO, 5, &scene).unwrap();
    for _ in 0..40 {
        engine.probe_tick(&mut q, &scene);
        if q.removed {
            break;
        }
    }
    assert!(q.removed && q.position.y < 65.0 && q.position.y > 64.0, "{}", q.position.y);
}

#[test]
fn bubbles_pop_out_of_water() {
    let mut scene = Scene::new();
    scene.flood(IVec3::new(0, 64, 0), "minecraft:water", 0);
    let mut engine = Engine::seeded(1);
    let mut q = engine.probe_create(&Options::simple(Type::Bubble), DVec3::new(0.5, 64.5, 0.5), DVec3::new(0.0, 0.5, 0.0), 8, &scene).unwrap();
    for _ in 0..q.lifetime + 1 {
        engine.probe_tick(&mut q, &scene);
    }
    assert!(q.removed);
}

#[test]
fn dust_keeps_its_packed_colour_and_scale() {
    let o = Options::dust([1.0, 0.2, 0.1], 1.0);
    assert_eq!(o.color[1], 51.0 / 255.0);
    assert_eq!(o.color[2], 25.0 / 255.0);
    let q = Engine::seeded(1).make(&o, UP, DVec3::ZERO, 6).unwrap();
    assert!(q.rgba[0] <= 1.0 && q.rgba[0] >= 0.8 * 0.6);
}

#[test]
fn a_note_is_coloured_by_its_pitch() {
    let low = made(Type::Note, UP, DVec3::ZERO, 1);
    let high = made(Type::Note, UP, DVec3::new(0.5, 0.0, 0.0), 1);
    assert!((low.rgba[0] - 0.35).abs() < 1e-3 && (high.rgba[0] - 0.35).abs() < 1e-3);
    assert!(low.rgba[1] > high.rgba[1], "{:?} {:?}", low.rgba, high.rgba);
}

#[test]
fn far_spawns_are_dropped_unless_the_type_overrides_the_limiter() {
    let mut engine = Engine::seeded(1);
    engine.add(&Options::simple(Type::Flame), DVec3::new(40.0, 65.0, 0.0), DVec3::ZERO);
    engine.add(&Options::simple(Type::Poof), DVec3::new(40.0, 65.0, 0.0), DVec3::ZERO);
    engine.tick(&Scene::new());
    assert_eq!(engine.particles.iter().map(|q| q.ty).collect::<Vec<_>>(), [Type::Poof]);
}

#[test]
fn an_explosion_seed_scatters_six_puffs_a_tick_for_eight_ticks() {
    let mut engine = Engine::seeded(1);
    engine.add(&Options::simple(Type::ExplosionEmitter), UP, DVec3::ZERO);
    let scene = Scene::new();
    for _ in 0..10 {
        engine.tick(&scene);
    }
    let puffs = engine.particles.iter().filter(|q| q.ty == Type::Explosion).count();
    assert!(engine.particles.iter().all(|q| q.ty != Type::ExplosionEmitter));
    assert!(puffs > 0 && puffs <= 48, "{puffs}");
}

#[test]
fn probes_dump_a_blocks_effects_and_a_particles_life() {
    let mut scene = Scene::new();
    let torch = IVec3::new(0, 64, 0);
    scene.place(torch, "minecraft:torch", "torch_facing_direction=top");
    let ticks = probe::block(&scene, torch, 5, 3, 0, &[]);
    assert_eq!(ticks.iter().map(|t| (t.spawns.len(), t.particles.len())).collect::<Vec<_>>(), [(2, 2), (2, 4), (2, 6)]);
    assert_eq!(ticks, probe::block(&scene, torch, 5, 3, 0, &[]), "the same seed, the same dump");
    let life = probe::particle(&scene, &Options::simple(Type::Smoke), UP, DVec3::ZERO, 8, 100, &[]);
    let lived = life.iter().take_while(|t| !t.particles.is_empty()).count() as i32;
    assert_eq!(lived, life[0].particles[0].lifetime + 1, "admitted unticked, then each tick of its life");
    assert_eq!(life[0].particles[0].sprite, Some("generic_7"));
    let mut seeds = probe::Seeds::default();
    seeds.insert(probe::key(Type::Smoke, UP, DVec3::ZERO), 77);
    let given = probe::particle(&scene, &Options::simple(Type::Smoke), UP, DVec3::ZERO, 8, 1, &[seeds]);
    assert_eq!(given[0].particles[0].seed, 77, "a particle Java made takes Java's seed");
}

#[test]
fn the_engine_is_deterministic_for_a_seed() {
    let run = || {
        let mut engine = Engine::seeded(42);
        engine.camera = UP;
        let scene = Scene::new();
        engine.add(&Options::simple(Type::Smoke), UP, DVec3::ZERO);
        engine.add(&Options::simple(Type::Flame), UP, DVec3::ZERO);
        for _ in 0..5 {
            engine.tick(&scene);
        }
        engine.particles.iter().map(|q| (q.position, q.lifetime, q.sprite)).collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}
