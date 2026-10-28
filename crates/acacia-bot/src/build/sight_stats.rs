//! `BUILD_SIGHT_STATS=1`: why `reach::click_for` found no click, counted by reason and, for hidden faces, by what
//! hid them; [`dump`] returns the table. Unset, nothing is counted.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use acacia_physics::{BlockPos, Vec3};
use acacia_world::{ray, selection, Aabb, BlockAccess, BlockRegistry};

static COUNTS: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());

/// How far a face got towards a click, the furthest of all faces counting as the call's reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Stage {
    NothingToClick,
    FaceAway,
    OutOfReach,
    NoPointGivesState,
    Hidden,
}

pub(super) fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("BUILD_SIGHT_STATS").is_some())
}

thread_local! {
    static SITE: std::cell::Cell<&'static str> = const { std::cell::Cell::new("other") };
}

/// Runs `f` with its counts filed under `site` (which caller asked).
pub(super) fn at<T>(site: &'static str, f: impl FnOnce() -> T) -> T {
    let before = SITE.replace(site);
    let out = f();
    SITE.set(before);
    out
}

/// Counts one call under the current site; `key` is built only when counting is on.
pub(super) fn count(key: impl FnOnce() -> String) {
    if enabled() {
        let key = format!("{}: {}", SITE.get(), key());
        *COUNTS.lock().unwrap_or_else(|e| e.into_inner()).entry(key).or_default() += 1;
    }
}

/// The counts, largest first, one `count key` line each; empty when off.
pub fn dump() -> String {
    let counts = COUNTS.lock().unwrap_or_else(|e| e.into_inner());
    let mut rows: Vec<_> = counts.iter().collect();
    rows.sort_by(|a, b| b.1.cmp(a.1));
    rows.iter().map(|(k, n)| format!("{n:>8} {k}\n")).collect()
}

/// What stops the ray from `eye` to `point` on `against` (placing into `pos`): the shape's kind, where it is, and
/// whether only its selection box stops the ray (its collision would let it through).
pub(super) fn blocker(world: &impl BlockAccess, eye: Vec3, point: Vec3, pos: BlockPos, against: BlockPos) -> String {
    let r = BlockRegistry::vanilla();
    let d = std::array::from_fn::<f64, 3, _>(|i| f64::from(point[i] - eye[i]));
    let len = d.iter().map(|c| c * c).sum::<f64>().sqrt();
    let hit = |with_selection: bool| {
        ray::cast(world, eye.map(f64::from), d.map(|c| c / len), len + 0.05, |p: [i32; 3], id: u32, out: &mut Vec<Aabb>| {
            if let Some(s) = r.get(id) {
                out.extend(s.boxes.iter().copied());
                if with_selection {
                    out.extend(selection::boxes_at(world, r, p));
                }
            }
        })
    };
    let Some(h) = hit(true) else { return "hidden: no hit".into() };
    let name = r.get(world.block(h.block[0], h.block[1], h.block[2])).map_or("?", |s| s.name.trim_start_matches("minecraft:"));
    let kind = ["dirt", "pane", "fence", "wall", "torch", "ladder", "button", "slab", "stairs", "door", "log", "planks", "cobblestone"]
        .into_iter()
        .find(|k| name.contains(k))
        .unwrap_or("other");
    let near = |a: BlockPos, b: BlockPos| (0..3).all(|i| (a[i] - b[i]).abs() <= 1);
    let place = match h.block {
        b if b == against => "the clicked block, another face",
        b if b == pos => "the cell itself",
        b if near(b, pos) => "next to the cell",
        b if near(b, eye.map(|c| c.floor() as i32)) => "next to the eye",
        _ => "between",
    };
    let by = if hit(false).is_some_and(|c| c.block == h.block) { "collision" } else { "selection only" };
    let e = eye.map(|c| c.floor() as i32);
    let (dy, dh) = (pos[1] - e[1], (pos[0] - e[0]).abs().max((pos[2] - e[2]).abs()));
    format!("hidden by {kind}, {place} ({by}); cell dy {dy} dh {dh}")
}
