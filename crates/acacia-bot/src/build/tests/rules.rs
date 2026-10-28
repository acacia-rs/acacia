//! The placement rules against what `examples/place_probe.rs` measured on strict BDS.

use crate::build::rules::{gives, item, matches, supported};
use crate::build::Block;
use crate::interact::Face;
use crate::pathfind::process::BlockView;

const STONE: BlockView = BlockView { name: "minecraft:stone", props: "", air: false, liquid: false, full: true, interactive: false };
/// The probe's target cell; the bot stood two blocks north of it (eye level inside the cell's height).
const T: [i32; 3] = [5, 1, 0];
const NORTH_EYE: [f32; 3] = [5.5, 1.62, -1.5];
const WEST_EYE: [f32; 3] = [3.5, 1.62, 0.5];

fn gives_at(block: &str, props: &str, face: Face, y: f32, eye: [f32; 3]) -> bool {
    let o = face.offset();
    let base = [0.5 + 0.5 * o[0] as f32, y, 0.5 + 0.5 * o[2] as f32];
    gives(&Block::new(block, props), face, base, eye, T, &STONE)
}

#[test]
fn stairs_face_where_the_player_looks_and_flip_on_high_clicks() {
    assert!(gives_at("oak_stairs", "minecraft:corner=none,upside_down_bit=0,weirdo_direction=2", Face::Up, 1.0, NORTH_EYE));
    assert!(gives_at("oak_stairs", "upside_down_bit=1,weirdo_direction=2", Face::North, 0.75, NORTH_EYE));
    assert!(!gives_at("oak_stairs", "upside_down_bit=1,weirdo_direction=2", Face::North, 0.25, NORTH_EYE));
    assert!(gives_at("oak_stairs", "upside_down_bit=0,weirdo_direction=0", Face::South, 0.25, WEST_EYE));
    assert!(!gives_at("oak_stairs", "upside_down_bit=0,weirdo_direction=3", Face::Up, 1.0, NORTH_EYE));
}

#[test]
fn halves_axes_and_attachments() {
    assert!(gives_at("oak_slab", "minecraft:vertical_half=top", Face::Down, 0.0, NORTH_EYE));
    assert!(gives_at("oak_slab", "minecraft:vertical_half=bottom", Face::East, 0.25, NORTH_EYE));
    assert!(gives_at("oak_log", "pillar_axis=x", Face::West, 0.25, NORTH_EYE));
    assert!(gives_at("oak_log", "pillar_axis=z", Face::North, 0.75, NORTH_EYE));
    assert!(gives_at("torch", "torch_facing_direction=west", Face::East, 0.25, NORTH_EYE));
    assert!(gives_at("torch", "torch_facing_direction=top", Face::Up, 1.0, NORTH_EYE));
    assert!(!gives_at("torch", "torch_facing_direction=south", Face::Down, 0.0, NORTH_EYE), "a ceiling click does not hang a torch");
    assert!(gives_at("ladder", "facing_direction=5", Face::East, 0.75, NORTH_EYE));
    assert!(!gives_at("ladder", "facing_direction=1", Face::Up, 1.0, NORTH_EYE));
    assert!(gives_at("stone_button", "button_pressed_bit=0,facing_direction=0", Face::Down, 0.0, NORTH_EYE));
}

#[test]
fn facing_blocks_from_the_north() {
    assert!(gives_at("furnace", "minecraft:cardinal_direction=north", Face::Up, 1.0, NORTH_EYE));
    assert!(gives_at("observer", "minecraft:facing_direction=south,powered_bit=0", Face::Up, 1.0, NORTH_EYE));
    assert!(gives_at("piston", "facing_direction=3", Face::Up, 1.0, NORTH_EYE));
    assert!(gives_at("dispenser", "facing_direction=2,triggered_bit=0", Face::Up, 1.0, NORTH_EYE));
    assert!(gives_at("dispenser", "facing_direction=4,triggered_bit=0", Face::Up, 1.0, WEST_EYE));
}

#[test]
fn six_way_blocks_turn_vertical_from_close_above_or_below() {
    let (high, low, high_far) = ([6.5, 3.62, 0.5], [6.5, 0.62, 0.5], [7.4, 3.62, 0.5]);
    assert!(gives_at("dispenser", "facing_direction=1", Face::East, 0.25, high));
    assert!(gives_at("observer", "minecraft:facing_direction=down", Face::East, 0.25, high));
    assert!(gives_at("piston", "facing_direction=1", Face::East, 0.25, high), "pistons face the player vertically");
    assert!(gives_at("piston", "facing_direction=0", Face::Down, 0.0, low));
    assert!(gives_at("observer", "minecraft:facing_direction=up", Face::Down, 0.0, low));
    assert!(!gives_at("dispenser", "facing_direction=1", Face::Up, 1.0, [6.5, 3.3, 2.0]), "1.5 off: not measured, avoided");
    assert!(gives_at("dispenser", "facing_direction=5", Face::Up, 1.0, high_far));
}

#[test]
fn derived_states_are_not_compared() {
    let wall = Block::new("cobblestone_wall", "wall_connection_type_east=tall,wall_connection_type_north=none,wall_post_bit=1");
    let seen = BlockView { name: "minecraft:cobblestone_wall", props: "wall_connection_type_east=none,wall_connection_type_north=short,wall_post_bit=0", ..STONE };
    assert!(matches(&wall, &seen));
    let double = Block::new("oak_double_slab", "minecraft:vertical_half=top");
    assert_eq!(item(&double).as_deref(), Some("minecraft:oak_slab"));
    assert!(matches(&double, &BlockView { name: "minecraft:oak_double_slab", props: "minecraft:vertical_half=bottom", ..STONE }));
}

#[test]
fn outside_the_common_set() {
    assert!(!supported("minecraft:scaffolding"));
    assert!(!supported("minecraft:redstone_wire"));
    assert!(!supported("minecraft:water"));
    assert!(supported("minecraft:torch"));
    assert!(supported("minecraft:glass_pane"));
    assert_eq!(item(&Block::new("cake", "bite_counter=0")), None, "unknown state");
}
