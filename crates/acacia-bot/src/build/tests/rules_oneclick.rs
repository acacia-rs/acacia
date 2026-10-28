//! The oneclick tier's rules against `place_probe SET=oneclick` (2026-10-28; docs/builder/oneclick.md).

use crate::build::rules::{gives, gravity, hangs, item, matches, post, supported};
use crate::build::{Block, Transform};
use crate::interact::Face;
use crate::pathfind::process::BlockView;

const STONE: BlockView = BlockView { name: "minecraft:stone", props: "", air: false, liquid: false, full: true, interactive: false };
/// The probe's target cell and its stands two blocks off on each side (eye level inside the cell's height).
const T: [i32; 3] = [5, 1, 0];
const N: [f32; 3] = [5.5, 1.62, -1.5];
const S: [f32; 3] = [5.5, 1.62, 2.5];
const W: [f32; 3] = [3.5, 1.62, 0.5];
const E: [f32; 3] = [7.5, 1.62, 0.5];

fn at(block: &str, props: &str, face: Face, y: f32, eye: [f32; 3]) -> bool {
    let o = face.offset();
    let click = [0.5 + 0.5 * o[0] as f32, if matches!(face, Face::Up) { 1.0 } else { y }, 0.5 + 0.5 * o[2] as f32];
    gives(&Block::new(block, props), face, click, eye, T, &STONE)
}

fn seen(name: &'static str, props: &'static str) -> BlockView {
    BlockView { name, props, full: false, ..STONE }
}

#[test]
fn trapdoors_face_the_player_and_flip_on_high_clicks() {
    let closed = |d: &str, top: u8| format!("direction={d},open_bit=0,upside_down_bit={top}");
    assert!(at("trapdoor", &closed("3", 0), Face::Up, 1.0, N));
    assert!(at("trapdoor", &closed("3", 1), Face::Down, 0.0, N));
    assert!(at("iron_trapdoor", &closed("2", 1), Face::East, 0.75, S), "side clicks: the look, not the face");
    assert!(at("spruce_trapdoor", &closed("1", 0), Face::South, 0.25, W));
    assert!(at("trapdoor", &closed("0", 0), Face::North, 0.25, E));
    assert!(!at("trapdoor", &closed("0", 0), Face::North, 0.75, E));
    assert_eq!(item(&Block::new("trapdoor", "direction=0,open_bit=1,upside_down_bit=0")), None, "open: the interact tier's");
    let t = Transform { turns: 1, ..Transform::default() };
    assert_eq!(t.block(&Block::new("trapdoor", "direction=3")).prop("direction"), Some("0"), "north turns east");
}

#[test]
fn gates_chests_and_anvils() {
    assert!(at("fence_gate", "in_wall_bit=0,minecraft:cardinal_direction=north,open_bit=0", Face::Up, 1.0, N));
    assert!(at("spruce_fence_gate", "in_wall_bit=1,minecraft:cardinal_direction=west,open_bit=0", Face::South, 0.25, W));
    assert!(at("chest", "minecraft:cardinal_direction=south", Face::Up, 1.0, S));
    assert!(at("trapped_chest", "minecraft:cardinal_direction=west", Face::Up, 1.0, W));
    assert!(at("anvil", "minecraft:cardinal_direction=east", Face::Up, 1.0, N), "a quarter turn off the furnace rule");
    assert!(at("damaged_anvil", "minecraft:cardinal_direction=north", Face::Up, 1.0, W));
    assert!(gravity("minecraft:anvil"));
    let gate = Block::new("fence_gate", "in_wall_bit=0,minecraft:cardinal_direction=north,open_bit=0");
    assert!(matches(&gate, &seen("minecraft:fence_gate", "in_wall_bit=1,minecraft:cardinal_direction=north,open_bit=0")), "in_wall_bit follows the walls");
}

#[test]
fn lanterns_hang_from_a_down_click_only() {
    assert!(at("lantern", "hanging=1", Face::Down, 0.0, N));
    assert!(!at("lantern", "hanging=1", Face::East, 0.75, N));
    assert!(at("soul_lantern", "hanging=0", Face::Up, 1.0, E));
    assert_eq!(hangs(&Block::new("lantern", "hanging=1")), Some(true));
    assert_eq!(hangs(&Block::new("lantern", "hanging=0")), None);
    assert_eq!(hangs(&Block::new("iron_chain", "pillar_axis=y")), Some(false), "a chain also stands on the block under it");
    assert!(post(&seen("minecraft:iron_chain", "pillar_axis=y")), "a chandelier's chain takes clicks at its ends");
}

#[test]
fn bells_and_grindstones_attach_to_the_clicked_face() {
    assert!(at("bell", "attachment=standing,direction=2,toggle_bit=0", Face::Up, 1.0, N), "on the floor: towards the player");
    assert!(at("bell", "attachment=hanging,direction=1,toggle_bit=0", Face::Down, 0.0, W));
    assert!(at("bell", "attachment=side,direction=2,toggle_bit=0", Face::North, 0.25, N), "on a wall: the clicked face");
    assert!(at("bell", "attachment=multiple,direction=3,toggle_bit=0", Face::East, 0.75, N));
    assert!(matches(&Block::new("bell", "attachment=side,direction=3"), &seen("minecraft:bell", "attachment=multiple,direction=3,toggle_bit=0")));
    assert!(at("grindstone", "attachment=side,direction=0", Face::South, 0.25, W));
    assert!(at("grindstone", "attachment=standing,direction=3", Face::Up, 1.0, E));
    assert!(at("loom", "direction=0", Face::Up, 1.0, S));
}

#[test]
fn rods_hoppers_and_chains() {
    assert!(at("hopper", "facing_direction=0,toggle_bit=0", Face::Up, 1.0, N));
    assert!(at("hopper", "facing_direction=0,toggle_bit=0", Face::Down, 0.0, N));
    assert!(at("hopper", "facing_direction=3,toggle_bit=0", Face::North, 0.25, N), "into the clicked block");
    assert!(at("end_rod", "facing_direction=1", Face::Up, 1.0, N));
    assert!(at("end_rod", "facing_direction=3", Face::North, 0.25, N), "BDS's backwards horizontal names");
    assert!(at("lightning_rod", "facing_direction=2,powered_bit=0", Face::North, 0.25, N));
    assert!(at("iron_chain", "pillar_axis=x", Face::East, 0.75, N));
}

#[test]
fn workstations_and_decorations() {
    assert!(at("barrel", "facing_direction=2,open_bit=0", Face::East, 0.25, N), "towards the player, as a dispenser");
    assert!(at("barrel", "facing_direction=1,open_bit=0", Face::Up, 1.0, [6.5, 3.62, 0.5]), "up from close above");
    assert!(at("campfire", "extinguished=0,minecraft:cardinal_direction=south", Face::Up, 1.0, N), "away from the player");
    assert!(at("white_glazed_terracotta", "facing_direction=4", Face::Up, 1.0, W));
    assert!(at("amethyst_cluster", "minecraft:block_face=west", Face::West, 0.25, N));
    assert!(at("beehive", "direction=3,honey_level=0", Face::Up, 1.0, E));
    assert!(at("chiseled_bookshelf", "books_stored=0,direction=2", Face::Up, 1.0, N));
    assert_eq!(item(&Block::new("beehive", "direction=3,honey_level=5")), None, "honey takes more than a click");
    assert!(matches(&Block::new("oak_leaves", "persistent_bit=0,update_bit=0"), &seen("minecraft:oak_leaves", "persistent_bit=1,update_bit=0")));
}

#[test]
fn denied_by_accident_no_more() {
    for n in ["redstone_block", "redstone_lamp", "redstone_torch", "sea_lantern", "lantern", "iron_chain", "stone_pressure_plate", "hopper", "dead_fire_coral_block", "campfire"] {
        assert!(supported(&format!("minecraft:{n}")), "{n}");
    }
    for n in ["redstone_wire", "repeater", "unpowered_comparator", "fire", "fire_coral_block", "bubble_coral_block"] {
        assert!(!supported(&format!("minecraft:{n}")), "{n}");
    }
    assert_eq!(item(&Block::new("chain_command_block", "conditional_bit=0,facing_direction=2")), None);
    assert_eq!(item(&Block::new("unlit_redstone_torch", "torch_facing_direction=north")).as_deref(), Some("minecraft:redstone_torch"));
    assert!(matches(&Block::new("lit_redstone_lamp", ""), &seen("minecraft:redstone_lamp", "")), "lit follows power");
}
