//! Where to stand and what to click: a placement's state depends on the clicked face, the click height and the
//! player's facing (see `rules`), so a cell is placed from a spot whose eye makes the click give the target.

use acacia_physics::{BlockPos, Vec3};
use acacia_world::{BlockAccess, BlockRegistry};

use super::rules;
use super::structure::Block;
use crate::interact::sight::Sight;
use crate::interact::Face;
use crate::movement::EYE_HEIGHT;
use crate::pathfind::process::{BlockView, Look};

/// Margin under the reach the builder plans with.
const MARGIN: f32 = 0.35;
/// More margin when choosing a stand: the pilot stops up to its tolerance off the cell's centre.
pub const STAND_SLACK: f32 = 0.45;
/// The reach stands are planned with, eye to the block's centre like `interact::BLOCK_REACH`. Live house16 A/B
/// (tools/build-ab.sh, then measured to the nearest point): 5.5 against Java's 4.5 took one bot ~146 s against
/// 161 s, two bots ~94 against 105.
const PLAN_REACH: f32 = 5.7;

/// [`PLAN_REACH`], or `BUILD_REACH` for an A/B (capped at the act limit).
fn plan_reach() -> f32 {
    static R: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *R.get_or_init(|| std::env::var("BUILD_REACH").ok().and_then(|v| v.parse().ok()).unwrap_or(PLAN_REACH).min(crate::interact::BLOCK_REACH))
}

/// How far around a cell stands are looked for.
fn radius() -> i32 {
    plan_reach().ceil() as i32 - 1
}
/// Pillar stands (over air) up to this many blocks above the ground under them.
const MAX_PILLAR: i32 = 6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Click {
    pub against: BlockPos,
    pub face: Face,
    /// Relative to `against`'s minimum corner.
    pub click: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stand {
    /// Feet cell, as the pathfinder keys a standing spot.
    pub feet: BlockPos,
    /// Feet height above the cell's bottom: 0.5 on a bottom slab in the cell.
    pub lift: f32,
    /// Blocks of pillar the pilot has to build under it (0: stands on something).
    pub pillar: i32,
}

impl Stand {
    pub fn eye(&self) -> Vec3 {
        [self.feet[0] as f32 + 0.5, self.feet[1] as f32 + self.lift + EYE_HEIGHT, self.feet[2] as f32 + 0.5]
    }
}

fn bottom_slab(b: &BlockView) -> bool {
    b.name.ends_with("_slab") && b.prop("minecraft:vertical_half") == Some("bottom")
}

pub fn in_reach(eye: Vec3, p: BlockPos) -> bool {
    within(eye, p, 0.0)
}

pub fn within(eye: Vec3, p: BlockPos, slack: f32) -> bool {
    crate::interact::block_distance(eye, p) <= plan_reach() - MARGIN - slack
}

/// The eye is on the outer side of `face` of the block at `p`: the face can be seen.
fn sees(eye: Vec3, p: BlockPos, face: Face) -> bool {
    let lo = |i: usize| p[i] as f32;
    match face {
        Face::Up => eye[1] > lo(1) + 1.0,
        Face::Down => eye[1] < lo(1),
        Face::North => eye[2] < lo(2),
        Face::South => eye[2] > lo(2) + 1.0,
        Face::West => eye[0] < lo(0),
        Face::East => eye[0] > lo(0) + 1.0,
    }
}

/// Clicks on `face` of `against`, the usual ones first: its centre, or low and high on a side face (a slab's or
/// stair's half follows the click); then a grid over the rest, so a partly hidden face still gives a seen click.
/// Only points on the block's shape: a full cube anywhere, a half slab its own half of the side faces.
fn points(face: Face, against: &BlockView) -> Vec<Vec3> {
    let [ox, oy, oz] = face.offset().map(|v| v as f32);
    let c = [0.5 + 0.5 * ox, 0.5 + 0.5 * oy, 0.5 + 0.5 * oz];
    const GRID: [f32; 3] = [0.5, 0.2, 0.8];
    // Side grids keep off y 0.5, where a stair's or slab's half flips.
    let side = |ys: &[f32], more: &[f32]| {
        let at = move |h: f32, y: f32| if ox != 0.0 { [c[0], y, h] } else { [h, y, c[2]] };
        ys.iter().map(|&y| at(0.5, y)).chain(GRID.iter().flat_map(|&h| more.iter().map(move |&y| at(h, y)))).collect()
    };
    let slab = against.name.ends_with("_slab").then(|| against.prop("minecraft:vertical_half")).flatten();
    match (face, slab) {
        (Face::Up | Face::Down, None) if rules::post(against) => vec![c],
        (Face::Up, None) if !against.full => rules::redstone::top(against.name).map(|y| vec![[c[0], y, c[2]]]).unwrap_or_default(),
        (_, None) if !against.full => Vec::new(),
        (Face::Up | Face::Down, None) => GRID.iter().flat_map(|&x| GRID.iter().map(move |&z| [x, c[1], z])).collect(),
        (_, None) => side(&[0.25, 0.75], &[0.15, 0.4, 0.6, 0.85]),
        (Face::Up | Face::Down, _) => Vec::new(),
        (_, Some("top")) => side(&[0.75], &[0.6, 0.9]),
        (_, Some(_)) => side(&[0.25], &[0.1, 0.4]),
    }
}

/// A click from `eye` that places `target` into `pos` with its state, reaching `slack` short of the limit, on a
/// point the eye sees: no other block's shape is in the way (`interact::sight`).
pub fn click_for(pos: BlockPos, target: &Block, eye: Vec3, look: &dyn Look, slack: f32) -> Option<Click> {
    let world = LookBlocks(look);
    let sight = Sight { world: &world, registry: BlockRegistry::vanilla() };
    for face in Face::ALL {
        let o = face.offset();
        let against = [pos[0] - o[0], pos[1] - o[1], pos[2] - o[2]];
        let Some(b) = look.block(against) else { continue };
        if !b.solid() || b.interactive || !sees(eye, against, face) || !within(eye, against, slack) {
            continue;
        }
        let into = look.block(pos).is_some_and(|t| t.solid()).then_some(pos);
        let visible = |c: &Vec3| sight.sees_face_into(eye, against, face, std::array::from_fn(|i| against[i] as f32 + c[i]), into);
        let Some(click) = points(face, &b).into_iter().find(|c| rules::gives(target, face, *c, eye, pos, &b) && visible(c)) else { continue };
        return Some(Click { against, face, click });
    }
    None
}

/// Some point of the block's shape at `pos` shows to the eye: where a use-click or a break aims.
pub fn sees_block(eye: Vec3, pos: BlockPos, look: &dyn Look) -> bool {
    let world = LookBlocks(look);
    !Sight { world: &world, registry: BlockRegistry::vanilla() }.visible_points(eye, pos).is_empty()
}

/// A break of `pos` from `eye` the act accepts: in reach and in view.
pub fn breaks_from(eye: Vec3, pos: BlockPos, look: &dyn Look) -> bool {
    in_reach(eye, pos) && sees_block(eye, pos, look)
}

/// A fill of `pos` from `eye` the act accepts (`pathfind::process::act::fill`): some solid, non-interactive
/// neighbour in reach whose face towards `pos` the eye sees at its centre.
pub fn fills_from(eye: Vec3, pos: BlockPos, look: &dyn Look) -> bool {
    let world = LookBlocks(look);
    let sight = Sight { world: &world, registry: BlockRegistry::vanilla() };
    Face::ALL.into_iter().any(|face| {
        let o = face.offset();
        let against = [pos[0] - o[0], pos[1] - o[1], pos[2] - o[2]];
        look.block(against).is_some_and(|b| !b.air && !b.liquid && !b.interactive) && in_reach(eye, against) && sight.sees_face_centre(eye, against, face)
    })
}

/// A [`Look`] as block ids for the shared ray. Unloaded cells, and blocks the vanilla registry lacks, read as air.
struct LookBlocks<'a>(&'a dyn Look);

impl BlockAccess for LookBlocks<'_> {
    fn block(&self, x: i32, y: i32, z: i32) -> u32 {
        let r = BlockRegistry::vanilla();
        match self.0.block([x, y, z]) {
            Some(b) if !b.air => r.find(b.name, b.props).unwrap_or_else(|| r.air_id()),
            _ => r.air_id(),
        }
    }

    fn liquid(&self, _: i32, _: i32, _: i32) -> u32 {
        BlockRegistry::vanilla().air_id()
    }
}

fn passable(look: &dyn Look, p: BlockPos) -> bool {
    look.block(p).is_some_and(|b| !b.solid())
}

/// The bot's box at `feet` keeps clear of `pos`.
fn clear_of(feet: BlockPos, pos: BlockPos) -> bool {
    feet != pos && [feet[0], feet[1] + 1, feet[2]] != pos
}

/// Where a stand may not be: `feet` cells to avoid, and cells a pillar may not fill.
pub struct Keep<'a> {
    pub feet: &'a dyn Fn(BlockPos) -> bool,
    pub pillar: &'a dyn Fn(BlockPos) -> bool,
}

/// Up to `limit` spots near `pos` the bot can stand at (or pillar up to) with `ok(eye)`, cheapest from `from` first.
pub fn stands(pos: BlockPos, from: Vec3, look: &dyn Look, keep: &Keep, ok: &dyn Fn(Vec3) -> bool, limit: usize) -> Vec<Stand> {
    let (mut all, r) = (Vec::new(), radius());
    for dx in -r..=r {
        for dz in -r..=r {
            for dy in -3..=1 {
                let feet = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
                let Some(s) = footing(look, feet) else { continue };
                let body_clear = clear_of(feet, pos) && (s.lift == 0.0 || [feet[0], feet[1] + 2, feet[2]] != pos);
                let pillar_ok = (0..s.pillar).all(|k| !(keep.pillar)([feet[0], feet[1] - 1 - k, feet[2]]));
                if body_clear && pillar_ok && !(keep.feet)(feet) && within(s.eye(), pos, STAND_SLACK) {
                    all.push(s);
                }
            }
        }
    }
    let cost = |s: &Stand| {
        let d = [s.feet[0] as f32 + 0.5 - from[0], s.feet[1] as f32 + s.lift - from[1], s.feet[2] as f32 + 0.5 - from[2]];
        (d[0] * d[0] + d[2] * d[2]).sqrt() + d[1].abs() * 2.0 + s.pillar as f32 * 4.0
    };
    all.sort_by(|a, b| cost(a).total_cmp(&cost(b)));
    all.into_iter().filter(|s| ok(s.eye())).take(limit).collect()
}

/// How feet in `cell` would rest, as the pathfinder sees a standing spot: on a bottom slab in the cell, on a full
/// block or upper slab under it, or on a pillar the pilot builds (up to [`MAX_PILLAR`]). A bottom slab under the
/// cell is no footing: that spot belongs to the slab's own cell. Partial blocks (fences, panes) are not stood on.
fn footing(look: &dyn Look, cell: BlockPos) -> Option<Stand> {
    let up = |n: i32| [cell[0], cell[1] + n, cell[2]];
    let here = look.block(cell)?;
    if bottom_slab(&here) {
        return (passable(look, up(1)) && passable(look, up(2))).then_some(Stand { feet: cell, lift: 0.5, pillar: 0 });
    }
    if here.solid() || !passable(look, up(1)) {
        return None;
    }
    for n in 0..=MAX_PILLAR {
        let b = look.block(up(-1 - n))?;
        match () {
            _ if b.liquid => return None,
            _ if bottom_slab(&b) => return None,
            // A pillar's first block is clicked onto this one: a click on a container opens it instead (live:
            // dirt onto a station barrel refused 3 times over).
            _ if n > 0 && b.interactive => return None,
            _ if b.solid() && (b.full || b.name.ends_with("_slab")) => return Some(Stand { feet: cell, lift: 0.0, pillar: n }),
            _ if b.solid() => return None,
            _ => {}
        }
    }
    None
}
