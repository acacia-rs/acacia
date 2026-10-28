//! What a footprint cell needs, from the world as the bot sees it now.

use super::rules;
use super::structure::Block;
use crate::pathfind::process::BlockView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    Done,
    /// Not loaded: visit it.
    Unknown,
    /// Empty (air or liquid): place the target.
    Place,
    /// Holds another block: break it (a target that is air, or a wrong block).
    Break,
    /// Liquid where the target is air: fill it with scaffold, then break that.
    Drain,
    /// Holds another block and the job is fill-only.
    Kept,
    /// The target is outside the builder's block set.
    Unsupported,
    /// A pair's second cell, or a first cell waiting for its second to be cleared (`pairs::settle`).
    Paired,
}

pub fn need(target: &Block, world: Option<BlockView>, fill_only: bool) -> Need {
    let Some(w) = world else { return Need::Unknown };
    if rules::matches(target, &w) {
        return Need::Done;
    }
    let wrong = if fill_only { Need::Kept } else { Need::Break };
    if target.is_air() {
        return match () {
            _ if w.liquid => Need::Drain,
            _ if w.air => Need::Done,
            _ => wrong,
        };
    }
    if rules::item(target).is_none() {
        return Need::Unsupported;
    }
    if !w.solid() || rules::completes(target, &w) {
        Need::Place
    } else {
        wrong
    }
}
