# Schematic builder (B1, B2)

`acacia_bot::build`: a `.mcstructure` placed block-exact by a [`Builder`] process (C2), every break and place
server-confirmed (C3). Roadmap: `baritone-roadmap.md` B1.

```rust
let mut job = BuildJob::new("house.mcstructure", Transform { origin, turns: 1, mirror: false });
job.stations = vec![barrel_a, barrel_b];
std::fs::write("job.json", job.to_json())?;          // all a restart needs
let builder = Builder::new(job)?;
let status = builder.status();                        // Arc<Mutex<BuildStatus>>, readable while it runs
let mut procs = Processes::new();
procs.add(builder, 10);
bot.run(&mut procs).await?;
```

## Decisions (user, 2026-10-17)

- Mismatched cells are cleared: anything differing from the structure, air cells included, is broken and
  replaced. `structure_void` is left alone. `BuildJob::fill_only` places into empty cells only (others: `kept`).
- Resume = re-survey. No progress log: only the job (structure path, transform, stations, options) is
  serialisable. Every tick the builder diffs the world against the structure, so a fresh builder after a restart
  carries on from what is there. Unloaded cells are `unknown` and visited.
- Block set (B1): full blocks, logs/pillars, slabs (both halves, double), stairs (facing, upside-down), facing
  blocks (furnace, observer, piston, dispenser), fences/walls/panes, glass, torches, ladders, buttons. Widened on
  2026-10-28, see "Block set" below. What is left is reported `unsupported` and skipped (`rules::DENIED`).

## Block set (2026-10-28)

Four families, each in its own rules module, measured on strict BDS with `examples/place_probe.rs SET=<tier>` and
accepted by a live scene (`tools/scenes.sh`). Each doc holds its rules table, evidence and what stays denied.

| Tier | Blocks | Planner change | Doc |
|---|---|---|---|
| oneclick | trapdoors, gates, chests, barrels, anvils, lanterns, chains, hoppers, rods, bells, workstations, redstone block/lamp/torch, leaves, lit furnaces (as unlit) | hanging cells wait for the block above | `builder/oneclick.md` |
| twocell | doors (hinge from neighbours), beds (colour from block entity), tall plants | `Need::Paired`; doors before the blocks that would flip their hinge | `builder/twocell.md` |
| interact | open bits, repeaters, comparators, levers, candles/pickles/eggs counts, signs with text | `Need::Adjust`: use or write instead of break; re-centre on a stand after a missed click | `builder/interact.md` |
| redstone | dust, rails (placement order), water/lava sources, plants, crops on tilled farmland | dry before wet; `Act::UseOn` for pours | `builder/redstone.md` |

User decisions (2026-10-28): block-entity contents (container items, banner patterns) are not built, except bed
colour and sign text; growth states and flowing liquid are not compared; natural leaves match persistent ones;
dirt matches grass under a solid target; mushrooms are denied (BDS refuses them in daylight); the kit carries a
hoe; a stand the bot reaches but cannot click from is re-centred on before another is tried.

## Placement rules (`build/rules/mod.rs`)

Measured on strict BDS 1.26.52.3 with `examples/place_probe.rs` (every item from four sides, every visible face,
clicked low and high; `VERT=1` for six-way blocks close above and below). Tests: `build/tests/rules.rs`.

| State | Decided by |
|---|---|
| `pillar_axis` | clicked face's axis |
| `vertical_half`, `upside_down_bit` | Down face or a side click above 0.5: top; else bottom |
| `weirdo_direction` (stairs) | player's horizontal look: east 0, west 1, south 2, north 3 |
| `cardinal_direction` (furnace) | towards the player |
| `torch_facing_direction` | Up: `top`; side: opposite of the clicked face; Down is not a hang (BDS picks a side) |
| `facing_direction` ladder, buttons | clicked face (ladders refuse Up/Down) |
| observer | look direction, vertical too (from close above: `down`) |
| piston | look direction horizontally, towards the player vertically |
| dispenser | towards the player, vertical too |
| six-way vertical | eye over 2 above the block's bottom, or below it, and within a block sideways (measured vertical at 1.0, horizontal at 1.9; between is avoided) |
| corner, connections, wall posts, powered/triggered/pressed bits | derived by the server: never compared |

A second slab clicked into a cell holding a half slab makes the double slab (whatever face). A double slab's
`vertical_half` is not compared. Clicks land only on a block's shape: full cubes anywhere, a half slab on its own
half of the side faces; panes, fences, walls, torches and other partial blocks are never clicked against
(`reach::points`). Interactive blocks are not clicked against either.

## Each tick (`build/plan.rs`)

1. Survey: each cell is Done, Place, Break, Drain (liquid where air belongs), Kept, Unsupported or Unknown.
2. Breaks and drains, top down. A liquid next to the cell (above or a side) that is not a solid target is sealed
   first with a scaffold block; seals outside the footprint stay (`status.seals`). Scaffold the pilot stands on in
   a cell where air belongs is left until the cleanup.
3. Places, bottom up, nearest first: first anything clickable from where the bot stands, then a walk to a stand
   for the lowest. A cell is never placed while the cell under it is still open (covering it takes away the
   click on the face below). A placement's state depends on the click and the player's facing, so a stand is a
   spot whose eye makes some click give the target (`reach::stands`, `reach::click_for`), with extra reach slack
   for the pilot stopping off-centre. Stands are keyed as the pathfinder keys standing spots: on a bottom slab
   the spot is the slab's own cell, feet 0.5 up (a cell above a bottom slab is no spot, and goals there were
   unreachable). Stands may be over air: the pilot pillars up (`allow_bridge`), never through cells the
   structure still has to fill, and the blocks it places arrive as `Event::Placed` and count as scaffold. A
   stand's feet and head stay out of cells the structure still fills (live: the bot stood in the unbuilt top of a
   ladder column and reached nothing from there). Stands are driven to with a 0.1 arrival tolerance: at the
   default 0.25 the body overlaps the cell next to the stand and the click is never allowed.
   Gravity blocks wait for their support; a cell with nothing clickable around it gets a scaffold column up to it.
4. Restock when the next placement's item is missing (`build/restock.rs`): everything the open placements still
   need minus the inventory, from the nearest station not yet found without it. A station that brought no more of
   an item is marked lacking it (cleared after 30 s of waiting).
5. Unknown cells: walk near. Then cleanup: scaffold where air belongs or outside the footprint, top down.
6. Done once every cell is right and no scaffold stands; the outcome is an error if cells were given up.

Failures: an act failing 4 times, a cell placed wrong more than 3 times, or 45 rests with nowhere to do it from
gives the cell up (`status.failed`, with the reason). A cell given up only for want of a stand is tried again
(up to 3 times) once nothing else can be done: the build around it changes what reaches it. A cell with no stand
rests 40 ticks so others go first. A drive the pilot gives up on (no route, a refused pillar) is not the cell's
failure: the stands it drove to are left out for 400 ticks and the task rests. The committed task
comes first in every rank and keeps its goal while open: re-picking the nearest each tick switched goals as the
bot moved, and every switch restarts the pilot (live: always moving, never arriving). A goal just reached that
still allows no click rests its task instead of being asked for again (the arbiter would hold still). In the air
the builder keeps driving the committed goal, so jumps and pillars finish.

`Builder::explain_for(bot)` lists each open placement with its held count, the click it has from where the bot
stands and the stands it could use: the first thing to read when a build stalls (`examples/build.rs` prints it).

## Evidence

- Sim (`pathfind/tests/build_tests.rs`, own BDS placement model in `build_sim.rs`): mixed 5×3×5 house with
  mismatches, restock by need, resume, support column cleanup, liquid seal; house16 (`--ignored`, release)
  block-exact in 2231 ticks.
- Live, strict BDS 1.26.52.3, fresh world, test pack off (`examples/build.rs`, 2026-10-17): house16 from two
  barrels, junk in the footprint, the bot disconnected after 150 placements and a fresh one resumed from the
  saved job. Observer diff 0 cells; 588 placed, 29 broken, 4 restocks, 32 corrections, 2357 ticks (512 s).
- An offline bot that reconnects to BDS comes back with an empty inventory (position kept): what the killed bot
  carried is lost, so the live stations hold the house twice over.
- Swarm sim (`pathfind/tests/swarm_sim.rs`, bots ticked in turn on one grid, each with its own inventory):
  house16 by two bots in 976 ticks against one bot's 2231 (0.44×), block-exact, no scaffold left; a bot stopped
  at tick 80 has its tile taken over after the lease and the other finishes; two bots restocking a 16×16 floor
  from a barrel holding exactly the floor both build, neither holding more than two stacks. Board rules:
  `build/tests/board.rs`.
- The B1 pass above needed its restart: without one, house16 failed on the last junk cell broken before the
  placing ([2, 0, 2], "placed wrong"), also at B1's commit. BDS kept destroying that cell (planks gone every
  3.3 s, their break time without an axe) because breaks never ended; every break now ends with `AbortBreak`
  (`interact::wire::break_step`; `StopBreak` does not end it: junk scene, Stop failed, Abort passed).
- Live scenes (`examples/build.rs` `SCENE=`, seconds each, regression checks; fresh world, stock 20 TPS):
  `wall` (wall with log ends and slab rim) PASS 46 s; `junk` (floor under a furnace where junk stood) PASS 43 s
  with Abort; `ladder` (wall over a floor with a ladder column) PASS 75 s (843 ticks before the 0.1 stand
  tolerance, 2079 with the stand rule alone; 230 after), also at 2× TPS (`ACACIA_TICK_SCALE=2`).
- Live house16 one bot, no restart (`examples/swarm_build.rs BOTS=1`): PASS, 454 s, observer diff 0.
- Live house16 two bots (claims and home bands, 2026-10-21): PASS each time, 125/108/118 s against one bot's
  184/193 s (0.56-0.68, mean 0.62): the 0.6 bar is met in one run of three. Kill run (B dropped after 150
  placements, never back): PASS 180 s, diff 0. Earlier with 4×4 tiles: 266-298 s against 454 s (mean 0.63).
- Live house16 after the speed round below (2026-10-21, `tools/build-ab.sh 4 one:BOTS=1 two:BOTS=2`): 8/8
  PASS, one bot 157.5 s (sd 12), two bots 96.8 s (sd 9), 0.61.

## Measuring (2026-10-21)

- Journals: each builder writes `journal/<bot>.jsonl` (`build::journal`: a line per tick with what it wanted,
  why it held, the cell and what it shows, claims; acts, walks, rests). The examples print `build::findings` at
  the end: time by hold reason, walk statistics, and named findings (`DOUBLE_PLACE`, `LONG_HOLD`,
  `STALE_CLAIM`, `REST_LOOP`, `WALK_NO_ACT`, `GOAL_THRASH`, `WALK_STUCK`) with every wrong cell's history.
- Scenes: `tools/scenes.sh <scene>...` builds once and runs every scene at once, each on a fresh world of a leased
  BDS from `tools/bds-pool.sh` (`up <n>` copies a configured strict BDS; ports 19400+2i). ~20 s a scene.
- A/B: `tools/build-ab.sh <runs> name:ENV=v,...` builds once on testbox and runs every run at once on pool BDS,
  variants interleaved, as one detached job (a local reap can't stop it). testbox is this host since 2026-10-24:
  every other session shares the CPU, so interleave and check `uptime`. Live spread is
  ±7-15 s per run; 3-4 runs per side are the least to trust a 10% change. `BUILD_SEARCHES` and `BUILD_REACH`
  stay as knobs for later A/Bs.
- What it found and changed, each judged live:
  - The pilot stood a tick on every new path (searched with the keys up, drove the next tick): it now drives
    in the tick its quick search lands; one bot's searching time 24 s -> 16 s.
  - Scaffold dirt broke at the hand's ~780 ms (the kit had no shovel): the examples give a shovel.
  - Stands scored for 24 ready cells, not 6 (one bot 195 -> 178 s, two 122 -> 108 s); reach 5.5, not 4.5 (BDS
    kept clicks 6.7 off, `examples/place_rate.rs`; one bot 161 -> 146 s, two 105 -> 94 s).
  - A bot could end in the outer half of a window cell (a pane beside its body, the cell solid to the planner)
    and fail every walk "Stuck 10 times" until the run ended (live 2 of ~12 one-bot runs, sim once): moves no
    longer end with the body in a thin target's cell (`SearchOpts::no_body`), and after two failed walks a thin
    block in the body's cells is broken (rebuilt later).
  - Each bot posts its feet every tick, not every 20 (sim: the other bot placed into a body a second stale).
  - Every act fails as timed out after 10 s (one live act hung 820 s until the watchdog; cause not found).
  - The live pad resets below the build limit (bots rejoin on the last pad; at y 340 nothing placed).
- Double slabs take two placements: `DOUBLE_PLACE` counts against the cell's target.
- The sims (`pathfind/tests/build_sim.rs`) hold a bot for an act's live ticks (place 2, break 4, restock 4),
  answer background searches 6 ticks late, and with `World::delay` (`LIVE_DELAY` = 3 in the two-bot house test)
  a bot sees other bots' blocks late; the two-bot house test also journals both bots and fails on
  `DOUBLE_PLACE`, `STALE_CLAIM` and `WALK_STUCK`. Sim house16: one bot ~105 s, two ~60 s (0.54-0.64 run to run)
  against live 157.5 / 96.8 s: moving and acting each still ~35% short (path work, ~11 s live, is left out:
  holding the bot for it broke pillars in the sim). It ranked reach 4.5 against 5.5 as live did but by 33%
  against ~10% for one bot: trust it for bugs and direction, the live A/B for size.
- Where a placement's time goes (`act_timing`, printed by the examples; one bot, house16, 470 s): acts 323 s,
  of them equip with a hotbar switch 198 s (235 switches, mean 842 ms: the human equip pause), echo wait 79 s
  (mean 120 ms), the tick after each act 31 s, aim 28 s, mining 24 s; planning CPU 3 s.
- Switches without the container-click pause (Human: a 50-200 ms key press; Optimal: none), held item ranked
  first, no clicks onto interactive blocks: one bot 271 s (-42%), switch time 2.9 s (191 switches), retries 3
  (were 42: a pillar started on a station barrel, whose click opens it). Ladder scene 55 s (was 74).
- Pipelined placements (Optimal; `interact::pipeline`, governor cap 4 in flight; BDS kept 6 a tick from one
  spot, `examples/place_rate.rs`): one bot 213 s, acts 62 s (p50 49 ms), echo waits overlapped; sim
  `pipeline_sim` (refused, lost and late echoes, no ghosts, never done with placements in flight).

## Swarm (B2: `build/board.rs`, `build/swarm.rs`)

Several builders share one job through a `TaskBoard` (`Builder::with_board(board, name)`; without one a builder
is exactly B1). `LocalBoard` serves bots in one process; a networked board can implement the same trait.

- Claims: each tick a bot claims the cells of the step it planned (every ready cell its chosen stand reaches,
  or the cell it acts on) in place of its last claim, plus each cell it acted on in the last 40 ticks (the
  others see a block only once the server sends it: live, two slabs went into one cell). Cells another bot
  holds are not planned; pillars stay a block away from them. A bot that holds still claims nothing else (a
  hold that kept its old claim stalled both bots for good, live).
- Home bands: the footprint is cut along its longer side into one band per live bot (board rank by name),
  each with as many blocks to place; work outside a bot's band costs 10 blocks of travel, so it goes there
  only when home has nothing near, and a dropped bot's band folds into the others. Replaced exclusive tiles
  (4×4: one bot idled through the tail while the other finished its own; 0.59-0.71) and repelling from the
  other bots' feet (the areas moved with the bots and overlapped: 133 walks of ~1.05 s against one bot's 103
  of 0.85 s).
- Leases: a builder joins the board when given it and heartbeats every 20 ticks with its feet; a lease lapses
  after the board's lease time without one (live: 10 s; vanilla-tick time under `ACACIA_TICK_SCALE`), and its
  claim and band go back to the pool. The board stores no progress: the next bot surveys the cells, so a dropped
  bot's half-done work needs nothing special. A bot finishes only when the whole job is done and no scaffold stands.
- Stations: a bot restocks its share of the job's remaining need (the need over the bots on the board: two bots
  each taking the whole need would empty the barrel, and with 4×4 tiles they walked back after nearly every
  tile), and one bot at a time opens a station (`open_station`, held under the same lease).
- Room: stands within 1.5 blocks of another bot are not used and no block goes into another bot's body.
  Scaffold is shared through the board; any bot not kept off it by a claim removes it.
  The pilot itself does not see other bots: its terrain memo, rollout cache and sim-verified macro moves are
  keyed on blocks only, so moving obstacles there is not a small change.

## Open (2026-10-21)

The two-bot ratio: 0.61 live (8 runs). A full in-flight cap holds a pipelined placement for the tick instead of
waiting inside it (`process::run::pacing`): waiting there never ended, since in-flight placements are decided
only at the start of a tick (live: the bot went silent mid-build).

- Next (`research/build-iteration-tooling.md`, `build-walk-overhead.md`, `build-throughput-deep.md`): short swarm
  scenes per bug with injected lag; the sim's path-work cost (left out, see "Measuring"). Acting is the largest
  share (~2 ticks a placement).

Tried and kept off (2026-10-21): placing while walking (`BUILD_ENROUTE=1`; `process::run::en_route`,
`build/en_route.rs`). On a plain walking step on the ground, a ready cell whose state needs no particular look
is aimed at (keys remapped to keep the course, no sprint), clicked the next tick, then two ticks only walk.
Live A/B, 4 runs each, all PASS: one bot 159.8 s against 148.8, two 91.8 against 106 (one 141 s outlier; ~94
without). Only 12-63 cells a run went in en route against the model's ~98: most steps are macro runs and
leaps (journal tally: Run 370, Parkour 249, Walk 103 of the refusals), and offering on those dropped the
cells walked past (a first version placed 116 en route and saved nothing). To retry: clicks on run macros
(their rollouts would have to include the remapped keys).
- Stands (`build/cover.rs`): the stand reaching the most ready cells for its travel, not the stand of the
  first cell. Live one bot it was neutral (200 s against 187-196); it gives the swarm its claim batches.
- Path work keeps out of built cells (`SearchOpts::keep`; sim: a pillar through a torch's cell replaced it
  again and again). Keeping out of every block target cost one bot 237-267 s against ~190: paths walked round
  unbuilt floor instead of over scaffold there.

Tried and dropped (2026-10-19): faster placement pacing. Clicking in the turn's own tick, a second placement in
the same tick, and no tick after a placement: with any two of the three, BDS silently ignored ~15% of placements
(no echo, no refusal; house16 retries 70-125 against 1-5), and one of them alone still left a mismatch. The
standing-still probe (`examples/place_rate.rs`, 6 a tick kept) did not catch it. Line-of-sight checks and
treating unconfirmed blocks as no support, added on the theory that hidden clicks were dropped, did not change
the rate and cost ~65 s of walking (stands farther away); the drops follow the pacing, not the view.

## Not in B1/B2

- Portable shulker restock (place, take, break, pick up); stations must be placed containers.
- Crafting missing items; dumping junk when the inventory is full (drops from clearing are picked up).
- Line of sight: clicks only need the face to point towards the eye.
- Other bots as pilot obstacles; a networked board.
