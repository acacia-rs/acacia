//! Scripted movement drills on a server where the bot is operator: exact control sequences on a
//! flat pad (plus whatever blocks a drill needs), reporting server corrections per drill. Used to
//! pin down vanilla movement rules the simulation must follow.
//! `cargo run -p acacia-bot --example drills -- <server> <name|@account> [drills]`
//! `BEDROCK_RECORD=<file>` records a movement trace for the `replay` example.
mod support;

use acacia_bot::movement::Controls;
use acacia_bot::Bot;
use support::{corrections, Error};

/// (ticks, forward, sprint, jump, yaw offset from +X in degrees, sneak)
type Step = (u32, f32, bool, bool, f32, bool);
type Fill = support::Fill<'static>;

struct Drill {
    name: &'static str,
    fills: &'static [Fill],
    /// Feet start, offset from the origin block's corner.
    start: [f32; 3],
    /// Ticks to wait after building (flowing liquids need time).
    settle: u32,
    steps: &'static [Step],
    /// Look pitch for the whole drill (negative looks up).
    pitch: f32,
}

const WALL3: &[Fill] = &[([4, 0, -12], [4, 2, 12], "glass")];
const WALL2: &[Fill] = &[([4, 0, -12], [4, 1, 12], "glass")];
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
/// A water source, then a block placed and removed above it: BDS `/fill` schedules no liquid update.
const PUDDLE: &[Fill] = &[([8, 0, 0], [8, 0, 0], "water"), ([8, 1, 0], [8, 1, 0], "stone"), ([8, 1, 0], [8, 1, 0], "air")];
const FLAT: [f32; 3] = [0.5, 0.0, 0.5];
const STOP: Step = (20, 0.0, false, false, 0.0, false);
const FALL: &[Step] = &[(60, 0.0, false, false, 0.0, false)];
const IDLE: &[Step] = &[(40, 0.0, false, false, 0.0, false)];

const fn drill(name: &'static str, fills: &'static [Fill], steps: &'static [Step]) -> Drill {
    Drill { name, fills, start: FLAT, settle: 30, steps, pitch: 0.0 }
}

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

const SLIME: &[Fill] = &[([-1, -1, -1], [1, -1, 1], "slime")];
const BOUNCE: &[Step] = &[(100, 0.0, false, false, 0.0, false)];

/// Dropped onto a slime floor from `y`, then left to bounce out.
const fn slime(name: &'static str, y: f32) -> Drill {
    Drill { name, fills: SLIME, start: [0.5, y, 0.5], settle: 10, steps: BOUNCE, pitch: 0.0 }
}

/// Dropped from the source's height beside the column, `x` = feet x offset (box half-width 0.3).
const fn waterfall(name: &'static str, x: f32) -> Drill {
    Drill { name, fills: WATERFALL, start: [x, 8.0, 0.5], settle: 80, steps: FALL, pitch: 0.0 }
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
const fn pinned_steps(gap: u32, jump: bool, yaw: f32) -> [Step; 4] {
    [(40, 1.0, true, false, 0.0, false), (gap, 0.0, false, false, 0.0, false), (15, 1.0, true, jump, yaw, false), (20, 0.0, false, false, yaw, false)]
}

/// Sprint `ticks` at `yaw` towards the pit, then sneak on.
const fn rush(ticks: u32, yaw: f32) -> [Step; 3] {
    [(ticks, 1.0, true, false, yaw, false), (25, 1.0, false, false, yaw, true), STOP]
}

/// Plain walk/stop/sneak steps for the double-tap drills.
const fn walk(ticks: u32, forward: f32, sprint: bool, sneak: bool) -> Step {
    (ticks, forward, sprint, false, 0.0, sneak)
}

const DRILLS: &[Drill] = &[
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
    slime("slime2", 2.0),
    slime("slime4", 4.0),
    slime("slime6", 6.0),
    slime("slime9", 9.0),
    // Double tap around a sneak release (capture session 2 tick 1653; fuzz 220323-0 tick 1255).
    drill("dtplain", &[], &[walk(3, 1.0, false, false), walk(2, 0.0, false, false), walk(15, 1.0, false, false), STOP]),
    drill("dtsneakheld", &[], &[walk(10, 1.0, false, true), walk(3, 1.0, false, false), walk(2, 0.0, false, false), walk(15, 1.0, false, false), STOP]),
    // Sprint at the surface, then sink (WantDown) until the eyes go under: does a continuing sprint start a
    // swim like a held key does (fuzz 092535-0 tick 331)?
    pool("swimsinkkey", 2.5, &[walk(5, 1.0, true, false), walk(25, 1.0, true, true), STOP]),
    pool("swimsinknokey", 2.5, &[walk(5, 1.0, true, false), walk(25, 1.0, false, true), STOP]),
    // A step lasts one tick less than asked after a control change, so 3 here holds forward for 2.
    drill("dtsneakpress", &[], &[walk(10, -1.0, false, true), walk(3, 1.0, false, false), walk(4, -1.0, false, false), walk(15, 1.0, false, false), STOP]),
    drill("dtsneakkey", &[], &[walk(10, -1.0, false, true), walk(3, 1.0, true, false), walk(4, -1.0, true, false), walk(15, 1.0, false, false), STOP]),
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
    duck("duck0", 0.0), duck("duck1", 0.005), duck("duck2", -0.002), duck("duck3", -0.004), duck("duck4", -0.006),
    duck("duck5", -0.008), duck("duck6", -0.01), duck("duck7", -0.012), duck("duck8", -0.016), duck("duck9", -0.02),
    reswim("reswim5j", BUBBLES, &reswim_steps(5, true)), reswim("reswim5", BUBBLES, &reswim_steps(5, false)),
    reswim("reswim2j", BUBBLES, &reswim_steps(2, true)), reswim("reswim20j", BUBBLES, &reswim_steps(20, true)),
    reswim("reswimpool5j", POOL3, &reswim_steps(5, true)), reswim("reswimpool5", POOL3, &reswim_steps(5, false)),
    reswim("pin5j", POOL_WALL, &pinned_steps(5, true, 180.0)), reswim("pin5", POOL_WALL, &pinned_steps(5, false, 180.0)),
    reswim("pin5side", POOL_WALL, &pinned_steps(5, false, 80.0)), reswim("pin1", POOL_WALL, &pinned_steps(1, false, 180.0)),
    reswim("pin20", POOL_WALL, &pinned_steps(20, false, 180.0)), reswim("pin60", POOL_WALL, &pinned_steps(60, false, 180.0)),
    reswim("pinbub5j", BUBBLES_WALL, &pinned_steps(5, true, 180.0)), reswim("pinbub5", BUBBLES_WALL, &pinned_steps(5, false, 180.0)),
    drill("pinland5", WALL3, &pinned_steps(5, false, 180.0)), drill("pinland1", WALL3, &pinned_steps(1, false, 180.0)),
    drill("pinland20", WALL3, &pinned_steps(20, false, 180.0)), drill("pinland5side", WALL3, &pinned_steps(5, false, 80.0)),
];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).try_init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "@default".into());
    let only: Option<Vec<String>> = args.next().map(|l| l.split(',').map(str::to_owned).collect());
    let (mut bot, pad) = support::connect(&server, &name).await?;
    // Magma floors burn.
    pad.run(&bot, "/effect @s fire_resistance 3600 1 true");

    for drill in DRILLS {
        if only.as_ref().is_some_and(|o| !o.iter().any(|n| n == drill.name)) {
            continue;
        }
        bot.trace_mark(&format!("{} build", drill.name));
        pad.build(&mut bot, drill.fills, drill.settle).await?;
        pad.teleport(&mut bot, drill.start, -90.0).await?;
        bot.trace_mark(drill.name);
        if std::env::var("BEDROCK_COLUMN").is_ok() {
            println!("  column under start: {}", column(&bot));
        }
        let before = corrections(&bot);
        for &(ticks, forward, sprint, jump, yaw, sneak) in drill.steps {
            if let Some(c) = bot.controls() {
                // Bedrock yaw 0 faces +Z; -90 faces +X.
                *c = Controls { forward, sprint, jump, sneak, yaw: -90.0 + yaw, pitch: drill.pitch, ..Controls::default() };
            }
            bot.wait_ticks(ticks).await?;
        }
        bot.wait_ticks(10).await?;
        println!("{:12} corrections {}", drill.name, corrections(&bot) - before);
    }
    pad.go_home(&mut bot).await?;
    bot.disconnect().await;
    Ok(())
}

/// Our view of the blocks from the feet down six cells (debugging stale terrain).
fn column(bot: &Bot) -> String {
    use acacia_world::BlockAccess;
    let (Some([x, y, z]), Some(w)) = (bot.block_position(), bot.world()) else { return "?".into() };
    let (Some(view), Some(r)) = (w.view(), w.registry()) else { return "?".into() };
    (0..6)
        .map(|d| r.get(view.block(x, y - d, z)).map_or("?", |s| s.name.trim_start_matches("minecraft:")))
        .collect::<Vec<_>>()
        .join(" ")
}
