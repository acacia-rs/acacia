use super::quad::{LIT, SPLIT_13};
use super::smooth::*;
use super::vertex_light::cube_face;
use super::volume::{Volume, cell};
use super::{Ctx, SectionMesh, greedy};
use crate::biome::BiomeColors;
use crate::blocks::{BlockTable, LightFacts, RenderBlock, Shape};
use crate::light::data::{self, LightCells};
use crate::look::Lighting;

const fn packed(block: u32, sky: u32) -> u32 {
    block << 4 | sky << 20
}

#[test]
fn smooth_blend_replaces_dark_samples_with_the_centre() {
    let center = packed(2, 15);
    let blended = smooth_blend(0, packed(8, 0), packed(4, 15), center);
    assert_eq!(blended & 0xFF, (32 + 128 + 64 + 32) / 4);
    assert_eq!(blended >> 16 & 0xFF, 240);
    assert_eq!(smooth_blend(0, 0, packed(15, 15), packed(1, 1)) & 0xFF, (240 + 16) / 4);
}

#[test]
fn colour_arithmetic_matches_argb_helpers() {
    assert_eq!((grey(1.0), grey(0.8)), (255, 204));
    assert_eq!((scale(255, 0.5), scale(grey(0.6), 0.8)), (127, 122));
    assert_eq!((cardinal(true, DOWN), cardinal(false, DOWN), cardinal(true, EAST)), (0.9, 0.5, 0.6));
}

#[test]
fn quad_shape_classifies_partial_and_cubic_faces() {
    let full_top = [[0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 0.0]];
    let s = shape(&full_top, UP, false);
    assert!(s.cubic && !s.partial);
    let slab_top = full_top.map(|[x, _, z]| [x, 0.5, z]);
    assert!(!shape(&slab_top, UP, false).cubic && !shape(&slab_top, UP, false).partial);
    assert!(shape(&slab_top, UP, true).cubic);
    let small = shape(&slab_top.map(|[x, y, z]| [x * 0.5, y, z]), UP, false);
    assert!(small.partial && small.bounds[usize::from(EAST)] == 0.5);
    assert_eq!(facing(&full_top), UP);
}

/// Ids: 0 air, 1 stone. Stone at `stone`, sky 15 everywhere else.
fn lit(stone: &[[i32; 3]]) -> (Volume, BlockTable, LightCells) {
    let air = RenderBlock { shape: Shape::None, occludes: false, shades: false, light: LightFacts::default(), ..BlockTable::cube(0) };
    let mut table = BlockTable::from_blocks(vec![air, BlockTable::cube(1)]);
    table.lighting = Lighting::Java;
    let mut v = Volume::filled(0);
    let mut cells = LightCells(Box::new([data::pack(0, 15); data::SIDE * data::SIDE * data::SIDE]));
    for &[x, y, z] in stone {
        v.blocks[cell(x, y, z)] = 1;
        cells.0[data::cell(x, y, z)] = 0;
    }
    (v, table, cells)
}

#[test]
fn a_corner_between_two_blocks_is_darkest() {
    // A floor at y 4, two blocks on it meeting the top face of (5, 4, 5) at its x0 z0 corner.
    let mut stone: Vec<[i32; 3]> = (0..16).flat_map(|x| (0..16).map(move |z| [x, 4, z])).collect();
    stone.extend([[4, 5, 5], [5, 5, 4]]);
    let (v, table, cells) = lit(&stone);
    let ctx = Ctx { v: &v, table: &table, biomes: &BiomeColors::default(), cells: Some(&cells) };
    let light = cube_face(&ctx.lighter().unwrap(), [5, 4, 5], 2);
    assert!(light.iter().all(|w| w & LIT != 0) && light[0] & SPLIT_13 == 0);
    let grey = light.map(|w| w >> 16 & 0xFF);
    // Corners in u/v order: x0z0 sees both blocks, x1z0 and x0z1 one each.
    let (two, one) = ((0.2f32 + 0.2 + 1.0 + 1.0) * 0.25, (0.2f32 + 1.0 + 1.0 + 1.0) * 0.25);
    assert_eq!(grey, [grey_of(two), grey_of(one), grey_of(1.0), grey_of(one)]);
    let sky = light.map(|w| w >> 8 & 0xFF);
    // Dark samples take the centre's light: every corner sees full sky.
    assert_eq!(sky, [240; 4]);
}

fn grey_of(b: f32) -> u32 {
    scale(grey(b), 1.0)
}

#[test]
fn faces_with_different_corners_do_not_merge() {
    let mut stone: Vec<[i32; 3]> = (0..16).flat_map(|x| (0..16).map(move |z| [x, 4, z])).collect();
    stone.push([4, 5, 5]);
    let (v, table, cells) = lit(&stone);
    let mut out = SectionMesh::default();
    greedy::cubes(&Ctx { v: &v, table: &table, biomes: &BiomeColors::default(), cells: Some(&cells) }, &mut out);
    let tops: Vec<_> = out.solid.iter().filter(|q| q.face() == 2 && (q.0[0] >> 9 & 511) == 80).collect();
    // The floor's top: one quad per face beside the block, merged runs elsewhere.
    assert!(tops.iter().all(|q| q.1[0] & LIT != 0));
    let area: u32 = tops.iter().map(|q| ((q.0[1] & 255) + 1) * ((q.0[1] >> 8 & 255) + 1) / 256).sum();
    assert_eq!(area, 255);
    assert!(tops.len() > 4 && tops.len() < 40, "{}", tops.len());
}
