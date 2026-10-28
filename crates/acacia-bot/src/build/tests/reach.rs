//! Stands and clicks against line of sight (`build::reach`): the planner accepts only what the act will.

use std::collections::HashMap;

use acacia_physics::{BlockPos, Vec3};
use acacia_world::BlockRegistry;

use crate::build::reach::{breaks_from, click_for, fills_from};
use crate::build::Block;
use crate::interact::Face;
use crate::pathfind::process::{BlockView, Look, Target};

/// Stone where listed, air elsewhere.
struct Cells(HashMap<BlockPos, &'static str>);

impl Look for Cells {
    fn solid(&self, p: BlockPos) -> Option<bool> {
        Some(self.0.contains_key(&p))
    }

    fn find(&self, _: &Target) -> Option<Vec3> {
        None
    }

    fn block(&self, p: BlockPos) -> Option<BlockView> {
        let r = BlockRegistry::vanilla();
        let name = self.0.get(&p).copied().unwrap_or("minecraft:air");
        r.get(r.find(name, "")?).map(BlockView::of)
    }
}

fn stone(cells: &[BlockPos]) -> Cells {
    Cells(cells.iter().map(|&p| (p, "minecraft:stone")).collect())
}

#[test]
fn a_click_behind_another_block_is_not_planned() {
    // Stone at x 2 to place on top of, from an eye at y 1.62 west of it: the Up face is in view until a
    // stone at x 1, y 1 hides it.
    let (pos, eye) = ([2, 1, 0], [0.5, 1.62 + 0.4, 0.5]);
    let target = Block::new("stone", "");
    assert!(click_for(pos, &target, eye, &stone(&[[2, 0, 0]]), 0.0).is_some_and(|c| c.face == Face::Up));
    assert_eq!(click_for(pos, &target, eye, &stone(&[[2, 0, 0], [1, 1, 0]]), 0.0), None);
}

/// From high in the north, a stone north of the cell hides the near half of the top face below it, centre included.
const HIGH_NORTH: Vec3 = [0.5, 4.8, -1.5];

#[test]
fn a_partly_hidden_face_still_gives_a_click() {
    let c = click_for([0, 1, 0], &Block::new("stone", ""), HIGH_NORTH, &stone(&[[0, 0, 0], [0, 1, -1]]), 0.0);
    assert!(c.is_some_and(|c| c.face == Face::Up && c.click[2] > 0.5), "{c:?}");
}

#[test]
fn fills_need_the_face_centre_as_the_fill_act_does() {
    assert!(fills_from(HIGH_NORTH, [0, 1, 0], &stone(&[[0, 0, 0]])));
    assert!(!fills_from(HIGH_NORTH, [0, 1, 0], &stone(&[[0, 0, 0], [0, 1, -1]])));
}

#[test]
fn a_buried_block_is_broken_only_once_uncovered() {
    let buried: Vec<BlockPos> = [[0, 0, 0], [0, 1, 0], [0, -1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]].into();
    let eye = [0.5, 2.62, 0.5];
    assert!(!breaks_from(eye, [0, 0, 0], &stone(&buried)));
    assert!(breaks_from(eye, [0, 0, 0], &stone(&buried[..1].iter().chain(&buried[2..]).copied().collect::<Vec<_>>())));
}
