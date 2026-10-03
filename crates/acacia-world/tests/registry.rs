use acacia_world::{Aabb, BlockFlags, BlockRegistry, BlockState, CustomBlock, fnv1_64};

fn state(name: &str, props: &str) -> &'static BlockState {
    let r = BlockRegistry::vanilla();
    let id = r.find(name, props).unwrap_or_else(|| panic!("{name}[{props}] missing"));
    r.get(id).unwrap()
}

fn b(min: [f32; 3], max: [f32; 3]) -> Aabb {
    Aabb::new(min, max)
}

#[test]
fn palette_shape_and_order() {
    let r = BlockRegistry::vanilla();
    assert_eq!(r.len(), 22091);
    assert_eq!(r.air_id(), 17025);
    assert!(r.get(r.air_id()).unwrap().is_air());
    assert!(r.get(r.len() as u32).is_none());
    let names: Vec<&str> = (0..r.len() as u32).map(|i| r.get(i).unwrap().name).collect();
    for w in names.windows(2) {
        assert!(w[0] == w[1] || fnv1_64(w[0]) < fnv1_64(w[1]), "{} before {}", w[0], w[1]);
    }
}

#[test]
fn stone_and_basic_flags() {
    let stone = state("minecraft:stone", "");
    assert!(stone.is_full_cube() && stone.is_solid() && !stone.is_liquid());
    assert_eq!(stone.boxes, &[Aabb::FULL]);
    assert_eq!(stone.friction, 0.6);
    assert_eq!(state("minecraft:ice", "").friction, 0.98);
    assert_eq!(state("minecraft:blue_ice", "").friction, 0.989);
    assert_eq!(state("minecraft:slime", "").friction, 0.8);
    assert_eq!(state("minecraft:honey_block", "").jump_factor(), 0.6);
    assert!(state("minecraft:ladder", "facing_direction=2").is_climbable());
    assert!(state("minecraft:vine", "vine_direction_bits=1").is_climbable());
    assert_eq!(state("minecraft:web", "").stuck_multiplier(), Some([0.25, 0.05, 0.25]));
    assert!(!state("minecraft:web", "").is_solid());
    assert!(state("minecraft:bubble_column", "drag_down=1").flags.contains(BlockFlags::BUBBLE_DRAG));
    assert!(!state("minecraft:bubble_column", "drag_down=0").flags.contains(BlockFlags::BUBBLE_DRAG));
    assert_eq!(state("minecraft:soul_sand", "").boxes, &[b([0.0; 3], [1.0, 0.875, 1.0])]);
}

#[test]
fn fences_and_panes_connect_by_state() {
    let conn = |n, e, s, w| {
        format!("minecraft:connection_east={e},minecraft:connection_north={n},minecraft:connection_south={s},minecraft:connection_west={w}")
    };
    let post = state("minecraft:oak_fence", &conn(0, 0, 0, 0));
    assert_eq!(post.boxes, &[b([0.375, 0.0, 0.375], [0.625, 1.5, 0.625])]);
    let ne = state("minecraft:nether_brick_fence", &conn(1, 1, 0, 0));
    assert_eq!(ne.boxes.len(), 3);
    assert!(ne.boxes.contains(&b([0.375, 0.0, 0.0], [0.625, 1.5, 0.375])));
    assert!(ne.boxes.contains(&b([0.625, 0.0, 0.375], [1.0, 1.5, 0.625])));
    let pane = state("minecraft:glass_pane", &conn(0, 0, 1, 0));
    assert_eq!(pane.boxes, &[b([0.4375, 0.0, 0.5], [0.5625, 1.0, 1.0])]);
    assert!(state("minecraft:iron_bars", &conn(0, 0, 0, 0)).is_solid());
}

#[test]
fn liquids() {
    let source = state("minecraft:water", "liquid_depth=0");
    assert!(source.is_water() && source.is_liquid() && !source.is_solid());
    assert_eq!(source.fluid_height(), 8.0 / 9.0);
    let flowing = state("minecraft:flowing_water", "liquid_depth=3");
    assert_eq!((flowing.liquid_depth, flowing.fluid_height()), (3, 5.0 / 9.0));
    assert!(state("minecraft:lava", "liquid_depth=0").is_lava());
}

#[test]
fn light_emission_and_filter() {
    let light = |name, props| {
        let s = state(name, props);
        (s.light_emission, s.light_filter)
    };
    assert_eq!(light("minecraft:stone", ""), (0, 15));
    assert_eq!(light("minecraft:air", ""), (0, 0));
    assert_eq!(light("minecraft:glowstone", ""), (15, 15));
    assert_eq!(light("minecraft:torch", "torch_facing_direction=top"), (14, 0));
    assert_eq!(light("minecraft:lit_furnace", "minecraft:cardinal_direction=north").0, 13);
    assert_eq!(light("minecraft:unlit_redstone_torch", "torch_facing_direction=top").0, 0);
    assert_eq!(light("minecraft:light_block_7", ""), (7, 0));
    assert_eq!(light("minecraft:white_candle", "candles=2,lit=1").0, 9);
    assert_eq!(light("minecraft:white_candle", "candles=2,lit=0").0, 0);
    assert_eq!(light("minecraft:sea_pickle", "cluster_count=3,dead_bit=0").0, 15);
    assert_eq!(light("minecraft:sea_pickle", "cluster_count=3,dead_bit=1").0, 0);
    assert_eq!(light("minecraft:respawn_anchor", "respawn_anchor_charge=2").0, 7);
    assert_eq!(light("minecraft:waxed_weathered_copper_bulb", "lit=1,powered_bit=0").0, 8);
    assert_eq!(light("minecraft:campfire", "extinguished=1,minecraft:cardinal_direction=north").0, 0);
    assert_eq!(light("minecraft:cauldron", "cauldron_liquid=lava,fill_level=6"), (15, 2));
    assert_eq!(light("minecraft:water", "liquid_depth=0").1, 1);
    assert_eq!(light("minecraft:oak_leaves", "persistent_bit=0,update_bit=0").1, 2);
    assert_eq!(light("minecraft:oak_slab", "minecraft:vertical_half=bottom").1, 1);
    assert_eq!(light("minecraft:oak_double_slab", "minecraft:vertical_half=bottom").1, 15);
    assert_eq!(light("minecraft:waxed_double_cut_copper_slab", "minecraft:vertical_half=bottom").1, 15);
    assert_eq!(light("minecraft:glowingobsidian", ""), (12, 15));
    assert_eq!(light("minecraft:glass", "").1, 0);
}

#[test]
fn stairs_have_step_and_corner_boxes() {
    let east = state("minecraft:oak_stairs", "minecraft:corner=none,upside_down_bit=0,weirdo_direction=0");
    assert_eq!(east.boxes, &[b([0.0; 3], [1.0, 0.5, 1.0]), b([0.5, 0.5, 0.0], [1.0; 3])]);
    let top_north = state("minecraft:oak_stairs", "minecraft:corner=none,upside_down_bit=1,weirdo_direction=3");
    assert_eq!(top_north.boxes, &[b([0.0, 0.5, 0.0], [1.0; 3]), b([0.0; 3], [1.0, 0.5, 0.5])]);
    let outer = state("minecraft:stone_brick_stairs", "minecraft:corner=outer_left,upside_down_bit=0,weirdo_direction=3");
    assert_eq!(outer.boxes[1], b([0.0, 0.5, 0.0], [0.5, 1.0, 0.5]));
    let inner = state("minecraft:white_wool_stairs", "minecraft:corner=inner_right,upside_down_bit=0,weirdo_direction=3");
    assert_eq!(inner.boxes.len(), 3);
}

#[test]
fn bedrock_specific_shapes() {
    let r = BlockRegistry::vanilla();
    let thin = |a: &Aabb| (0..3).any(|i| (a.max[i] - a.min[i] - 0.1825).abs() < 1e-6);
    for name in ["minecraft:wooden_door", "minecraft:iron_door", "minecraft:poplar_door", "minecraft:trapdoor"] {
        let states: Vec<_> = r.states_of(name).collect();
        assert!(!states.is_empty(), "{name}");
        for (_, s) in &states {
            assert!(s.boxes.len() == 1 && thin(&s.boxes[0]), "{name}[{}] {:?}", s.properties, s.boxes);
        }
        let distinct: std::collections::HashSet<_> = states.iter().map(|(_, s)| format!("{:?}", s.boxes)).collect();
        assert!(distinct.len() >= 4, "{name} has orientation-dependent shapes");
    }
    let chest = state("minecraft:chest", "minecraft:cardinal_direction=north");
    assert_eq!(chest.boxes[0].min, [0.025, 0.0, 0.025]);
    assert!(state("minecraft:scaffolding", "stability=0,stability_check=0").flags.contains(BlockFlags::DYNAMIC_SHAPE));
    assert_eq!(state("minecraft:end_portal_frame", "end_portal_eye_bit=1,minecraft:cardinal_direction=north").boxes.len(), 1);
}

#[test]
fn custom_blocks_merge_in_hash_order() {
    let vanilla = BlockRegistry::vanilla();
    let custom = vanilla.with_custom_blocks(&[
        CustomBlock { name: "geyser_custom:thing".into(), state_count: 3 },
        CustomBlock { name: "minecraft:white_wool_stairs".into(), state_count: 40 },
    ]);
    assert_eq!(custom.len(), vanilla.len() + 3);
    let ids: Vec<u32> = custom.states_of("geyser_custom:thing").map(|(i, _)| i).collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[2] - ids[0], 2, "states of one block stay contiguous");
    let h = fnv1_64("geyser_custom:thing");
    let before = (0..vanilla.len() as u32).filter(|&i| fnv1_64(vanilla.get(i).unwrap().name) < h).count() as u32;
    assert_eq!(ids[0], before);
    let shifted = u32::from(fnv1_64("minecraft:air") > h) * 3;
    assert_eq!(custom.air_id(), vanilla.air_id() + shifted);
    assert!(custom.get(ids[0]).unwrap().is_full_cube());
}

#[test]
fn network_hashes_are_unique() {
    let r = BlockRegistry::vanilla();
    for id in [0, r.air_id(), r.find("minecraft:stone", "").unwrap(), r.len() as u32 - 1] {
        assert_eq!(r.runtime_id_from_hash(r.get(id).unwrap().network_hash), Some(id));
    }
    let hashes: std::collections::HashSet<u32> = (0..r.len() as u32).map(|i| r.get(i).unwrap().network_hash).collect();
    assert_eq!(hashes.len(), r.len());
}

#[test]
fn property_lookup() {
    let s = state("minecraft:oak_stairs", "weirdo_direction=2,upside_down_bit=1,minecraft:corner=none");
    assert_eq!(s.property("weirdo_direction"), Some("2"));
    assert_eq!(s.property("minecraft:corner"), Some("none"));
    assert_eq!(s.property("nope"), None);
}
