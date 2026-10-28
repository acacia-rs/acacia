//! What a footprint cell needs, from the world as the bot sees it now.

use acacia_physics::BlockPos;

use super::rules;
use super::structure::{Block, SignText};
use crate::pathfind::process::{BlockView, Look};

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
    /// Holds the target's block with only use-toggled states (`rules::uses`) or sign text still to set: used or
    /// written, not broken. Fill-only jobs too: the builder's own placements come out this way.
    Adjust,
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
    if rules::uses(target, &w).is_some() {
        return Need::Adjust;
    }
    if !w.solid() || rules::completes(target, &w) {
        Need::Place
    } else {
        wrong
    }
}

/// [`need`], with a done sign whose text differs from `text` to be written.
pub fn need_with_text(target: &Block, text: Option<&SignText>, pos: BlockPos, look: &dyn Look, fill_only: bool) -> Need {
    match need(target, look.block(pos), fill_only) {
        Need::Done if text.is_some_and(|t| unwritten(t, pos, look).is_some()) => Need::Adjust,
        n => n,
    }
}

/// The first side of the sign at `pos` (front first) whose text differs from `text`: (front, its text). A sign
/// whose block entity is not known reads as blank.
pub fn unwritten(text: &SignText, pos: BlockPos, look: &dyn Look) -> Option<(bool, String)> {
    [(true, &text.front), (false, &text.back)].into_iter().find(|(front, want)| look.sign_text(pos, *front).unwrap_or_default() != **want).map(|(f, t)| (f, t.clone()))
}
