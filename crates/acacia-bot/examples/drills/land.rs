//! Drills on land: sprints into walls, double taps, sneak edges, slime, honey, powder snow, low ceilings,
//! piston arms, hunger and crawling.

use super::liquid::pinned_steps;
use super::{drill, Drill, Fill, Step, FALL, STOP};

const WALL3: &[Fill] = &[([4, 0, -12], [4, 2, 12], "glass")];
const WALL2: &[Fill] = &[([4, 0, -12], [4, 1, 12], "glass")];

const SLIME: &[Fill] = &[([-1, -1, -1], [1, -1, 1], "slime")];
/// A piston at x+3 pushing east (facing_direction 4), powered from the side: its arm fills x+4 at z+1.
const ARM: &[Fill] = &[([3, 0, 1], [3, 0, 1], "piston [\"facing_direction\"=4]"), ([3, 0, 2], [3, 0, 2], "redstone_block")];
const BOUNCE: &[Step] = &[(100, 0.0, false, false, 0.0, false)];

/// Dropped onto a slime floor from `y`, then left to bounce out.
const fn slime(name: &'static str, y: f32) -> Drill {
    Drill { name, fills: SLIME, start: [0.5, y, 0.5], settle: 10, steps: BOUNCE, pitch: 0.0 }
}

/// A 2-deep pit from x+4: the sneak edge stop at its rim.
const PIT: &[Fill] = &[([4, -2, -12], [24, -1, 12], "air")];

/// A honey column at x+2, alone or with a 2-deep pool at its foot.
const HONEY: &[Fill] = &[([2, 0, 0], [2, 4, 0], "honey_block")];
const HONEY_POOL: &[Fill] = &[([2, 0, 0], [2, 4, 0], "honey_block"), ([0, 0, -1], [1, 1, 1], "water")];
/// One floating honey block: in a column the block above keeps the slide going, hiding the top threshold.
const HONEY_ONE: &[Fill] = &[([2, 3, 0], [2, 3, 0], "honey_block")];

/// Dropped from `y` beside the honey column while pressing into it, sliding down its side (fuzz 154827 tick
/// 4752: BDS slides up to the block's full height, and not while touching water). Start heights vary where
/// tick ends fall relative to each block's top.
const fn honey(name: &'static str, fills: &'static [Fill], y: f32) -> Drill {
    let steps: &[Step] = &[(50, 1.0, false, false, 0.0, false), STOP];
    Drill { name, fills, start: [1.5, y, 0.5], settle: 10, steps, pitch: 0.0 }
}

/// Powder snow over the open pad, ankle deep: freezing slows the walk every tick (fuzz 145851 ticks 5575/5625/
/// 5631, each after two `minecraft:movement` updates stamped with the same input tick).
const POWDER: &[Fill] = &[([0, 0, -6], [20, 0, 6], "powder_snow")];
const POWDER_WALK: &[Step] = &[
    (30, 1.0, false, false, 0.0, false), (30, 1.0, true, false, 20.0, false), (30, -1.0, false, false, -20.0, false),
    (30, 1.0, true, false, -30.0, false), (40, 1.0, false, false, 0.0, false), STOP,
];
/// A powder snow pit 3 deep with its top at the pad's surface: BDS's shape for a player on top depends on the
/// fall and the boots (docs/research/unported-small-rules.md §8).
const POWDER_PIT: &[Fill] = &[([0, -3, -3], [12, -1, 3], "powder_snow")];
/// Walk on top, sneak (sinks through), then hold jump (the boots' climb).
const POWDER_BOOTS: &[Step] =
    &[(40, 1.0, false, false, 0.0, false), (20, 0.0, false, false, 0.0, true), (30, 0.0, false, true, 0.0, false), STOP];

const fn powder_drop(name: &'static str, y: f32) -> Drill {
    Drill { name, fills: POWDER_PIT, start: [2.5, y, 0.5], settle: 10, steps: FALL, pitch: 0.0 }
}

/// A 1.5-high tunnel over slabs from x+3 to x+5, as the fuzz builds its low ceilings.
const TUNNEL: &[Fill] = &[([1, 0, -1], [9, 0, 1], "smooth_stone_slab"), ([3, 2, -1], [5, 2, 1], "stone")];

/// Sneak into the tunnel, release the sneak inside and walk on: the crouch ends on the tick standing fits. `x`
/// shifts the start, so each drill leaves the ceiling with a different overlap (fuzz 104210 tick 3951: BDS stood
/// up with the box still 0.0077 under the ceiling's edge). Timed for Swift Sneak III leggings: the box is
/// 0.0083 under the edge in `duck0`, 0.002 more per drill from `duck2` on.
const fn duck(name: &'static str, x: f32) -> Drill {
    let steps: &[Step] = &[(22, 1.0, false, false, 0.0, true), (110, 0.25, false, false, 0.0, false), STOP];
    Drill { name, fills: TUNNEL, start: [1.5 + x, 0.5, 0.5], settle: 20, steps, pitch: 0.0 }
}

/// A 1-high tunnel from x+2 to x+8 (stone at head height): a block set in the head's cell (`super::hooks`) leaves
/// no room to stand or sneak, which a vanilla client answers by crawling.
const CRAWL_TUNNEL: &[Fill] = &[([2, 1, -1], [8, 1, 1], "stone")];

/// Sprint `ticks` at `yaw` towards the pit, then sneak on.
const fn rush(ticks: u32, yaw: f32) -> [Step; 3] {
    [(ticks, 1.0, true, false, yaw, false), (25, 1.0, false, false, yaw, true), STOP]
}

/// Plain walk/stop/sneak steps for the double-tap drills.
const fn walk(ticks: u32, forward: f32, sprint: bool, sneak: bool) -> Step {
    (ticks, forward, sprint, false, 0.0, sneak)
}

pub(super) const DRILLS: &[Drill] = &[
    drill("sprint", &[], &[(30, 1.0, true, false, 0.0, false), STOP]),
    drill("sprintwall", WALL3, &[(30, 1.0, true, false, 0.0, false), (15, 1.0, true, false, 60.0, false), (20, 0.0, false, false, 60.0, false)]),
    drill("sprintwall30", WALL3, &[(12, 1.0, true, false, 0.0, false), (20, 1.0, true, false, 30.0, false), (20, 0.0, false, false, 30.0, false)]),
    drill("fracsprint", &[], &[(20, 1.0, true, false, 0.0, false), (10, 0.5, true, false, 0.0, false), STOP]),
    drill("frac75", &[], &[(20, 1.0, true, false, 0.0, false), (10, 0.75, true, false, 0.0, false), STOP]),
    drill("fracwalk", &[], &[(40, 0.5, false, false, 0.0, false), STOP]),
    drill("frac75long", &[], &[(10, 1.0, true, false, 0.0, false), (40, 0.75, true, false, 0.0, false), STOP]),
    drill("jumpwall", WALL2, &[(8, 1.0, true, false, 0.0, false), (12, 1.0, true, true, 20.0, false), (15, 1.0, true, false, 60.0, false), (20, 0.0, false, false, 60.0, false)]),
    drill("turnsprint", &[], &[(15, 1.0, true, false, 0.0, false), (10, 1.0, true, false, 90.0, false), (10, 1.0, true, false, 150.0, false), STOP]),
    Drill { name: "drop", fills: &[], start: [0.5, 8.0, 0.5], settle: 10, steps: FALL, pitch: 0.0 },
    slime("slime2", 2.0),
    slime("slime4", 4.0),
    slime("slime6", 6.0),
    slime("slime9", 9.0),
    // Double tap around a sneak release (capture session 2 tick 1653; fuzz 220323-0 tick 1255).
    drill("dtplain", &[], &[walk(3, 1.0, false, false), walk(2, 0.0, false, false), walk(15, 1.0, false, false), STOP]),
    drill("dtsneakheld", &[], &[walk(10, 1.0, false, true), walk(3, 1.0, false, false), walk(2, 0.0, false, false), walk(15, 1.0, false, false), STOP]),
    // A step lasts one tick less than asked after a control change, so 3 here holds forward for 2.
    drill("dtsneakpress", &[], &[walk(10, -1.0, false, true), walk(3, 1.0, false, false), walk(4, -1.0, false, false), walk(15, 1.0, false, false), STOP]),
    drill("dtsneakkey", &[], &[walk(10, -1.0, false, true), walk(3, 1.0, true, false), walk(4, -1.0, true, false), walk(15, 1.0, false, false), STOP]),
    // The sneak edge stop meeting leftover sprint momentum, so it only shrinks the velocity (fuzz 145851 tick 281:
    // BDS kept a partly shrunk axis's velocity but zeroes a fully stopped one).
    drill("edge4", PIT, &rush(4, 0.0)), drill("edge6", PIT, &rush(6, 0.0)), drill("edge8", PIT, &rush(8, 0.0)),
    drill("edge10", PIT, &rush(10, 0.0)), drill("edge12", PIT, &rush(12, 0.0)),
    drill("edged4", PIT, &rush(4, 25.0)), drill("edged6", PIT, &rush(6, 25.0)), drill("edged8", PIT, &rush(8, 25.0)),
    drill("edged10", PIT, &rush(10, 25.0)), drill("edged12", PIT, &rush(12, 25.0)),
    honey("honey0", HONEY, 6.0), honey("honey1", HONEY, 6.03), honey("honey2", HONEY, 6.07), honey("honey3", HONEY, 6.12),
    honey("honey4", HONEY, 6.18), honey("honey5", HONEY, 6.25),
    honey("honeyw0", HONEY_POOL, 6.0), honey("honeyw1", HONEY_POOL, 6.03), honey("honeyw2", HONEY_POOL, 6.07),
    honey("honeyw3", HONEY_POOL, 6.12), honey("honeyw4", HONEY_POOL, 6.18), honey("honeyw5", HONEY_POOL, 6.25),
    // Start heights 0.05 apart over more than a tick's fall at the block, so some tick ends inside its top 1/16.
    honey("honeyt0", HONEY_ONE, 6.0), honey("honeyt1", HONEY_ONE, 6.05), honey("honeyt2", HONEY_ONE, 6.1),
    honey("honeyt3", HONEY_ONE, 6.15), honey("honeyt4", HONEY_ONE, 6.2), honey("honeyt5", HONEY_ONE, 6.25),
    honey("honeyt6", HONEY_ONE, 6.3), honey("honeyt7", HONEY_ONE, 6.35), honey("honeyt8", HONEY_ONE, 6.4),
    honey("honeyt9", HONEY_ONE, 6.45), honey("honeyt10", HONEY_ONE, 6.5), honey("honeyt11", HONEY_ONE, 6.55),
    drill("powder1", POWDER, POWDER_WALK), drill("powder2", POWDER, POWDER_WALK), drill("powder3", POWDER, POWDER_WALK),
    drill("powder4", POWDER, POWDER_WALK),
    // Falls of 2 (sinks in), 3 and 5 (lands a tick on the 0.9 slab), then boots on top and boots after a fall.
    powder_drop("powderfall2", 2.0), powder_drop("powderfall3", 3.0), powder_drop("powderfall5", 5.0),
    Drill { name: "powderboots", fills: POWDER_PIT, start: [2.5, 0.0, 0.5], settle: 10, steps: POWDER_BOOTS, pitch: 0.0 },
    powder_drop("powderbootsfall", 5.0),
    duck("duck0", 0.0), duck("duck1", 0.005), duck("duck2", -0.002), duck("duck3", -0.004), duck("duck4", -0.006),
    duck("duck5", -0.008), duck("duck6", -0.01), duck("duck7", -0.012), duck("duck8", -0.016), duck("duck9", -0.02),
    drill("pinland5", WALL3, &pinned_steps(5, false, 180.0)), drill("pinland1", WALL3, &pinned_steps(1, false, 180.0)),
    drill("pinland20", WALL3, &pinned_steps(20, false, 180.0)), drill("pinland5side", WALL3, &pinned_steps(5, false, 80.0)),
    // Clear of the plate (x+4.75..5): walking +z meets the collar at z+1.3125, a full cube at z+1; dropped, the
    // collar's top is at y+0.6875 (crates/acacia-world/tools/rules.mjs).
    Drill { name: "armwalk", fills: ARM, start: [4.35, 0.0, -0.5], settle: 30, steps: &[(30, 1.0, false, false, 90.0, false), STOP], pitch: 0.0 },
    Drill { name: "armdrop", fills: ARM, start: [4.35, 2.0, 1.5], settle: 30, steps: FALL, pitch: 0.0 },
    // Starved (food ≤ 6), the sprint key is ignored unless the player may fly.
    drill("hungrysprint", &[], &[(120, 0.0, false, false, 0.0, false), (40, 1.0, true, false, 0.0, false), STOP]),
    // A spectator flies through the wall, then sinks through the pad (no-clip).
    drill("spectatorwall", WALL3, &[(40, 1.0, false, false, 0.0, false), (20, 0.0, false, false, 0.0, true), STOP]),
    Drill { name: "crawlenter", fills: CRAWL_TUNNEL, start: [1.5, 0.0, 0.5], settle: 10, steps: &[(10, 0.0, false, false, 0.0, false), (40, 1.0, false, false, 0.0, false), STOP], pitch: 0.0 },
];
