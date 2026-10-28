//! Cells holding the target's block with use-toggled states or sign text still to set (`Need::Adjust`): used
//! (right-clicked) or written instead of broken and placed again. A use needs reach and some visible point of
//! the block; a sign's back is written from behind it (the server opens the side the player stands on).

use acacia_physics::Vec3;

use super::builder::Builder;
use super::plan::{go, no_stand, unbuilt, Next, STANDS};
use super::reach::{in_reach, sees_block, stands, Keep};
use super::rules;
use super::status::Phase;
use super::survey::unwritten;
use crate::interact::Face;
use crate::pathfind::process::{Act, Ctx};

/// Cells tried per tick, nearest first.
const TRIES: usize = 8;

/// The act for the nearest open adjustment, or a walk to where it can be done; `None`: none can be.
pub(super) fn next(b: &mut Builder, cx: &Ctx, cells: &[usize]) -> Option<Next> {
    for &i in cells.iter().take(TRIES) {
        let pos = b.cells[i].pos;
        let Some(w) = cx.look.block(pos) else { continue };
        let act = match (rules::uses(&b.cells[i].target, &w), &b.cells[i].text) {
            (Some((_, item)), _) => {
                if item.as_ref().is_some_and(|i| cx.look.count(i) == 0) {
                    continue;
                }
                Act::Use { pos, item, face: Face::Up }
            }
            (None, Some(t)) => match unwritten(t, pos, cx.look) {
                Some((front, text)) => Act::Sign { pos, front, text },
                None => continue,
            },
            _ => continue,
        };
        let side = match &act {
            Act::Sign { front, .. } => Some(*front),
            _ => None,
        };
        let ok = |eye: Vec3| in_reach(eye, pos) && sees_block(eye, pos, cx.look) && side.is_none_or(|f| rules::front(&w, pos, eye) == Some(f));
        if ok(cx.eye()) {
            return Some(Next::Act(Phase::Building, act));
        }
        let found = stands(pos, cx.pos, cx.look, &Keep { feet: &|f| no_stand(b, cx.look, f), pillar: &|c| unbuilt(b, cx.look, c) }, &ok, STANDS);
        match go(Phase::Building, pos, found, b) {
            Some(next) => return Some(next),
            None => b.rest(pos),
        }
    }
    None
}
