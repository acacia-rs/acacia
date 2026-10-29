//! Targets whose state only a top-face click gives (an upright log or pillar, a floor torch, rail or dust), found
//! by asking [`super::gives`] about every side face from eyes all around, so the rule follows the placement rules
//! rather than block names.

use std::cell::RefCell;
use std::collections::HashMap;

use super::super::structure::Block;
use crate::interact::Face;
use crate::pathfind::process::BlockView;

const STONE: BlockView = BlockView { name: "minecraft:stone", props: "", air: false, liquid: false, full: true, interactive: false };
const SIDES: [Face; 4] = [Face::North, Face::South, Face::West, Face::East];
const AT: [f32; 3] = [0.1, 0.5, 0.9];

thread_local! {
    static KNOWN: RefCell<HashMap<(String, String), bool>> = RefCell::new(HashMap::new());
}

fn probe(target: &Block, face: Face, against: &BlockView) -> bool {
    let eyes = (0..8).flat_map(|k| {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        [-1.0, 1.6, 3.0].map(move |y| [0.5 + 2.5 * a.cos(), y, 0.5 + 2.5 * a.sin()])
    });
    eyes.into_iter().any(|eye| AT.iter().any(|&x| AT.iter().any(|&y| AT.iter().any(|&z| super::gives(target, face, [x, y, z], eye, [0, 0, 0], against)))))
}

/// No side-face click gives `target`, and a top-face one does.
pub fn only_from_above(target: &Block) -> bool {
    let key = (target.name.clone(), target.props.clone());
    if let Some(known) = KNOWN.with(|k| k.borrow().get(&key).copied()) {
        return known;
    }
    let only = probe(target, Face::Up, &STONE) && !SIDES.iter().any(|f| probe(target, *f, &STONE));
    KNOWN.with(|k| k.borrow_mut().insert(key, only));
    only
}
