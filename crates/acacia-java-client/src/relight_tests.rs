use acacia_java_proto::chunk::{Section, Values};
use acacia_java_proto::light::LightSection;
use acacia_registry::Block;

use super::*;

fn stone() -> u32 {
    Block::Stone.default_state().id()
}

/// A column at min y 0 with a stone floor in section 0 and light as a server sends it: sections -1..1 stored,
/// dark through the floor and full above.
fn floored() -> World {
    let mut world = World { dimension: "minecraft:overworld".into(), ..World::default() };
    let air = Section { non_empty: 0, fluids: 0, blocks: Values::Single(0), biomes: Values::Single(0) };
    let mut column = vec![air; 24];
    column[0] = Section { non_empty: 4096, blocks: Values::Single(stone()), ..column[0].clone() };
    world.chunks.insert(0, 0, column);
    let full = LightSection::Data(vec![0xff; 2048].into());
    world.sky.queue((0, 0), vec![(0, LightSection::Empty), (1, LightSection::Empty), (2, full)]);
    world.update_light();
    world
}

fn lit(world: &World, pos: [i32; 3]) -> bool {
    world.sky.sees_sky(pos, world.min_y())
}

#[test]
fn a_roof_darkens_the_column_under_it_after_the_frame() {
    let mut world = floored();
    world.set_block([3, 18, 3], stone());
    assert!(lit(&world, [3, 17, 3]), "relit only by the frame after the tick");
    world.update_light();
    assert!(!lit(&world, [3, 17, 3]) && !lit(&world, [3, 16, 3]), "under the roof");
    assert!(lit(&world, [3, 19, 3]) && lit(&world, [4, 17, 3]), "on it and beside it");
    world.set_block([3, 18, 3], 0);
    world.update_light();
    assert!(lit(&world, [3, 16, 3]), "the roof is gone");
}

#[test]
fn glass_lets_the_sky_through_and_a_bottom_slab_stops_it() {
    let mut world = floored();
    world.set_block([3, 18, 3], Block::Glass.default_state().id());
    world.set_block([5, 18, 3], Block::StoneSlab.default_state().id());
    world.update_light();
    assert!(lit(&world, [3, 17, 3]) && !lit(&world, [5, 17, 3]));
}

#[test]
fn a_roof_in_an_unstored_section_stores_it_and_its_neighbours() {
    let mut world = floored();
    world.set_block([3, 100, 3], stone());
    assert!(lit(&world, [3, 99, 3]), "new sections above the data start full");
    world.update_light();
    assert!(!lit(&world, [3, 99, 3]) && !lit(&world, [3, 50, 3]) && !lit(&world, [3, 20, 3]), "dark down to the floor");
    assert!(lit(&world, [3, 101, 3]) && lit(&world, [4, 99, 3]));
    world.set_block([3, 100, 3], 0);
    world.update_light();
    assert!(lit(&world, [3, 50, 3]) && lit(&world, [3, 99, 3]) && lit(&world, [3, 20, 3]));
    assert!(world.sky.layer((0, 0), 7).is_none(), "the emptied section is dropped");
}

#[test]
fn server_light_waits_for_the_frame_and_wins_over_the_check() {
    let mut world = floored();
    world.set_block([3, 18, 3], stone());
    world.sky.queue((0, 0), vec![(2, LightSection::Data(vec![0xff; 2048].into()))]);
    world.update_light();
    assert!(lit(&world, [3, 17, 3]), "the server's data, applied after the check, says lit");
}
