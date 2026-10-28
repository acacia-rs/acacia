//! The interact tier's rules against what `place_probe SET=interact` and `interact_probe` measured (2026-10-28).

use crate::build::rules::{completes, front, gives, item, items_per_cell, matches, uses};
use crate::build::survey::{need, Need};
use crate::build::{Block, SignText, Structure, Transform};
use crate::interact::Face;
use crate::pathfind::process::BlockView;

const STONE: BlockView = BlockView { name: "minecraft:stone", props: "", air: false, liquid: false, full: true, interactive: false };
const T: [i32; 3] = [5, 1, 0];
const NORTH_EYE: [f32; 3] = [5.5, 1.62, -1.5];
const EAST_EYE: [f32; 3] = [7.5, 1.62, 0.5];

fn gives_at(block: &str, props: &str, face: Face, y: f32, eye: [f32; 3]) -> bool {
    let o = face.offset();
    let base = [0.5 + 0.5 * o[0] as f32, y, 0.5 + 0.5 * o[2] as f32];
    gives(&Block::new(block, props), face, base, eye, T, &STONE)
}

fn view(name: &'static str, props: &'static str) -> BlockView {
    BlockView { name, props, full: false, interactive: true, ..STONE }
}

#[test]
fn levers_follow_the_face_and_on_floors_the_look() {
    assert!(gives_at("lever", "lever_direction=up_north_south,open_bit=1", Face::Up, 1.0, NORTH_EYE));
    assert!(gives_at("lever", "lever_direction=up_east_west,open_bit=0", Face::Up, 1.0, EAST_EYE));
    assert!(gives_at("lever", "lever_direction=down_east_west,open_bit=0", Face::Down, 0.0, EAST_EYE));
    assert!(gives_at("lever", "lever_direction=east,open_bit=0", Face::East, 0.25, NORTH_EYE));
    assert!(!gives_at("lever", "lever_direction=west,open_bit=0", Face::East, 0.25, NORTH_EYE));
}

#[test]
fn repeaters_and_comparators_face_the_player_and_take_their_item() {
    assert!(gives_at("unpowered_repeater", "minecraft:cardinal_direction=north,repeater_delay=3", Face::Up, 1.0, NORTH_EYE));
    assert!(gives_at("unpowered_comparator", "minecraft:cardinal_direction=east,output_lit_bit=0,output_subtract_bit=1", Face::Up, 1.0, EAST_EYE));
    assert_eq!(item(&Block::new("powered_repeater", "minecraft:cardinal_direction=north,repeater_delay=0")).as_deref(), Some("minecraft:repeater"));
    assert_eq!(item(&Block::new("daylight_detector_inverted", "redstone_signal=7")).as_deref(), Some("minecraft:daylight_detector"));
    let powered = BlockView { name: "minecraft:powered_comparator", props: "minecraft:cardinal_direction=east,output_lit_bit=1,output_subtract_bit=0", ..STONE };
    assert!(matches(&Block::new("unpowered_comparator", "minecraft:cardinal_direction=east,output_lit_bit=0,output_subtract_bit=0"), &powered), "power is derived");
}

#[test]
fn use_clicks_count_toggles_and_cycles() {
    let repeater = Block::new("unpowered_repeater", "minecraft:cardinal_direction=north,repeater_delay=2");
    let seen = view("minecraft:unpowered_repeater", "minecraft:cardinal_direction=north,repeater_delay=3");
    assert_eq!(uses(&repeater, &seen), Some((3, None)), "3 -> 0 -> 1 -> 2");
    assert_eq!(need(&repeater, Some(seen), true), Need::Adjust, "fill-only too");
    let turned = view("minecraft:unpowered_repeater", "minecraft:cardinal_direction=south,repeater_delay=2");
    assert_eq!(uses(&repeater, &turned), None, "a direction is no use-click");
    assert_eq!(need(&repeater, Some(turned), false), Need::Break);
    let lever = Block::new("lever", "lever_direction=north,open_bit=1");
    assert_eq!(uses(&lever, &view("minecraft:lever", "lever_direction=north,open_bit=0")), Some((1, None)));
    let inverted = Block::new("daylight_detector_inverted", "redstone_signal=0");
    assert_eq!(uses(&inverted, &view("minecraft:daylight_detector", "redstone_signal=12")), Some((1, None)));
    let subtract = Block::new("unpowered_comparator", "minecraft:cardinal_direction=east,output_lit_bit=0,output_subtract_bit=1");
    assert_eq!(uses(&subtract, &view("minecraft:powered_comparator", "minecraft:cardinal_direction=east,output_lit_bit=1,output_subtract_bit=0")), Some((1, None)));
    let wall = view("minecraft:wall_sign", "facing_direction=2");
    assert_eq!(uses(&Block::new("standing_sign", "ground_sign_direction=8"), &wall), None, "one item, another block: broken");
}

#[test]
fn counts_take_placements_into_the_cell_or_clicks_on_it() {
    let candles = Block::new("white_candle", "candles=2,lit=0");
    assert_eq!(items_per_cell(&candles), 3);
    assert!(completes(&candles, &view("minecraft:white_candle", "candles=0,lit=0")));
    assert!(!completes(&candles, &view("minecraft:white_candle", "candles=3,lit=0")));
    assert_eq!(item(&Block::new("white_candle", "candles=0,lit=1")), None, "lit candles need flint and steel");
    assert_eq!(item(&Block::new("candle_cake", "lit=0")), None);
    let pickles = Block::new("sea_pickle", "cluster_count=3,dead_bit=0");
    let one = view("minecraft:sea_pickle", "cluster_count=0,dead_bit=1");
    assert!(!completes(&pickles, &one), "BDS refuses a pickle placed into a pickle's cell");
    assert_eq!(uses(&pickles, &one), Some((3, Some("minecraft:sea_pickle".into()))));
    let eggs = Block::new("turtle_egg", "cracked_state=slightly_cracked,turtle_egg_count=two_egg");
    assert_eq!(uses(&eggs, &view("minecraft:turtle_egg", "cracked_state=no_cracks,turtle_egg_count=one_egg")), Some((1, Some("minecraft:turtle_egg".into()))));
}

#[test]
fn signs_stand_on_floors_hang_on_walls_and_turn_with_the_look() {
    assert!(gives_at("standing_sign", "ground_sign_direction=8", Face::Up, 1.0, NORTH_EYE), "looking south: 8");
    assert!(gives_at("standing_sign", "ground_sign_direction=12", Face::Up, 1.0, EAST_EYE), "looking west: 12");
    assert!(!gives_at("standing_sign", "ground_sign_direction=8", Face::North, 0.25, NORTH_EYE));
    assert!(gives_at("wall_sign", "facing_direction=2", Face::North, 0.75, NORTH_EYE));
    assert!(!gives_at("wall_sign", "facing_direction=2", Face::Up, 1.0, NORTH_EYE));
    assert_eq!(item(&Block::new("darkoak_wall_sign", "facing_direction=2")).as_deref(), Some("minecraft:dark_oak_sign"));
    let hanging = "attached_bit=0,ground_sign_direction=0";
    assert!(gives_at("oak_hanging_sign", &format!("{hanging},facing_direction=2,hanging=1"), Face::Down, 0.0, NORTH_EYE));
    assert!(gives_at("oak_hanging_sign", &format!("{hanging},facing_direction=5,hanging=0"), Face::North, 0.25, [6.5, 1.62, -1.5]), "along the wall, towards the player");
    assert!(!gives_at("oak_hanging_sign", &format!("{hanging},facing_direction=5,hanging=0"), Face::North, 0.25, NORTH_EYE), "straight on: BDS's tie");
    assert!(gives_at("oak_hanging_sign", &format!("{hanging},facing_direction=2,hanging=0"), Face::East, 0.25, NORTH_EYE));
    assert_eq!(item(&Block::new("oak_hanging_sign", "attached_bit=1,facing_direction=2,ground_sign_direction=4,hanging=1")), None, "attached: not measured");
}

#[test]
fn the_server_opens_the_side_the_player_stands_on() {
    let standing = view("minecraft:standing_sign", "ground_sign_direction=0");
    assert_eq!(front(&standing, [0, 0, 0], [0.5, 1.6, 2.5]), Some(true), "rotation 0 faces south");
    assert_eq!(front(&standing, [0, 0, 0], [0.5, 1.6, -1.5]), Some(false));
    assert_eq!(front(&standing, [0, 0, 0], [2.5, 1.6, 0.5]), None, "beside it");
    assert_eq!(front(&view("minecraft:wall_sign", "facing_direction=4"), [0, 0, 0], [-1.0, 1.6, 0.5]), Some(true));
}

#[test]
fn a_door_upper_half_open_bit_is_not_compared() {
    let upper = Block::new("copper_door", "door_hinge_bit=0,minecraft:cardinal_direction=south,open_bit=1,upper_block_bit=1");
    assert!(matches(&upper, &view("minecraft:copper_door", "door_hinge_bit=0,minecraft:cardinal_direction=south,open_bit=0,upper_block_bit=1")));
}

#[test]
fn sign_rotation_and_lever_axes_turn_with_the_structure() {
    let quarter = Transform { turns: 1, ..Transform::default() };
    assert_eq!(quarter.block(&Block::new("standing_sign", "ground_sign_direction=1")).prop("ground_sign_direction"), Some("5"));
    let mirror = Transform { mirror: true, ..Transform::default() };
    assert_eq!(mirror.block(&Block::new("standing_sign", "ground_sign_direction=4")).prop("ground_sign_direction"), Some("12"));
    assert_eq!(quarter.block(&Block::new("lever", "lever_direction=up_north_south,open_bit=0")).prop("lever_direction"), Some("up_east_west"));
    assert_eq!(quarter.block(&Block::new("lever", "lever_direction=north,open_bit=0")).prop("lever_direction"), Some("east"));
}

#[test]
fn sign_text_round_trips_through_mcstructure_bytes() {
    let mut s = Structure::new([1, 1, 2]);
    s.set([0, 0, 1], Some(Block::new("standing_sign", "ground_sign_direction=0")));
    s.set_text([0, 0, 1], Some(SignText { front: "a\nb".into(), back: "c".into() }));
    let back = Structure::load(&s.to_bytes()).expect("loads");
    assert_eq!(back.text([0, 0, 1]), Some(&SignText { front: "a\nb".into(), back: "c".into() }));
    assert_eq!(back.text([0, 0, 0]), None);
}
