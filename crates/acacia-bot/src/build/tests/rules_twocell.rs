//! Doors, beds and tall plants against what `place_probe SET=twocell` measured (2026-10-28), and the pair rules
//! of the survey.

use std::collections::HashMap;

use acacia_physics::{BlockPos, Vec3};
use acacia_world::BlockRegistry;

use crate::build::builder::Cell;
use crate::build::pairs::settle;
use crate::build::rules::twocell::{hinge_now, partner};
use crate::build::rules::{gives, item, matches, supported};
use crate::build::survey::Need;
use crate::build::{Block, Transform};
use crate::interact::Face;
use crate::pathfind::process::{BlockView, Look, Target};

const STONE: BlockView = BlockView { name: "minecraft:stone", props: "", air: false, liquid: false, full: true, interactive: false };
/// The probe's target cell on the floor; stands two blocks off it, feet level with it.
const T: BlockPos = [5, 0, 0];
const EYES: [(&str, Vec3); 4] = [("N", [5.5, 1.62, -1.5]), ("S", [5.5, 1.62, 2.5]), ("W", [3.5, 1.62, 0.5]), ("E", [7.5, 1.62, 0.5])];

fn door(facing: &str, hinge: u8, upper: u8) -> Block {
    Block::new("wooden_door", &format!("door_hinge_bit={hinge},minecraft:cardinal_direction={facing},open_bit=0,upper_block_bit={upper}"))
}

fn up(target: &Block, eye: Vec3) -> bool {
    gives(target, Face::Up, [0.5, 1.0, 0.5], eye, T, &STONE)
}

#[test]
fn doors_face_the_look_turned_clockwise_from_the_top_face_only() {
    for ((_, eye), facing) in EYES.iter().zip(["west", "east", "south", "north"]) {
        assert!(up(&door(facing, 0, 0), *eye), "{facing}");
    }
    assert!(!up(&door("north", 0, 0), EYES[0].1));
    assert!(!gives(&door("west", 0, 0), Face::North, [0.5, 0.25, 0.0], EYES[0].1, T, &STONE), "side clicks were refused");
}

#[test]
fn beds_face_the_look_and_plants_take_side_clicks() {
    for ((_, eye), d) in EYES.iter().zip(["0", "2", "3", "1"]) {
        assert!(up(&Block::new("bed", &format!("direction={d},head_piece_bit=0,occupied_bit=0")), *eye), "{d}");
    }
    let sunflower = Block::new("sunflower", "upper_block_bit=0");
    assert!(up(&sunflower, EYES[0].1));
    assert!(gives(&sunflower, Face::North, [0.5, 0.25, 0.0], EYES[0].1, T, &STONE));
    assert!(!gives(&sunflower, Face::Down, [0.5, 0.0, 0.5], EYES[0].1, T, &STONE));
    let dripleaf = |d: &str| Block::new("small_dripleaf_block", &format!("minecraft:cardinal_direction={d},upper_block_bit=0"));
    assert!(up(&dripleaf("north"), EYES[0].1), "faces the player");
    assert!(!up(&dripleaf("south"), EYES[0].1));
}

#[test]
fn pairs_are_placed_from_the_lower_half_and_the_foot() {
    assert_eq!(partner(&door("east", 0, 0), T), Some(([5, 1, 0], true)));
    assert_eq!(partner(&door("east", 1, 1), [5, 1, 0]), Some((T, false)));
    let foot = Block::new("bed", "direction=2,head_piece_bit=0,occupied_bit=0");
    assert_eq!(partner(&foot, T), Some(([5, 0, -1], true)));
    assert_eq!(partner(&Block::new("bed", "direction=2,head_piece_bit=1,occupied_bit=0"), [5, 0, -1]), Some((T, false)));
    assert_eq!(partner(&Block::new("stone", ""), T), None);
}

#[test]
fn items_and_compared_states() {
    assert_eq!(item(&door("east", 0, 0)).as_deref(), Some("minecraft:wooden_door"));
    assert_eq!(item(&Block::new("bed", "direction=0,head_piece_bit=0,occupied_bit=0")).as_deref(), Some("minecraft:bed"));
    assert_eq!(item(&Block::new("tall_grass", "upper_block_bit=0")).as_deref(), Some("minecraft:tall_grass"));
    assert_eq!(item(&Block::new("iron_door", "door_hinge_bit=0,minecraft:cardinal_direction=east,open_bit=1,upper_block_bit=0")), None, "open doors are the interact tier's");
    assert_eq!(item(&Block::new("bed", "direction=0,head_piece_bit=0,occupied_bit=1")), None);
    assert!(!supported("minecraft:bedrock"));
    let lower = BlockView { name: "minecraft:wooden_door", props: "door_hinge_bit=0,minecraft:cardinal_direction=east,open_bit=0,upper_block_bit=0", ..STONE };
    assert!(matches(&door("east", 1, 0), &lower), "BDS writes 0 in the lower half's hinge");
    let upper = BlockView { props: "door_hinge_bit=0,minecraft:cardinal_direction=east,open_bit=0,upper_block_bit=1", ..lower };
    assert!(!matches(&door("east", 1, 1), &upper));
}

#[test]
fn mirrored_doors_turn_half_round_and_swap_hinges() {
    let m = Transform { mirror: true, ..Transform::default() };
    let d = m.block(&door("east", 1, 1));
    assert_eq!((d.prop("minecraft:cardinal_direction"), d.prop("door_hinge_bit")), (Some("east"), Some("0")));
    assert_eq!(m.block(&door("north", 0, 1)).prop("minecraft:cardinal_direction"), Some("south"));
    let t = Transform { turns: 1, ..Transform::default() };
    assert_eq!(t.block(&door("east", 1, 1)).prop("minecraft:cardinal_direction"), Some("south"));
}

/// Blocks by name and state where listed, air elsewhere.
struct Cells(HashMap<BlockPos, Block>);

impl Look for Cells {
    fn solid(&self, p: BlockPos) -> Option<bool> {
        self.block(p).map(|b| b.solid())
    }

    fn find(&self, _: &Target) -> Option<Vec3> {
        None
    }

    fn block(&self, p: BlockPos) -> Option<BlockView> {
        let r = BlockRegistry::vanilla();
        let id = match self.0.get(&p) {
            Some(b) => r.find(&b.name, &b.props).unwrap_or_else(|| panic!("no state {b:?}")),
            None => r.air_id(),
        };
        r.get(id).map(BlockView::of)
    }
}

fn world(cells: &[(BlockPos, Block)]) -> Cells {
    Cells(cells.iter().cloned().collect())
}

#[test]
fn hinges_follow_the_neighbours_seen_from_the_placing_side() {
    // Facing east: placed looking north, so west is left and east right (probe "S" cases).
    let upper = door("east", 0, 1);
    let at = [5, 1, 0];
    let stone = Block::new("stone", "");
    let (l, r, lo, hi) = ([4, 0, 0], [6, 0, 0], door("north", 0, 0), door("east", 0, 1));
    let hinge = |cells: &[(BlockPos, Block)]| hinge_now(&upper, at, &world(cells));
    let both_cells = |p: BlockPos, b: &Block| [(p, b.clone()), ([p[0], 1, p[2]], b.clone())];
    assert_eq!(hinge(&[]), Some("0"));
    assert_eq!(hinge(&both_cells(r, &stone)), Some("1"));
    assert_eq!(hinge(&[(r, stone.clone()), (l, stone.clone())]), Some("0"));
    assert_eq!(hinge(&[([6, 1, 0], stone.clone())]), Some("1"), "one block right");
    assert_eq!(hinge(&[(l, stone.clone()), (r, stone.clone()), ([6, 1, 0], stone.clone())]), Some("1"), "two right against one left");
    assert_eq!(hinge(&[(l, lo.clone()), ([4, 1, 0], hi.clone())]), Some("1"), "a door left, any facing");
    assert_eq!(hinge(&[(r, lo.clone())]), Some("0"));
    assert_eq!(hinge(&[(l, lo.clone()), (r, lo.clone())]), Some("0"), "doors both sides");
    assert_eq!(hinge(&[(l, lo.clone()), (r, stone.clone()), ([6, 1, 0], stone.clone())]), Some("1"));
    for b in [Block::new("glass", ""), Block::new("oak_slab", "minecraft:vertical_half=bottom")] {
        assert_eq!(hinge(&both_cells(r, &b)), Some("0"), "{} does not count", b.name);
    }
    assert_eq!(hinge_now(&Block::new("sunflower", "upper_block_bit=1"), at, &world(&[])), None);
}

#[test]
fn the_survey_places_a_pair_once_and_clears_it_whole() {
    let (lo, hi) = (door("east", 0, 0), door("east", 0, 1));
    let cells = vec![Cell { pos: T, target: lo.clone() }, Cell { pos: [5, 1, 0], target: hi.clone() }];
    let index: HashMap<BlockPos, usize> = cells.iter().enumerate().map(|(i, c)| (c.pos, i)).collect();
    let survey = |w: &Cells| {
        let mut needs: Vec<Need> = cells.iter().map(|c| crate::build::survey::need(&c.target, w.block(c.pos), false)).collect();
        settle(&cells, &index, w, false, &mut needs);
        needs
    };
    assert_eq!(survey(&world(&[])), [Need::Place, Need::Paired]);
    assert_eq!(survey(&world(&[(T, lo.clone()), ([5, 1, 0], hi.clone())])), [Need::Done, Need::Done]);
    let flipped = door("east", 1, 1);
    assert_eq!(survey(&world(&[(T, lo.clone()), ([5, 1, 0], flipped)])), [Need::Break, Need::Paired], "a wrong hinge clears the pair from below");
    let stone = Block::new("stone", "");
    assert_eq!(survey(&world(&[([5, 1, 0], stone.clone())])), [Need::Paired, Need::Break], "junk in the upper cell goes first");
    assert_eq!(survey(&world(&[(T, stone)])), [Need::Break, Need::Paired]);
    let half = vec![Cell { pos: T, target: lo }];
    let mut needs = vec![Need::Place];
    settle(&half, &[(T, 0)].into(), &world(&[]), false, &mut needs);
    assert_eq!(needs, [Need::Unsupported], "the upper half outside the footprint");
}
