//! The redstone tier's order and survey: what waits for what, what goes last, and which seen states are on their
//! way rather than wrong. See docs/builder/redstone.md.

use acacia_physics::BlockPos;

use super::rails::{self, World};
use super::redstone::{kind, liquid_of, short, source, Kind, VINE_BITS};
use crate::build::builder::Cell;
use crate::build::structure::Block;
use crate::build::survey::Need;
use crate::pathfind::process::{BlockView, Look};

fn below(p: BlockPos) -> BlockPos {
    [p[0], p[1] - 1, p[2]]
}

/// The cell shows its target (outside the footprint: something solid).
fn done(p: BlockPos, look: &dyn Look, targets: &dyn Fn(BlockPos) -> Option<Block>) -> bool {
    match targets(p) {
        Some(t) => look.block(p).is_some_and(|w| super::matches(&t, &w)),
        None => look.solid(p) == Some(true),
    }
}

fn sides(p: BlockPos) -> [BlockPos; 4] {
    [[p[0], p[1], p[2] - 1], [p[0], p[1], p[2] + 1], [p[0] - 1, p[1], p[2]], [p[0] + 1, p[1], p[2]]]
}

/// The cells the target needs first are built: its support (a vine's side), water next to sugar cane's, and for a
/// rail an order that keeps every rail on the way to its shape ([`rail_ready`]).
pub fn ready(target: &Block, pos: BlockPos, look: &dyn Look, targets: &dyn Fn(BlockPos) -> Option<Block>) -> bool {
    let Some(k) = kind(&target.name) else { return true };
    let wet = |p: BlockPos| targets(p).is_some_and(|t| kind(&t.name) == Some(Kind::Liquid));
    match k {
        Kind::Liquid | Kind::Farmland => true,
        Kind::Vine => VINE_BITS.iter().find(|(_, b)| target.prop("vine_direction_bits") == Some(b.to_string().as_str())).is_some_and(|(f, _)| {
            let o = f.offset();
            done([pos[0] - o[0], pos[1] - o[1], pos[2] - o[2]], look, targets)
        }),
        Kind::Plant if short(&target.name) == "reeds" => done(below(pos), look, targets) && sides(below(pos)).into_iter().filter(|p| wet(*p)).all(|p| done(p, look, targets)),
        Kind::Rail => done(below(pos), look, targets) && rail_ready(target, pos, look, targets),
        _ => done(below(pos), look, targets),
    }
}

/// Liquids and what lives in them go last: water flows over cells still to build and washes dust, rails and
/// plants away. Drops them from `ready` while anything dry is ready.
pub fn dry_first(ready: &mut Vec<(usize, String)>, target: impl Fn(usize) -> Block) {
    let wet = |i: usize| matches!(kind(&target(i).name), Some(Kind::Liquid | Kind::Aquatic));
    if ready.iter().any(|(i, _)| !wet(*i)) {
        ready.retain(|(i, _)| !wet(*i));
    }
}

fn rail_of(look: &dyn Look, p: BlockPos) -> Option<rails::Rail> {
    let v = look.block(p).filter(|v| rails::is_rail(v.name))?;
    Some((rails::straight(v.name), v.prop("rail_direction")?.parse().ok()?))
}

fn target_shape(targets: &dyn Fn(BlockPos) -> Option<Block>, p: BlockPos) -> Option<u8> {
    targets(p).filter(|t| rails::is_rail(&t.name))?.prop("rail_direction")?.parse().ok()
}

/// Every rail the placement shapes joins only rails its target leads to.
fn keeps_shapes(w: &mut World, pos: BlockPos, straight: bool, targets: &dyn Fn(BlockPos) -> Option<Block>) -> bool {
    w.place(pos, straight).into_iter().all(|(p, _)| {
        let Some(t) = target_shape(targets, p) else { return true };
        let to = rails::ends(p, t);
        w.joined(p).iter().all(|j| to.iter().any(|e| e[0] == j[0] && e[2] == j[2]))
    })
}

/// Rail cells around `pos` (same height, one up, one down).
fn around(pos: BlockPos) -> impl Iterator<Item = BlockPos> {
    sides(pos).into_iter().flat_map(|s| [-1, 0, 1].map(|dy| [s[0], s[1] + dy, s[2]]))
}

/// Placing the rail now keeps every rail on the way to its shape, and so does placing any open rail beside it
/// next (one step of lookahead: a junction's middle waits for both ends of its own line).
fn rail_ready(target: &Block, pos: BlockPos, look: &dyn Look, targets: &dyn Fn(BlockPos) -> Option<Block>) -> bool {
    let base = |p: BlockPos| rail_of(look, p);
    let mut w = World::new(&base);
    if !keeps_shapes(&mut w, pos, rails::straight(&target.name), targets) {
        return false;
    }
    around(pos).all(|q| match targets(q).filter(|t| rails::is_rail(&t.name)) {
        Some(t) if w.get(q).is_none() => keeps_shapes(&mut w.clone(), q, rails::straight(&t.name), targets),
        _ => true,
    })
}

/// The liquids the structure has sources of.
pub fn poured<'a>(blocks: impl Iterator<Item = &'a Block>) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = blocks.filter(|b| kind(&b.name) == Some(Kind::Liquid) && b.prop("liquid_depth").is_none_or(|d| d == "0")).filter_map(|b| liquid_of(&b.name)).collect();
    out.dedup();
    out
}

/// Liquid flowing from the structure's own sources where air belongs: left alone, and not a mismatch.
pub fn spilled(poured: &[&str], target: &Block, w: &BlockView) -> bool {
    target.is_air() && w.liquid && !source(w) && liquid_of(w.name).is_some_and(|l| poured.contains(&l))
}

/// Corrects the survey for this family: a rail on its way to its shape (joined only where its target leads, a
/// rail at one of its target's ends not at its own shape yet) is not wrong yet; liquid flowing from the job's own
/// sources into cells where air belongs is left alone (draining it would fight the sources).
pub fn settle(cells: &[Cell], look: &dyn Look, targets: &dyn Fn(BlockPos) -> Option<Block>, needs: &mut [Need]) {
    let base = |p: BlockPos| rail_of(look, p);
    let w = World::new(&base);
    let poured = poured(cells.iter().map(|c| &c.target));
    for (c, n) in cells.iter().zip(needs.iter_mut()) {
        let Some(seen) = look.block(c.pos) else { continue };
        match *n {
            Need::Drain if spilled(&poured, &c.target, &seen) => *n = Need::Done,
            Need::Break if seen.name == c.target.name && rails::is_rail(seen.name) => {
                let Some(t) = target_shape(targets, c.pos) else { continue };
                let to = rails::ends(c.pos, t);
                let joined = w.joined(c.pos);
                let on_way = joined.iter().all(|j| to.iter().any(|e| e[0] == j[0] && e[2] == j[2]));
                // Not yet at its own shape either: a neighbour just placed shows the client's prediction, and
                // the server's turn of this rail comes with its echo (live: broken a tick before it turned).
                let to_come = to.iter().any(|e| [*e, [e[0], e[1] + 1, e[2]], below(*e)].iter().any(|q| target_shape(targets, *q).is_some() && !done(*q, look, targets)));
                if on_way && to_come {
                    *n = Need::Done;
                }
            }
            _ => {}
        }
    }
}
