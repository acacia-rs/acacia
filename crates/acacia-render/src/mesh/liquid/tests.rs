use acacia_world::{BlockRegistry, BlockState};

use super::super::model::{CORNER2, MODEL};
use super::super::volume::{Volume, cell};
use super::super::{Ctx, SectionMesh};
use super::{LiquidFace, Sprite, java_faces, liquid};
use crate::biome::BiomeColors;
use crate::blocks::{BlockTable, Fluid, FluidRules, Layer, Material, RenderBlock, Shape, Tint};

const AIR: u32 = 0;
const STONE: u32 = 1;
/// Water with `liquid_depth` 0..=8 (8 falling).
const WATER: u32 = 2;
const LAVA: u32 = 11;
const GLASS: u32 = 12;
const SLAB: u32 = 13;
const STAIRS_EAST: u32 = 14;
const STAIRS_WEST: u32 = 15;
const LEAVES: u32 = 16;

fn state(name: &str, props: &[&str]) -> &'static BlockState {
    let reg = BlockRegistry::vanilla();
    let found = reg.states_of(name).find(|(_, s)| props.iter().all(|p| s.properties.split(',').any(|kv| kv == *p)));
    found.unwrap_or_else(|| panic!("no state {name} {props:?}")).1
}

fn block(s: &BlockState, base: RenderBlock) -> RenderBlock {
    RenderBlock { fluid_rules: FluidRules::of(s), ..base }
}

fn table() -> BlockTable {
    let cube = BlockTable::cube(0);
    let none = RenderBlock { shape: Shape::None, layer: Layer::Invisible, occludes: false, shades: false, ..cube.clone() };
    let water = |depth: u8| {
        let s = state(if depth == 0 { "minecraft:water" } else { "minecraft:flowing_water" }, &[&format!("liquid_depth={depth}")]);
        let base = RenderBlock {
            shape: Shape::Liquid,
            layer: Layer::Translucent,
            textures: [10, 11, 12, 12, 10, 10],
            tint: [Tint::Water; 6],
            material: [Material::Blend; 6],
            fluid: Fluid::Water,
            ..none.clone()
        };
        block(s, base)
    };
    let lava = RenderBlock { shape: Shape::Liquid, layer: Layer::Solid, textures: [20, 20, 21, 21, 20, 20], fluid: Fluid::Lava, ..none.clone() };
    let see_through = RenderBlock { layer: Layer::Translucent, occludes: false, ..cube.clone() };
    let boxes = RenderBlock { shape: Shape::Boxes(Box::new([])), occludes: false, ..cube.clone() };
    let stairs = |d: &str| block(state("minecraft:oak_stairs", &[&format!("weirdo_direction={d}"), "upside_down_bit=0", "minecraft:corner=none"]), boxes.clone());
    let mut blocks = vec![block(state("minecraft:air", &[]), none.clone()), block(state("minecraft:stone", &[]), cube.clone())];
    blocks.extend((0..=8).map(water));
    blocks.extend([
        block(state("minecraft:lava", &["liquid_depth=0"]), lava),
        block(state("minecraft:glass", &[]), see_through.clone()),
        block(state("minecraft:smooth_stone_slab", &["minecraft:vertical_half=bottom"]), boxes.clone()),
        stairs("0"),
        stairs("1"),
        block(state("minecraft:oak_leaves", &[]), see_through),
    ]);
    BlockTable::from_blocks(blocks)
}

/// Air everywhere, stone at y 4 under everything.
fn world(set: &[([i32; 3], u32)]) -> Volume {
    let mut v = Volume::filled(AIR);
    (-1..17).for_each(|x| (-1..17).for_each(|z| v.blocks[cell(x, 4, z)] = STONE));
    set.iter().for_each(|&([x, y, z], id)| v.blocks[cell(x, y, z)] = id);
    v
}

fn faces(v: &Volume, p: [i32; 3]) -> Vec<LiquidFace> {
    java_faces(v, &table(), &BiomeColors::default(), p)
}

fn by_face(faces: &[LiquidFace], face: usize) -> Vec<&LiquidFace> {
    faces.iter().filter(|f| f.face == face).collect()
}

fn pool(r: std::ops::RangeInclusive<i32>) -> Vec<([i32; 3], u32)> {
    r.clone().flat_map(|x| r.clone().map(move |z| ([x, 5, z], WATER))).collect()
}

#[test]
fn a_still_pool_shows_its_surface_from_both_sides_and_walls_only_at_its_edge() {
    let v = world(&pool(4..=6));
    let centre = faces(&v, [5, 5, 5]);
    // Up, and its backward face: the cells above are air.
    assert_eq!(centre.len(), 2, "{centre:?}");
    let top = &centre[0];
    assert_eq!((top.face, top.sprite, top.light), (2, Sprite::Still, [5, 5, 5]));
    assert!(top.corners.iter().all(|c| (c[1] - (5.0 + 8.0 / 9.0 - 0.001)).abs() < 1e-5), "{top:?}");
    assert_eq!(top.uv, [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]);
    assert_eq!(centre[1].corners, [0, 3, 2, 1].map(|i| top.corners[i]));
    // The corner cell walls off air to the west and north, with flowing sides and their back faces.
    let edge = faces(&v, [4, 5, 4]);
    for face in [1, 5] {
        let walls = by_face(&edge, face);
        assert_eq!(walls.len(), 2);
        assert_eq!((walls[0].sprite, walls[0].uv[2], walls[0].uv[3]), (Sprite::Flow, [0.5, 0.5], [0.0, 0.5]));
        assert!((walls[0].corners[2][1] - 5.0).abs() < 1e-6, "no bottom face: the side reaches the floor");
    }
    // Air weighs one tenth of a nearly full cell: the outer corner sags.
    let nw = edge[0].corners[0][1] - 5.0;
    assert!((nw - (8.0 / 9.0 * 10.0 / 12.0 - 0.001)).abs() < 1e-5, "{nw}");
    assert_eq!(by_face(&edge, 3).len(), 0, "stone below hides the bottom");
    let covered = world(&[([5, 5, 5], WATER), ([5, 6, 5], WATER)]);
    let below = faces(&covered, [5, 5, 5]);
    assert!(by_face(&below, 2).is_empty() && by_face(&below, 0)[0].corners[0][1] == 6.0, "under more water the sides reach the top");
}

#[test]
fn flowing_water_slopes_and_its_texture_turns_downstream() {
    // A source at x 4, then liquid_depth 1, 2, 3 eastwards.
    let v = world(&[([4, 5, 5], WATER), ([5, 5, 5], WATER + 1), ([6, 5, 5], WATER + 2), ([7, 5, 5], WATER + 3)]);
    let f = faces(&v, [5, 5, 5]);
    let top = by_face(&f, 2)[0];
    assert_eq!(top.sprite, Sprite::Flow);
    // Flow along +x: Mth.atan2(0, 1) - pi/2 turns the texture a quarter.
    let [u, v0] = top.uv[0];
    assert!((u - 0.75).abs() < 1e-6 && (v0 - 0.25).abs() < 1e-6, "{:?}", top.uv);
    let [nw, ne] = [top.corners[0][1], top.corners[3][1]];
    assert!(nw > ne, "higher upstream: {nw} {ne}");
}

#[test]
fn water_spilling_over_an_edge_flows_towards_it() {
    let v = world(&[([5, 5, 5], WATER), ([6, 4, 5], WATER)]);
    let top = faces(&v, [5, 5, 5]).into_iter().find(|f| f.face == 2).unwrap();
    assert_eq!(top.sprite, Sprite::Flow);
    assert!((top.uv[0][0] - 0.75).abs() < 1e-6, "{:?}", top.uv);
}

#[test]
fn falling_water_against_a_wall_shows_its_still_texture() {
    let v = world(&[([5, 5, 5], WATER + 8), ([6, 5, 5], STONE)]);
    let f = faces(&v, [5, 5, 5]);
    assert_eq!(by_face(&f, 2)[0].sprite, Sprite::Still);
    assert!(by_face(&f, 0).is_empty() && by_face(&f, 1).len() == 2);
}

#[test]
fn a_side_hides_behind_what_covers_it_to_the_surface() {
    let east = |against: u32| by_face(&faces(&world(&[([5, 5, 5], WATER), ([6, 5, 5], against)]), [5, 5, 5]), 0).len();
    assert_eq!((east(AIR), east(STONE), east(SLAB)), (2, 0, 2), "a bottom slab covers half the side");
    // One of the stairs turns its full face to the water.
    assert_eq!(east(STAIRS_EAST) + east(STAIRS_WEST), 2);
}

#[test]
fn water_beside_glass_and_leaves_shows_the_overlay_once() {
    for against in [GLASS, LEAVES] {
        let sides = by_face(&faces(&world(&[([5, 5, 5], WATER), ([6, 5, 5], against)]), [5, 5, 5]), 0).into_iter().cloned().collect::<Vec<_>>();
        assert_eq!(sides.len(), 1, "no back face");
        assert_eq!(sides[0].sprite, Sprite::Overlay);
    }
}

#[test]
fn a_waterlogged_slab_keeps_its_bottom_and_shows_its_sides() {
    let mut v = world(&[([5, 5, 5], SLAB)]);
    v.liquid[cell(5, 5, 5)] = WATER;
    v.blocks[cell(5, 4, 5)] = AIR;
    let f = faces(&v, [5, 5, 5]);
    assert!(by_face(&f, 3).is_empty(), "the slab's own bottom covers the water's");
    assert_eq!(by_face(&f, 0).len(), 2);
}

#[test]
fn lava_is_white_flowing_and_solid() {
    let v = world(&[([5, 5, 5], LAVA)]);
    let f = faces(&v, [5, 5, 5]);
    assert!(f.iter().all(|f| f.tint == [255; 3]));
    assert_eq!(by_face(&f, 4)[0].color(false), [204; 3]);
    assert_eq!(by_face(&f, 2)[0].color(true), [229; 3]);
    let table = table();
    let mut out = SectionMesh::default();
    liquid(&Ctx { v: &v, table: &table, biomes: &BiomeColors::default() }, [5, 5, 5], &mut out);
    assert!(out.translucent.is_empty() && out.solid.len() == 4 * f.len());
    let head = out.solid[0].0[0];
    assert_eq!(head & (MODEL | CORNER2), MODEL | CORNER2);
    // The layer of each sprite: flowing sides read slot 4, the still top slot 2.
    assert_eq!(head >> 16 & 4095, 21);
}
