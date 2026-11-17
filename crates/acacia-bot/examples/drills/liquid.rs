//! Drills in water and lava: pools, puddles, dives, lanes, waterfalls, re-swims and levitation.

use super::{Drill, Fill, Step, FALL, IDLE, STOP};

/// A falling-water column at x+5 from a floating source 8 blocks up.
const WATERFALL: &[Fill] = &[([5, 8, 0], [5, 8, 0], "water")];
/// Water 4 deep from x+1 to x+20 (surface at y-1); deeper leaves a superflat world.
const POOL: &[Fill] = &[([1, -4, -3], [20, -1, 3], "water")];
/// Forward with one-tick jump taps every 8 ticks, like the path follower keeping afloat.
const SWIM_TAP: &[Step] = &[
    (7, 1.0, false, false, 0.0, false), (1, 1.0, false, true, 0.0, false),
    (7, 1.0, false, false, 0.0, false), (1, 1.0, false, true, 0.0, false),
    (7, 1.0, false, false, 0.0, false), (1, 1.0, false, true, 0.0, false),
    (7, 1.0, false, false, 0.0, false), (1, 1.0, false, true, 0.0, false),
    (7, 1.0, false, false, 0.0, false), (1, 1.0, false, true, 0.0, false),
    (7, 1.0, false, false, 0.0, false), (1, 1.0, false, true, 0.0, false),
    STOP,
];
const LAVA_POOL: &[Fill] = &[([1, -4, -3], [20, -1, 3], "lava")];
/// A water source, then a block placed and removed above it: BDS `/fill` schedules no liquid update.
const PUDDLE: &[Fill] = &[([8, 0, 0], [8, 0, 0], "water"), ([8, 1, 0], [8, 1, 0], "stone"), ([8, 1, 0], [8, 1, 0], "air")];

/// Started at the pool's surface; big fills can reach the client as whole-chunk resends, so wait.
const fn pool(name: &'static str, x: f32, steps: &'static [Step]) -> Drill {
    Drill { name, fills: POOL, start: [x, -1.0, 0.5], settle: 40, steps, pitch: 0.0 }
}

const fn puddle(name: &'static str, x: f32, steps: &'static [Step]) -> Drill {
    Drill { name, fills: PUDDLE, start: [x, 0.0, 0.5], settle: 60, steps, pitch: 0.0 }
}

/// The pool plus a platform 1-3 blocks up at its edge, for sprinting off into deep water.
const DIVES: [&[Fill]; 3] = [
    &[([1, -4, -3], [20, -1, 3], "water"), ([-3, 0, -1], [0, 0, 1], "stone")],
    &[([1, -4, -3], [20, -1, 3], "water"), ([-3, 0, -1], [0, 1, 1], "stone")],
    &[([1, -4, -3], [20, -1, 3], "water"), ([-3, 0, -1], [0, 2, 1], "stone")],
];

/// Sprint off a `height`-block platform into the pool looking at `pitch`, so the eyes go under while already
/// sprinting (fuzz 105051-1 tick 3261, 164225-0 tick 4460: BDS delayed StartSwimming while looking up).
const fn dive(name: &'static str, pitch: f32, height: usize) -> Drill {
    let steps: &[Step] = &[(45, 1.0, true, false, 0.0, false), STOP];
    Drill { name, fills: DIVES[height - 1], start: [-2.5, height as f32, 0.5], settle: 40, steps, pitch }
}

/// Sprint while dropping into the pool from `y` above its surface: the height varies how deep the eyes are on
/// the first tick they count as under water.
const fn plunge(name: &'static str, pitch: f32, y: f32) -> Drill {
    let steps: &[Step] = &[(45, 1.0, true, false, 0.0, false), STOP];
    Drill { name, fills: POOL, start: [2.5, y, 0.5], settle: 40, steps, pitch }
}

/// A walled lane of source water over oak slabs: standing on a slab the feet are at 0.5 and the water top at 2,
/// so the breathing point is under water at feet + 1.27 and out at feet + 1.62 (BDS `OffsetsComponent`).
const SLAB_LANE: &[Fill] = &[
    ([0, 0, -2], [13, 2, 2], "glass"),
    ([1, 0, -1], [12, 2, 1], "air"),
    ([1, 0, -1], [12, 0, 1], "oak_slab"),
    ([1, 1, -1], [12, 1, 1], "water"),
];
/// The lane over daylight detectors (0.375 high): the breathing point is under the water top at feet + 1.62
/// (1.995) and out at feet + 1.636 (2.011).
const SENSOR_LANE: &[Fill] = &[
    ([0, 0, -2], [13, 2, 2], "glass"),
    ([1, 0, -1], [12, 2, 1], "air"),
    ([1, 0, -1], [12, 0, 1], "daylight_detector"),
    ([1, 1, -1], [12, 1, 1], "water"),
];
/// The same lane without slabs (feet at 0, eyes under the water top at 2).
const STONE_LANE: &[Fill] = &[
    ([0, 0, -2], [13, 2, 2], "glass"),
    ([1, 0, -1], [12, 2, 1], "air"),
    ([1, 0, -1], [12, 1, 1], "water"),
];

const fn lane(name: &'static str, fills: &'static [Fill], y: f32) -> Drill {
    let steps: &[Step] = &[(30, 1.0, true, false, 0.0, false), STOP];
    Drill { name, fills, start: [1.5, y, 0.5], settle: 40, steps, pitch: 0.0 }
}

/// Dropped from the source's height beside the column, `x` = feet x offset (box half-width 0.3).
const fn waterfall(name: &'static str, x: f32) -> Drill {
    Drill { name, fills: WATERFALL, start: [x, 8.0, 0.5], settle: 80, steps: FALL, pitch: 0.0 }
}

/// Water 3 deep, alone or over magma (a downward bubble column).
const POOL3: &[Fill] = &[([1, -3, -3], [20, -1, 3], "water")];
const BUBBLES: &[Fill] = &[([1, -3, -3], [20, -1, 3], "water"), ([1, -4, -3], [20, -4, 3], "magma")];

/// Sprint-swim along the pool floor, stop for `gap` ticks, then press forward and sprint again, with or
/// without jump (fuzz 101523 tick 3023: BDS started that swim a tick late and its sprint two late, after
/// swimming into a corner; in open water these all match, so the wall is what matters).
const fn reswim(name: &'static str, fills: &'static [Fill], steps: &'static [Step]) -> Drill {
    Drill { name, fills, start: [2.5, -3.0, 0.5], settle: 40, steps, pitch: 0.0 }
}

const fn reswim_steps(gap: u32, jump: bool) -> [Step; 4] {
    [(15, 1.0, true, false, 0.0, false), (gap, 0.0, false, false, 0.0, false), (15, 1.0, true, jump, 0.0, false), STOP]
}

/// The 3-deep pool, plain or bubbling, with a glass wall across it at x+8.
const POOL_WALL: &[Fill] = &[([1, -3, -3], [20, -1, 3], "water"), ([8, -3, -3], [8, 1, 3], "glass")];
const BUBBLES_WALL: &[Fill] =
    &[([1, -3, -3], [20, -1, 3], "water"), ([1, -4, -3], [20, -4, 3], "magma"), ([8, -3, -3], [8, 1, 3], "glass")];

/// Sprint into a wall until pinned, stand still for `gap` ticks, then sprint off at `yaw` (fuzz 101523 tick
/// 3023: after a swim pinned in a corner, BDS started the next sprint and swim late).
pub(super) const fn pinned_steps(gap: u32, jump: bool, yaw: f32) -> [Step; 4] {
    [(40, 1.0, true, false, 0.0, false), (gap, 0.0, false, false, 0.0, false), (15, 1.0, true, jump, yaw, false), (20, 0.0, false, false, yaw, false)]
}

/// Levitation from the bottom of a pool up through the surface (see `super::hooks`).
const fn levitate(name: &'static str, fills: &'static [Fill], steps: &'static [Step]) -> Drill {
    Drill { name, fills, start: [2.5, -4.0, 0.5], settle: 40, steps, pitch: 0.0 }
}

const WALK60: &[Step] = &[(60, 1.0, false, false, 0.0, false)];

pub(super) const DRILLS: &[Drill] = &[
    pool("swimhold", 2.5, &[(60, 1.0, false, true, 0.0, false), STOP]),
    pool("swimtap", 2.5, SWIM_TAP),
    // A source at x+8 spreads over the pad: depth falls one level per block towards the start.
    puddle("puddle1", 1.5, IDLE),
    puddle("puddle3", 3.5, IDLE),
    puddle("puddle6", 6.5, IDLE),
    puddle("puddlewalk", 0.5, &[(40, 1.0, false, false, 0.0, false), STOP]),
    pool("bob60", 8.5, &[(60, 0.0, false, true, 0.0, false), (40, 0.0, false, false, 0.0, false)]),
    pool("bob13", 8.5, &[(13, 0.0, false, true, 0.0, false), (40, 0.0, false, false, 0.0, false)]),
    pool("bob5", 8.5, &[(5, 0.0, false, true, 0.0, false), (40, 0.0, false, false, 0.0, false)]),
    pool("float", 2.5, IDLE),
    waterfall("wf+05", 4.75),
    waterfall("wf+02", 4.72),
    waterfall("wf-01", 4.69),
    waterfall("wf-10", 4.6),
    // Sprint at the surface, then sink (WantDown) until the eyes go under: does a continuing sprint start a
    // swim like a held key does (fuzz 092535-0 tick 331)?
    pool("swimsinkkey", 2.5, &[(5, 1.0, true, false, 0.0, false), (25, 1.0, true, false, 0.0, true), STOP]),
    pool("swimsinknokey", 2.5, &[(5, 1.0, true, false, 0.0, false), (25, 1.0, false, false, 0.0, true), STOP]),
    dive("dive-40", -40.0, 2),
    dive("dive-20", -20.0, 2),
    dive("dive-5", -5.0, 2),
    dive("dive0", 0.0, 2),
    dive("dive10", 10.0, 2),
    dive("dive30", 30.0, 2),
    dive("dive-8", -8.0, 2),
    dive("dive-9", -9.0, 2),
    dive("dive-10", -10.0, 2),
    dive("dive-95", -9.5, 2),
    dive("dive-11", -11.0, 2),
    dive("dive-14", -14.0, 2),
    dive("dive-17", -17.0, 2),
    dive("dive1-10", -10.0, 1),
    dive("dive1-20", -20.0, 1),
    dive("dive1-40", -40.0, 1),
    dive("dive1-70", -70.0, 1),
    dive("dive3-10", -10.0, 3),
    dive("dive3-20", -20.0, 3),
    dive("dive3-40", -40.0, 3),
    dive("dive3-70", -70.0, 3),
    dive("dive-70", -70.0, 2),
    lane("slablane", SLAB_LANE, 0.5),
    lane("stonelane", STONE_LANE, 0.0),
    lane("sensorlane", SENSOR_LANE, 0.375),
    plunge("pl11a", -11.0, 1.0), plunge("pl11b", -11.0, 1.15), plunge("pl11c", -11.0, 1.3), plunge("pl11d", -11.0, 1.45),
    plunge("pl11e", -11.0, 1.6), plunge("pl11f", -11.0, 1.75),
    plunge("pl39a", -39.0, 1.0), plunge("pl39b", -39.0, 1.15), plunge("pl39c", -39.0, 1.3), plunge("pl39d", -39.0, 1.45),
    plunge("pl39e", -39.0, 1.6), plunge("pl39f", -39.0, 1.75),
    plunge("pl70a", -70.0, 1.0), plunge("pl70b", -70.0, 1.15), plunge("pl70c", -70.0, 1.3), plunge("pl70d", -70.0, 1.45),
    plunge("pl70e", -70.0, 1.6), plunge("pl70f", -70.0, 1.75),
    reswim("reswim5j", BUBBLES, &reswim_steps(5, true)), reswim("reswim5", BUBBLES, &reswim_steps(5, false)),
    reswim("reswim2j", BUBBLES, &reswim_steps(2, true)), reswim("reswim20j", BUBBLES, &reswim_steps(20, true)),
    reswim("reswimpool5j", POOL3, &reswim_steps(5, true)), reswim("reswimpool5", POOL3, &reswim_steps(5, false)),
    reswim("pin5j", POOL_WALL, &pinned_steps(5, true, 180.0)), reswim("pin5", POOL_WALL, &pinned_steps(5, false, 180.0)),
    reswim("pin5side", POOL_WALL, &pinned_steps(5, false, 80.0)), reswim("pin1", POOL_WALL, &pinned_steps(1, false, 180.0)),
    reswim("pin20", POOL_WALL, &pinned_steps(20, false, 180.0)), reswim("pin60", POOL_WALL, &pinned_steps(60, false, 180.0)),
    reswim("pinbub5j", BUBBLES_WALL, &pinned_steps(5, true, 180.0)), reswim("pinbub5", BUBBLES_WALL, &pinned_steps(5, false, 180.0)),
    levitate("lev_water", POOL, FALL),
    levitate("lev_waterwalk", POOL, WALK60),
    levitate("lev3_water", POOL, FALL),
    levitate("lev_lava", LAVA_POOL, FALL),
    levitate("lev_lavawalk", LAVA_POOL, WALK60),
];
