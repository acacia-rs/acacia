//! Asset loading and meshing without a GPU. Needs assets/vanilla (tools/fetch-vanilla-pack.sh);
//! tests skip when it is missing.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use acacia_proto::nbt::Value;
use acacia_proto::packets::{LevelChunk, StartGame};
use acacia_proto::{Packet, RawPacket};
use acacia_render::assets::Pack;
use acacia_render::assets::image::{Alpha, Texture};
use acacia_render::biome::{BiomeColors, BiomeDef};
use acacia_render::blocks::{BlockTable, Layer, Material, Shape, Tint};
use acacia_render::light::{LightEvent, Lighting};
use acacia_render::mesh::{Volume, mesh_section};
use acacia_world::{BlockIds, BlockRegistry, ChunkChange, ChunkView, CustomBlock, World};
use bytes::Bytes;

fn pack() -> Option<Pack> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/vanilla");
    if !dir.is_dir() {
        eprintln!("skipped: {} missing (run tools/fetch-vanilla-pack.sh)", dir.display());
        return None;
    }
    Some(Pack::load(&dir).unwrap())
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../acacia-world/tests/fixtures/geyser").join(name)
}

fn packet<T: Packet>(path: PathBuf) -> T {
    RawPacket::parse(Bytes::from(std::fs::read(&path).unwrap())).unwrap().decode().unwrap()
}

fn geyser_view() -> ChunkView {
    let sg: StartGame = packet(fixture("start_game.bin"));
    let enum_len = |p: &Value| match p.get("enum") {
        Some(Value::List(e)) => e.items.len().max(1) as u32,
        _ => 1,
    };
    let custom: Vec<CustomBlock> = sg
        .block_properties
        .iter()
        .map(|b| CustomBlock {
            name: b.name.clone(),
            state_count: match b.state.value.get("properties") {
                Some(Value::List(l)) => l.items.iter().map(enum_len).product(),
                _ => 1,
            },
        })
        .collect();
    let world = World::new(Arc::new(BlockRegistry::vanilla().with_custom_blocks(&custom)), 0, BlockIds::Runtime);
    let mut view = ChunkView::new(world);
    let mut paths: Vec<_> = std::fs::read_dir(fixture(""))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_str().unwrap().starts_with("level_chunk"))
        .collect();
    paths.sort();
    for c in paths.into_iter().map(packet::<LevelChunk>).filter(|c| c.sub_chunk_count > 0) {
        view.insert_level_chunk(c.x, c.z, c.sub_chunk_count, &c.payload).unwrap();
    }
    view
}

#[test]
fn vanilla_blocks_resolve_to_textures_and_shapes() {
    let Some(pack) = pack() else { return };
    let registry = BlockRegistry::vanilla();
    let (table, textures, report) = BlockTable::build(registry, &pack);
    eprintln!("{} layers, {} animated, {} untextured blocks", textures.layers.len(), textures.animations.len(), report.blocks_without_textures.len());
    assert!(report.missing_images.is_empty(), "{:?}", report.missing_images);
    let get = |name: &str, props: &str| table.get(registry.find(name, props).unwrap_or_else(|| panic!("{name} {props}")));

    let stone = get("minecraft:stone", "");
    assert!(stone.occludes && stone.textures[0] != 0);
    let grass = get("minecraft:grass_block", "");
    assert_eq!(grass.tint[2], Tint::Grass);
    assert_eq!(grass.material[0], Material::Overlay, "grass side tints through its overlay alpha");
    assert_ne!(grass.textures[2], grass.textures[3]);
    let log_y = get("minecraft:oak_log", "pillar_axis=y");
    let log_x = get("minecraft:oak_log", "pillar_axis=x");
    assert_ne!(log_y.textures[2], log_y.textures[4], "top differs from side");
    assert_eq!(log_x.textures[0], log_y.textures[2], "x axis log shows its top facing east");
    assert_eq!(get("minecraft:water", "liquid_depth=0").layer, Layer::Translucent);
    assert_eq!(get("minecraft:short_grass", "").shape, Shape::Cross);
    let first = |name: &str| table.get(registry.states_of(name).next().unwrap().0);
    assert!(matches!(first("minecraft:oak_stairs").shape, Shape::Boxes(_)));
    assert_eq!(first("minecraft:oak_leaves").material[0], Material::Cutout);
    assert_ne!(first("minecraft:sea_lantern").textures[0], 0);
    let water = first("minecraft:water").textures[2];
    assert!(textures.animations.iter().any(|a| a.layer == water), "still water animates");
    // Education Edition blocks (elements, hard glass, lab tables) aren't in the vanilla pack.
    let untextured: Vec<_> = report
        .blocks_without_textures
        .iter()
        .filter(|n| !["element_", "hard_", "colored_torch", "chemical", "compound", "lab_table", "material_reducer", "underwater_"].iter().any(|e| n.contains(e)))
        .collect();
    assert!(untextured.len() < 15, "{untextured:?}");
}

/// Pins the overlay convention the shader assumes: grass side alpha is the tint mask.
#[test]
fn grass_side_overlay_alpha_marks_the_tinted_strip() {
    let Some(pack) = pack() else { return };
    let tex = pack.texture("grass_side").unwrap();
    let image = Texture::load(&pack.image_file(&tex.path).unwrap(), tex.quad).unwrap();
    let alpha_row = |y: usize| image.rgba[y * 64..y * 64 + 64].chunks(4).map(|p| u32::from(p[3])).sum::<u32>() / 16;
    assert_eq!((alpha_row(0), alpha_row(15)), (255, 0), "grass strip on top, dirt below");
    assert_ne!(image.alpha(), Alpha::Opaque);
}

#[test]
fn biome_colors_follow_colormaps_and_exceptions() {
    let Some(pack) = pack() else { return };
    let def = |id, name: &str, temperature, downfall| BiomeDef { id, name: name.into(), temperature, downfall };
    let colors = BiomeColors::build(&[def(1, "plains", 0.8, 0.4), def(2, "desert", 2.0, 0.0), def(6, "swampland", 0.8, 0.9)], &pack);
    let (plains, desert) = (colors.get(1), colors.get(2));
    eprintln!("plains {plains:?}\ndesert {desert:?}");
    assert!(plains.grass[1] > plains.grass[0] && plains.grass[1] > plains.grass[2], "plains grass is green");
    assert!(desert.grass[0] > plains.grass[0], "desert grass is yellower");
    assert_eq!(colors.get(6).grass, [0x6A, 0x70, 0x39], "swamp override");
    assert_eq!(colors.get(999), colors.get(12345), "unknown ids share the plains default");
}

/// Version-9 section of one block, `id` zigzag-varint encoded.
fn uniform_section(section_y: i8, id: u32) -> Vec<u8> {
    let mut out = vec![9, 1, section_y as u8, 1];
    let mut v = id << 1;
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
    out
}

#[test]
fn water_draws_no_faces_between_sections() {
    let Some(pack) = pack() else { return };
    let registry = BlockRegistry::vanilla_arc();
    let water = registry.find("minecraft:water", "liquid_depth=0").unwrap();
    let mut view = ChunkView::new(World::new(registry.clone(), 0, BlockIds::Runtime));
    for (x, z) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
        let mut payload = uniform_section(2, water);
        payload.extend(uniform_section(3, water));
        view.insert_level_chunk(x, z, 2, &payload).unwrap();
    }
    let world = view.world().clone();
    let (table, _, _) = BlockTable::build(&registry, &pack);
    let mesh = mesh_section(&Volume::gather(&world, 0, 3, 0).unwrap(), &table, &BiomeColors::default());
    let faces: Vec<u32> = mesh.translucent.iter().map(|q| q.face()).collect();
    let per_face: Vec<usize> = (0..6).map(|f| faces.iter().filter(|&&x| x == f).count()).collect();
    assert_eq!(per_face, [0, 0, 256, 256, 0, 0], "only the top surface and its underside");
}

#[test]
fn flowing_water_slopes_and_turns_downstream() {
    let Some(pack) = pack() else { return };
    let registry = BlockRegistry::vanilla_arc();
    let mut view = ChunkView::new(World::new(registry.clone(), 0, BlockIds::Runtime));
    view.insert_level_chunk(0, 0, 1, &uniform_section(2, registry.find("minecraft:air", "").unwrap())).unwrap();
    view.set_block(4, 40, 4, 0, registry.find("minecraft:water", "liquid_depth=0").unwrap());
    for (x, depth) in [(5, "liquid_depth=1"), (6, "liquid_depth=2")] {
        view.set_block(x, 40, 4, 0, registry.find("minecraft:flowing_water", depth).unwrap());
    }
    let (table, _, _) = BlockTable::build(&registry, &pack);
    let mesh = mesh_section(&Volume::gather(view.world(), 0, 2, 0).unwrap(), &table, &BiomeColors::default());
    let top = mesh.translucent.iter().find(|q| q.0[0] >> 27 == 12 && q.0[0] & 511 == 5 * 16).expect("a surface at x = 5");
    let [west, east, east2, west2] = [0, 4, 8, 12].map(|shift| (top.0[1] >> shift) & 15);
    assert!(west > east && (west, east) == (west2, east2), "lower downstream: {west} {east}");
    assert_eq!(top.0[2] & 255, 192, "flowing east is three quarters of a turn");
}

#[test]
fn translucent_quads_blend_far_plane_first_per_direction() {
    let Some(pack) = pack() else { return };
    let registry = BlockRegistry::vanilla_arc();
    let water = registry.find("minecraft:water", "liquid_depth=0").unwrap();
    let ice = registry.find("minecraft:ice", "").unwrap();
    let mut view = ChunkView::new(World::new(registry.clone(), 0, BlockIds::Runtime));
    view.insert_level_chunk(0, 0, 1, &uniform_section(2, water)).unwrap();
    for (x, z) in [(3, 3), (3, 4)] {
        view.set_block(x, 47, z, 0, ice);
    }
    let (table, _, _) = BlockTable::build(&registry, &pack);
    let mesh = mesh_section(&Volume::gather(view.world(), 0, 2, 0).unwrap(), &table, &BiomeColors::default());
    let heights = |face: u32| -> Vec<u32> {
        mesh.translucent.iter().filter(|q| q.face() == face).map(|q| (q.0[0] >> 9) & 511).collect()
    };
    let (up, down) = (heights(2), heights(3));
    assert!(up.is_sorted() && up.first() < up.last(), "water under the ice, then the ice top: {up:?}");
    assert!(down.is_sorted_by(|a, b| a >= b) && down.contains(&240), "the water surface has an underside: {down:?}");
}

#[test]
fn water_faces_wait_for_unknown_neighbour_sections_but_not_known_air() {
    let Some(pack) = pack() else { return };
    let registry = BlockRegistry::vanilla_arc();
    let water = registry.find("minecraft:water", "liquid_depth=0").unwrap();
    let mut view = ChunkView::new(World::new(registry.clone(), 0, BlockIds::Runtime));
    for (x, z) in [(0, 0), (-1, 0), (0, 1), (0, -1)] {
        let mut payload = uniform_section(2, water);
        payload.extend(uniform_section(3, water));
        view.insert_level_chunk(x, z, 2, &payload).unwrap();
    }
    // Request mode: the +x neighbour exists, but none of its sub-chunks have arrived.
    view.insert_biomes(1, 0, &[0xff]);
    let world = view.world().clone();
    let (table, _, _) = BlockTable::build(&registry, &pack);
    let east_faces = |world: &World| {
        let mesh = mesh_section(&Volume::gather(world, 0, 3, 0).unwrap(), &table, &BiomeColors::default());
        mesh.translucent.iter().filter(|q| q.face() == 0).count()
    };
    assert_eq!(east_faces(&world), 0, "unknown neighbour section hides the wall");
    view.insert_sub_chunk_air(1, 3, 0);
    assert_eq!(east_faces(&world), 256, "known air next door shows the whole 16×16 wall");
}

#[test]
fn geyser_chunks_light_on_the_light_thread() {
    let view = geyser_view();
    let world = view.world().clone();
    let columns = world.chunk_positions();
    let start = Instant::now();
    let lighting = Lighting::new(world.clone());
    let (mut lit, mut touched) = (0, 0);
    // A batch's world changes come before its light changes; stop once both go quiet.
    let mut wait = Duration::from_secs(10);
    while let Ok(event) = lighting.events.recv_timeout(wait) {
        match event {
            LightEvent::World(ChunkChange::Column { .. }) => lit += 1,
            LightEvent::World(_) => {}
            LightEvent::Light(_) => touched += 1,
        }
        if lit == columns.len() {
            wait = Duration::from_millis(200);
        }
    }
    assert_eq!(lit, columns.len());
    assert!(touched >= lit * 24, "every section of a lit column is touched");
    eprintln!("{lit} columns lit in {:?}, {touched} sections touched", start.elapsed());
    let data = lighting.data.read();
    let (x, z) = (columns[0].0 * 16 + 8, columns[0].1 * 16 + 8);
    let sky = |y| data.light(x, y, z).unwrap() & 15;
    assert_eq!(sky(300), 15);
    let floor = (-64..300).rev().find(|&y| sky(y) < 15).unwrap();
    eprintln!("first sky light below 15 at y {floor}");
    assert!(floor < 0, "superflat surface is below y 0");
}

#[test]
fn geyser_chunks_mesh() {
    let Some(pack) = pack() else { return };
    let view = geyser_view();
    let world = view.world().clone();
    let biome = view.chunk(world.chunk_positions()[0].0, world.chunk_positions()[0].1).unwrap().read().biome(0, -60, 0);
    assert!(biome.is_some(), "full chunks carry biomes after their sections");
    let (table, _, _) = BlockTable::build(world.registry(), &pack);
    let (mut sections, mut quads, mut translucent) = (0, 0, 0);
    let start = Instant::now();
    for (cx, cz) in world.chunk_positions() {
        for sy in -4..20 {
            let v = Volume::gather(&world, cx, sy, cz).unwrap();
            let mesh = mesh_section(&v, &table, &BiomeColors::default());
            sections += 1;
            quads += mesh.solid.len();
            translucent += mesh.translucent.len();
        }
    }
    let elapsed = start.elapsed();
    eprintln!("{sections} sections, {quads} solid + {translucent} translucent quads in {elapsed:?} ({:?}/section)", elapsed / sections);
    assert!(quads > 100, "superflat terrain has a surface");
}
