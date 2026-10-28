//! [`GotoOpts::settle`](super::GotoOpts): a goal met where the bot already stands is still driven to.

use acacia_physics::Vec3;

use crate::pathfind::{Entry, Momentum, MoveKind, PathNode, Work};

/// `nodes`, or (`settle`, none needed) one walk to the centre of the cell the feet are in at `pos`.
pub(super) fn nodes(nodes: Vec<PathNode>, settle: bool, pos: Vec3) -> Vec<PathNode> {
    if !settle || !nodes.is_empty() {
        return nodes;
    }
    let at = pos.map(|v| v.floor() as i32);
    vec![PathNode { pos: at, feet: pos[1], kind: MoveKind::Walk, wet: false, work: Work::default(), entry: Entry::Rest, mom: Momentum::rest(pos[1], at), mac: None, cost: 0.0 }]
}
