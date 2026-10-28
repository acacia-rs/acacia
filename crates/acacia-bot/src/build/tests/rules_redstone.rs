//! The redstone tier's rules against `place_probe SET=redstone` (BDS 1.26.52.3, 2026-10-28).

use std::collections::HashMap;

use acacia_physics::BlockPos;

use crate::build::builder::Cell;
use crate::build::rules::rails::{self, Rail, World};
use crate::build::rules::redstone::{self, ready, settle};
use crate::build::rules::{gives, item, matches, supported};
use crate::build::survey::Need;
use crate::build::Block;
use crate::interact::Face;
use crate::pathfind::process::{BlockView, Look, Target};

const C: BlockPos = [5, 0, 0];
const N: BlockPos = [5, 0, -1];
const S: BlockPos = [5, 0, 1];
const E: BlockPos = [6, 0, 0];
const W: BlockPos = [4, 0, 0];

/// Places plain (or straight-only) rails in order as BDS did; returns every rail's shape.
fn lay(order: &[BlockPos], straight: bool) -> HashMap<BlockPos, u8> {
    let none = |_: BlockPos| None::<Rail>;
    let mut w = World::new(&none);
    for p in order {
        w.place(*p, straight);
    }
    order.iter().map(|p| (*p, w.get(*p).expect("placed").1)).collect()
}

#[test]
fn rails_join_as_the_probe_measured() {
    assert_eq!(lay(&[C], false)[&C], 0, "a lone rail runs north-south whatever the facing");
    assert_eq!(lay(&[C, E], false), [(C, 1), (E, 1)].into());
    assert_eq!(lay(&[N, E, C], false), [(C, 9), (N, 0), (E, 1)].into(), "rail-curve-last");
    assert_eq!(lay(&[C, N, E], false), [(C, 9), (N, 0), (E, 1)].into(), "rail-curve-second");
    assert_eq!(lay(&[W, C, E, N], false), [(W, 1), (C, 1), (E, 1), (N, 0)].into(), "a full rail does not turn");
    assert_eq!(lay(&[W, C, N, E], false), [(W, 1), (C, 8), (N, 0), (E, 0)].into(), "rail-tee-open");
    assert_eq!(lay(&[N, E, W, C], false)[&C], 9, "the last fitting curve of NW, NE");
    assert_eq!(lay(&[E, S, W, C], false)[&C], 6);
    assert_eq!(lay(&[N, S, E, W, C], false)[&C], 6);
    assert_eq!(lay(&[W, C, [6, 0, -1], E], false)[&E], 8, "rail-steal");
    assert_eq!(lay(&[N, E, C], true), [(C, 1), (N, 0), (E, 1)].into(), "straight rails along both axes run east-west");
    assert_eq!(lay(&[C, N, E], true)[&C], 1);
}

#[test]
fn a_rail_rises_to_a_rail_one_up() {
    for order in [[C, [6, 1, 0]], [[6, 1, 0], C]] {
        assert_eq!(lay(&order, false), [(C, 2), ([6, 1, 0], 1)].into(), "{order:?}");
    }
}

#[test]
fn rail_shapes_turn_with_the_structure() {
    assert_eq!(rails::turned(7, 1, false), 8, "south-west turned clockwise is north-west");
    assert_eq!(rails::turned(2, 1, false), 5, "rising east turns to rising south");
    assert_eq!(rails::turned(2, 0, true), 3, "mirrored: rising west");
    assert_eq!(rails::turned(1, 3, false), 0);
    assert_eq!(redstone::turned_rail("9", 2, false), "7");
}

/// Blocks by cell, stone below y 0, air elsewhere.
struct Cells(HashMap<BlockPos, (&'static str, &'static str)>);

impl Look for Cells {
    fn solid(&self, p: BlockPos) -> Option<bool> {
        self.block(p).map(|b| b.solid())
    }

    fn find(&self, _: &Target) -> Option<acacia_physics::Vec3> {
        None
    }

    fn block(&self, p: BlockPos) -> Option<BlockView> {
        let (name, props) = self.0.get(&p).copied().unwrap_or(if p[1] < 0 { ("minecraft:stone", "") } else { ("minecraft:air", "") });
        let liquid = name.ends_with("water") || name.ends_with("lava");
        Some(BlockView { name, props, air: name == "minecraft:air", liquid, full: !liquid && name != "minecraft:air", interactive: false })
    }
}

fn rail(shape: u8) -> Block {
    Block::new("rail", &format!("rail_direction={shape}"))
}

#[test]
fn a_junction_waits_for_its_line() {
    // C runs east-west between W and E; N runs north-south into C's side.
    let targets: HashMap<BlockPos, Block> = [(W, rail(1)), (C, rail(1)), (E, rail(1)), (N, rail(0))].into();
    let t = |p: BlockPos| targets.get(&p).cloned();
    let empty = Cells(HashMap::new());
    assert!(ready(&rail(1), W, &empty, &t));
    assert!(!ready(&rail(1), C, &empty, &t), "C now would turn to N later");
    assert!(!ready(&rail(0), N, &empty, &t), "N first would make C curve");
    let line = Cells([(W, ("minecraft:rail", "rail_direction=1")), (E, ("minecraft:rail", "rail_direction=0"))].into());
    assert!(ready(&rail(1), C, &line, &t));
    assert!(!ready(&rail(0), N, &line, &t));
}

#[test]
fn a_rail_on_its_way_is_not_broken() {
    // W placed alone shows north-south; its target runs east-west towards C, still to come.
    let cells = vec![Cell { pos: W, target: rail(1) }, Cell { pos: C, target: rail(1) }];
    let t = |p: BlockPos| cells.iter().find(|c| c.pos == p).map(|c| c.target.clone());
    let look = Cells([(W, ("minecraft:rail", "rail_direction=0"))].into());
    let mut needs = vec![Need::Break, Need::Place];
    settle(&cells, &look, &t, &mut needs);
    assert_eq!(needs, [Need::Done, Need::Place]);
}

#[test]
fn flows_from_the_jobs_sources_are_left_alone() {
    let cells = vec![Cell { pos: C, target: Block::new("water", "liquid_depth=0") }, Cell { pos: E, target: Block::new("air", "") }, Cell { pos: W, target: Block::new("air", "") }];
    let t = |p: BlockPos| cells.iter().find(|c| c.pos == p).map(|c| c.target.clone());
    let look = Cells([(C, ("minecraft:water", "liquid_depth=0")), (E, ("minecraft:flowing_water", "liquid_depth=1")), (W, ("minecraft:lava", "liquid_depth=0"))].into());
    let mut needs = vec![Need::Done, Need::Drain, Need::Drain];
    settle(&cells, &look, &t, &mut needs);
    assert_eq!(needs, [Need::Done, Need::Done, Need::Drain], "a source where air belongs is still drained");
}

#[test]
fn supports_come_first() {
    let t = |p: BlockPos| (p == [5, -1, 0]).then(|| Block::new("farmland", "moisturized_amount=0"));
    let dirt = Cells([([5, -1, 0], ("minecraft:dirt", ""))].into());
    let tilled = Cells([([5, -1, 0], ("minecraft:farmland", "moisturized_amount=3"))].into());
    let wheat = Block::new("wheat", "growth=0");
    assert!(!ready(&wheat, C, &dirt, &t), "seeds wait for the tilling");
    assert!(ready(&wheat, C, &tilled, &t));
    let vine = Block::new("vine", "vine_direction_bits=8");
    assert!(!ready(&vine, C, &Cells(HashMap::new()), &|_| None));
    assert!(ready(&vine, C, &Cells([(E, ("minecraft:stone", ""))].into()), &|_| None));
}

#[test]
fn items_states_and_clicks() {
    let item_of = |n: &str, p: &str| item(&Block::new(n, p));
    assert_eq!(item_of("redstone_wire", "redstone_signal=7").as_deref(), Some("minecraft:redstone"));
    assert_eq!(item_of("reeds", "age=3").as_deref(), Some("minecraft:sugar_cane"));
    assert_eq!(item_of("potatoes", "growth=7").as_deref(), Some("minecraft:potato"));
    assert_eq!(item_of("farmland", "moisturized_amount=7").as_deref(), Some("minecraft:dirt"));
    assert_eq!(item_of("water", "liquid_depth=0").as_deref(), Some("minecraft:water_bucket"));
    assert_eq!(item_of("golden_rail", "rail_data_bit=1,rail_direction=1").as_deref(), Some("minecraft:golden_rail"));
    assert_eq!(item_of("vine", "vine_direction_bits=5"), None, "one click attaches one side");
    assert_eq!(item_of("seagrass", "sea_grass_type=double_top"), None);
    assert!(!supported("minecraft:bamboo") && supported("minecraft:bamboo_sapling") && supported("minecraft:bamboo_planks"));
    assert!(!supported("minecraft:redstone_torch") && !supported("minecraft:twisting_vines") && !supported("minecraft:waterlily"));
    let view = |name: &'static str, props: &'static str| BlockView { name, props, air: false, liquid: name.contains("water"), full: false, interactive: false };
    assert!(matches(&Block::new("water", "liquid_depth=3"), &view("minecraft:air", "")), "flows are not compared");
    assert!(matches(&Block::new("water", "liquid_depth=0"), &view("minecraft:flowing_water", "liquid_depth=0")));
    assert!(!matches(&Block::new("water", "liquid_depth=0"), &view("minecraft:flowing_water", "liquid_depth=2")));
    assert!(matches(&Block::new("wheat", "growth=0"), &view("minecraft:wheat", "growth=6")), "growth is not compared");
    let stone = view("minecraft:stone", "");
    let click = |b: &Block, f: Face| gives(b, f, [0.5, 0.5, 0.5], [5.5, 1.62, -1.5], C, &stone);
    assert!(click(&Block::new("vine", "vine_direction_bits=8"), Face::West), "a vine on a block's west face hangs east of it: 8");
    assert!(!click(&Block::new("vine", "vine_direction_bits=8"), Face::North));
    assert!(click(&Block::new("wheat", "growth=0"), Face::Up) && !click(&Block::new("wheat", "growth=0"), Face::East));
    assert!(click(&Block::new("water", "liquid_depth=0"), Face::East));
}
