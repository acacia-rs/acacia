# Pathfind: Baritone-style planner

Branch `pathfind/parkour`. Prior art (ideas only, MIT, no code copied): `../research/mc-zuri-pathfinder.md`.
Paths below are under `crates/acacia-bot/src/` unless marked.

## Decisions (user, 2026-10-13)

- Extend the existing A* (`pathfind/astar.rs`), keep Baritone tick costs and existing moves/tests.
  Add mode-tagged nodes and **macro moves** verified by forking `acacia-physics` `PlayerState`.
  Planner and follower share one controller per move; follower re-verifies every tick and
  replans in place on divergence.
- Milestones: 1 ground speed & parkour → 2 elytra + fireworks → 3 boats → 4 entity avoidance.
- `MovementProfile {Optimal, Human}` drives planner and follower. Human applies everywhere
  (walk steering too): eased/jittered aim, plans only moves that land under timing/aim
  perturbations with an edge margin.
- Heuristic stays admissible: per-profile ticks-per-block measured from steady hop speed when
  macros are on, 3.563 otherwise.
- Flat-ground hop chains on by default; dropped when food ≤ 6. Gap leaps unaffected.
- Human aim constants are placeholders in `human.rs` until a vanilla capture fits them.
- Neo/ladder jumps: stretch at the end of M1, slip to M2 if they stall.
- 2026-10-14, after `../research/pathfinder-survey.md` (Tungsten/unionclef, azalea, ParkourCalculator,
  SwarmBot): nodes are momentum-aware; follower gets an any-angle shortcut; leaps search takeoff
  yaw/point and in-air correction with an obstruction prefilter.
- Done bar for M1: headless sim tests per move under both profiles; parkour legs in
  `examples/course.rs` with 0 corrections on a dedicated strict testbox BDS; ticks-to-goal
  comparison vs macros off; `trace_regression` green.

## M1 design

### Node state (`pathfind/node.rs`)
- `Mode: u8` (`Ground` now; `Glide`, `Boat` later). `Entry: Rest | Run(dir8, speed_bin) | Air` (Air: start only).
  `speed_bin` (horizontal speed, ~4 bins) keeps a faster later arrival from being pruned by a slower
  earlier one; if bins prove too coarse, keep a small non-dominated set per key instead.
- `NodeKey {pos, mode, entry}` replaces `BlockPos` as the A* key (`astar.rs` `Rec`, `PosMap`).
- `Momentum {off:[i16;3] (1/1024 blk), vel:[i16;3] (1/4096 blk/t), yaw_q, flags}`;
  `snap(&PlayerState)` / `synth(pos, Momentum) -> PlayerState`. Stored per `Rec`.
- Classic and alter edges yield `Rest`; macro edges need `made == 0` at both ends.
- `PathNode` gains `entry` and `mac: Option<Macro>`; `MoveKind` gains `Run`, `Hop`; gap leaps stay `Parkour`.
- As built (steps 1–2): `Momentum::snap(st, at)` / `synth(at, base)`; `base` (live state) supplies
  attribute, freeze, effects, equipment, flight; flags = on_ground, sprinting, sneaking, pressing_jump,
  jump_delay. Fidelity held (worst 0.0013 blk over 45 snaps × 30 ticks), no full-`PlayerState` fallback.
  `Run(dir, bin)` bins via `node::speed_bin` (edges placeholder until step 6). `Momentum` per `Rec` lands
  with the first macro edge (step 4).

### Shared sim
- `movement/keys.rs`: `KeyLatch` lifted from `Movement::tick` (double-tap, sprint latch, wire
  rotation) — the only Controls→Input mapping. `Movement::fork() -> Rollout`.
- `world/adapter.rs`: extract `block_physics`; `pathfind/physview.rs`: `WorldView for Terrain<B>`
  (unloaded = `Blocks::ids` → None). `Cell` gains `liquid`.
- `pathfind/rollout.rs`: `Rollout {st, keys}`, `step`, `run(ctl, max) -> Trace {bodies, landed, ticks, min_margin}`.
- As built: `Rollout {st, keys, using_item}` + `step` live in `movement/fork.rs` (keeps pathfind →
  movement one-way); `rollout.rs` has `Body`, `run(r, ctl, c, max, w)`, `peek`. `min_margin` comes with
  leap acceptance (step 4).

### Controllers (`pathfind/control.rs`, `pathfind/profile.rs`)
- `trait Controller { fn step(&mut self, b:&Body, peek:&mut dyn FnMut(&Controls)->Body, c:&mut Controls) -> Phase }`.
- `RunCtl {dir, cells}`; `LeapCtl {target, jump: AtEdge|AtTick(k)|AtAlong(x), coast, aim}`; `HopCtl`.
- `Aim`: Optimal snaps; Human eases with capped rate + splitmix jitter.
  `profile.perturbations()`: Optimal `[nominal]`; Human nominal, jump ±1 tick, yaw ±jitter, start ±0.05.

### Macro moves (`pathfind/leap.rs`, `pathfind/hop.rs`)
- Leap from `Stand && !wet`: cardinal 2–5, diagonals (2,1)(2,2)(3,1)(3,2)(4,1); dy +1, 0, −1..−3
  (deeper only into water). Geometric prefilter (corridor `body_clear`, ≥1 gap cell) then
  jump-timing search (AtEdge/AtAlong, AtTick 0..6) × coast {None,3,5}; fewest ticks wins.
- Before timing search: SwarmBot-style obstruction prefilter (drop targets whose straight corridor
  is blocked). Timing search also tries takeoff yaws (±11.25°, ±22.5° off the target line), a free
  takeoff point along the last block (ParkourCalculator-style), and an in-air yaw correction.
- `Run(d, 1..3)` only before a gap or on lanes ≥4; `Hop` only from `Run(d)` along d.
- Accept: on_ground, feet within 1e-3 of spot, xz in target cell, no AVOID/danger touch, no
  pre-apex h-collision, ≤40 ticks. Human: all perturbations land in the cell, edge margin ≥0.15.
- Cost: simulated ticks (worst perturbation for Human) + `DANGER_NEAR`; Human + `HUMAN_JUMP` per
  jump + `RISK*(1-margin/0.3)`. Classic `gap_jump` only when `!macros`.
- As built (step 4): `macros.rs` (successors), `solve.rs` (timing search, shared with the follower), `leap.rs`
  (`LeapCtl`). Cardinal reach 7, not 5: from rest a 4-gap needs a 2-block run-up inside the leap (a jump from the
  edge block falls short). Prefilter = walk the lane, a blocked cell shadows the rest. Timings: `Tick(0..=12)`
  deduped by actual takeoff tick, coasts 8..0 only on overshoot; candidates judged fastest-first. Human = every
  perturbation lands, hazard pad 0.25, no absolute 0.15 margin (survey rec 5); Human refuses 4-gaps. Search starts
  from the live state (`search_from`); test `opts()` keeps macros off so the classic suite still runs.
- As built (step 5): targets in `leap_moves.rs` (all 8 symmetries of the diagonals; dry dy +1..−3, first water
  below up to 16). Solve = earliest takeoff that lands wins (air time barely varies), coast by binary search;
  `Never` (run off the edge) for drops — a jumped down-2 falls 3.25 and is rejected (fall > 3). Takeoff yaws
  ±11.25/±22.5 + air aim only when a diagonal misses straight. Head-hitters: any ceiling contact rejects.
  Hazard pad is sideways and up, not below the feet (lava level with the floor is cleared).
- As built (step 6): `hop.rs`. `Run` = `RunCtl` one cell onto a lane ≥4, or up to a gap's edge (≤3); from
  `Run(d)` only along d. `Hop` = `LeapCtl` jumping at once along d, landing cell found by a free rollout, and
  `slack` (coming down beside the cell on the level counts). A straight Walk from `Run(d)` along d keeps the
  momentum (no free stop). Optimal also gets perturbations (start ±0.01) so quantized momentum (≤0.0013 drift)
  cannot flip a pixel-perfect plan. Margin counts only sides with no floor beyond. Followers re-solve a leap
  that would arrive >0.03 b/t off the planned velocity, picking the landing nearest it (keeps chains on plan).
- Measured: steady hop 0.35633 b/t (12-tick hops) → heuristic 2.80 t/b Optimal, 3.27 Human
  (`costs::macro_ticks_per_block`); plus a startup term from rest, min(4, (3.5638−tpb)·d), checked against
  sprints from rest. Speed bins [0.12, 0.25, 0.30] (walk 0.216, sprint 0.281, hop landing 0.27–0.29).
  Human hops rarely pay (HUMAN_JUMP 2 vs 15.2 classic ticks per 4.28 blocks): Human plans classic on short runs.

### Cache and budget (`pathfind/cache.rs`)
- Key `(family, params, dir, Momentum, corridor signature hash)`; caches failures too.
- `SearchOpts` gains `profile`, `macros`, `hops`, `max_rollout_ticks` (400k); spent → classic only.
- As built (step 7): no corridor hash in the key. Key = (Momentum, family params); each result stores the
  cells its computation read (`Terrain::record`, `memo.rs`) as relative (pos, block+liquid id), reused where
  all read the same (≤32 per key, failing read moved first). Results stored relative, shifted in f32 (≤ float
  rounding off a fresh rollout). `Navigator` keeps one `RolloutCache` across re-plans (dropped on another
  profile/player state, or past 50k results). Quick inline search: 1000 rollout ticks; a plan cut by the
  budget goes to `spawn_blocking`. Budget checked between moves (one solve may overrun).
- Pruning: edge jump first, short ⇒ whole yaw short; else bisect the first reaching takeoff (reused); dry
  landing not below takeoff over >4 gap cells in a row is skipped (no jump/speed effect); no leaps back
  (dot ≤ 0) from a running node (≥0.12 b/t); wide retry only after `Miss::Other`; a dry leap fails once
  falling below its target's floor; last-failing perturbation judged first. `Human` drops hops no faster
  than sprinting to the landing (+`HUMAN_JUMP`): it never chose hop chains (flat 5–60 blocks), and these
  were most of its refusal cost. Physics per tick also got a cell front cache and memoized `block_physics`.
- Measured (release, testbox at load ~24/12 cores, best of 5, `tests::perf::perf_report`): flat 80 Optimal
  56→24 ms, Human 2498→51 ms (path found); gap course Optimal 26→11 ms, Human 227→70 ms; 5-gap refusal
  Human 297→35 ms; diagonal 30×11 313→29 / 131→5 ms. Re-plans: 1–20 ms. Gap-course Human misses 30 ms:
  11.5k simulated ticks at ~5 µs (near-gap results do not translate, 59% hits); refusal misses 20 ms on
  430 nodes' cache checks. ~8% of rollout time is libm `floorf`/`ceilf` calls (baseline x86-64 target).
- Known issue (item 7): classic costs ignore acceleration, but plans under-run by a near-constant 8–11
  ticks classic and 6–12 macro (5–60 blocks, `tests::perf::cost_skew_report`): no measurable skew, so
  classic costs are not calibrated.

### Follower (`pathfind/follow_macro.rs`)
- `Follower::tick(&mut self, s, live: Option<Rollout>, w: &dyn WorldView, c)`; hands off to
  `Exec {ctl, predicted, t, target}` for macro nodes.
- Per tick compare live `Body` to `predicted[t]` (Optimal 1e-4, Human 0.03); on mismatch
  re-predict from live, retry other timings, else `FollowStatus::Diverged`.
- Navigator on `Diverged`: no stop, not counted in `max_replans` (cap 5 consecutive); hold
  controls while airborne; inline quick search from live `Momentum` (Run entry).
- Invariant test: Optimal executed ticks == predicted.
- As built (step 3): `tick(s, live: Option<&Rollout>, w, c)`; `Exec` sits in `MacroRunner`; "retry other
  timings" waits for the step-4 timing search (TODO in `follow_macro.rs`); the in-place re-plan searches
  from the live position (Rest) until Run entries exist (TODO in `goto.rs`). `LeapCtl` ignores air before
  its first ground contact. A full sprint jump overshoots a cell-exact landing, so coast/timing search
  is required, not optional (step-3 test picks the coast by rollout).
- As built (step 4): a leap that does not finish from the live state is solved again (`resolve`: fresh timing
  search before takeoff, coasts once airborne), at start and on drift; `MacroReport.resolved` marks it. A
  finished macro hands over to the next on the same tick. The classic follower sprints into a macro heading
  the same way (run-up). Navigator always plans from the live state, so a re-plan keeps the momentum.

## M1 steps (one commit each, tests with each)

0. Refactor only: `KeyLatch`, `block_physics`, `Movement::fork`, `Cell.liquid`. Gate: `trace_regression` + pathfind tests unchanged.
1. `physview` + `rollout` + `Momentum`. Tests: Movement vs Rollout bit-exact 200 ticks; snap/synth within 1/512 over 30 ticks.
2. `NodeKey`/`Entry` in A* (Rest only); existing tests green.
3. Controller trait, `profile.rs`, `follow_macro.rs`, `Diverged` in navigator. Tests: hand-built leap path predicted == executed; forced perturbation → replan without stopping.
4. Flat cardinal Leap 1–4 + timing search; Human detours around pixel-perfect gaps.
5. Height-changing and diagonal leaps; head-hitter rejected; lava gap margin.
6. `Run`/`Hop` + `Run(d)` entries; stepping stones chain with no Rest between; measure hop speed, switch heuristic; hunger gate.
6b. Any-angle follower shortcut (azalea-style): on Ground segments, simulate sprint / sprint-jump
    straight at nodes up to ~20 ahead, take the fastest clean rollout that reaches the path; removes
    8-direction staircasing. Test: 30-block diagonal ticks drop vs 6; Optimal predicted == executed.
7. Cache + rollout budget; perf guard (flat 100×100, 80 blocks, ≤2× macros off). As built: see "Cache and
   budget"; guard = rollout/hit-rate bounds per case (`tests::cache`), not timings.
   As built: `shortcut.rs` (`ShortcutCtl`, `find`), tried once per node index with nothing running; spans of
   flat Walk/Run/Hop nodes at the bot's level; k ∈ {20,15,10,5,4,3,2}, styles {sprint, sprint-jump until
   1 or 3 blocks}; taken when its ticks beat the span's polyline at sprint (hop) speed. Runs as an `Exec` like
   any macro (same tolerance, same report). `Macro` moved to `mac.rs` (control ↔ leap/shortcut cycle).
   `GotoOpts::shortcuts` (default on); `Follower::shortcuts(profile)`.
8. Human complete: aim on all segments incl. walk, perturbations, risk cost; 5 seeds, 0 Diverged failures.
   As built: `Follower::moving(profile, seed)` (Navigator seeds from `cadence::seed`); classic steering and the
   work approach turn through the follower's `Aim`; each live macro runs `reseeded` on its own jitter stream (plan
   stays on `AIM_SEED`), so Human execution differs from the plan and leans on the perturbation sets. Final-node
   settling stops walking while the eased aim is >30° off (it circled the goal). Tests: `tests::drive`
   `plan_and_follow`/`seeds` — Optimal once, Human seeds 1–5, every step 4–6b test, all with 0 Diverged.
   Live re-solve budget: `follow::ROLLOUT_BUDGET` 1500 rollout ticks per follower tick (`Spend`, shortcut search
   and re-solves share it, a rollout only starts if its cap fits); `retry` = takeoff bisection + coast search
   (`Solver::first_reaching`/`land`), other coasts only to match velocity, stop within 0.03 b/t. Out of budget →
   `Diverged` with `spend().short` → Navigator re-plans in the background only (no inline search that tick).
   Not cached: live states are unquantized and the retry ranks by planned velocity, so `RolloutCache` keys do
   not fit; the re-plan after it uses the cache. Max per-tick spend over the suite 2443 → 1271. Water landings
   (Rest entry) no longer re-solve for an arrival velocity they never planned. `off_path` also accepts the next
   segment (a classic gap jump that carries past its node). Bridging maneuvers (`maneuver.rs`) still snap.
9. `tests/speed.rs`: ticks-to-goal macros on vs off (flat, diagonal, gaps, stairs); on ≤ off.
   As built: `src/pathfind/tests/speed.rs` (needs the test grid), shortcuts off and on, Human = mean of 5 seeds.
   Gap course = 1-gap + 2-gap (classic can do both). Measured saved ticks (off → on), shortcuts off: flat 40
   Optimal 154→127, Human 154→154; diagonal 30×11 149→117 / 149.6→144.2; gaps 109→81 / 115.4→94.4; stairs
   145→141 / 145→145. Shortcuts on: flat 126→127 / 126→126; diagonal 122→111 / 130.4→111; gaps 100→81 /
   107.8→88.4; stairs 137→140 / 141→141. Strict gains asserted: flat Optimal, gaps both (shortcuts off ≥20/15,
   on ≥12). Deviations: Human gains nothing on flat (hops do not pay, step 6). Shortcut tweaks: rollout cap by
   distance (4 t/b + 20, a hop node spans ~4 blocks) and a from-rest startup term (`MACRO_STARTUP`) in the
   take-it comparison.
   Fixed after: with shortcuts on, Optimal flat (126→127) and stairs (137→140) lost to macros off. Not the plan:
   (a) the take-it comparison charged a whole span at hop speed when any node in it was a hop, so on stairs the
   shortcut stopped short of the stair foot and the bot walked the rest (sprint dropped); (b) the fixed
   `AHEAD` steps never targeted the last flat node of a short macro path; (c) final-node settling released
   forward from a friction estimate, overshot the centre, turned back and walked. Now `PathNode::cost` carries
   each edge's planned cost; a span costs its macro nodes' simulated ticks plus walked segments at sprint
   speed (+ startup from standing); the span's last node is tried first; settling releases once a simulated
   coast (`Follower::coast_end`) ends inside the tolerance. Shortcuts on (off → on): flat 125→124 / 125→125,
   diagonal 116→111 / 125→113.4, gaps 94→82 / 101.6→87, stairs 137→137 / 142.4→142.4. With the held
   shortcut heading of step 10: diagonal 120→108 / 123.4→114.2, gaps 94→82 / 101.2→87, stairs 138→138 /
   143→143, flat unchanged. Asserted: on ≤ off
   everywhere, gaps on ≥10 with shortcuts. The test takes ~150 s in debug on testbox, so it is
   `#[ignore]`d (default `--lib` 22 s): `cargo test --release -p acacia-bot --lib macros_never_slow -- --ignored --nocapture`.
10. `examples/course.rs` parkour legs (gap3, gap4, up1, down2, diag, stones, sprint40), `--profile`, `--macros off`; dedicated strict testbox BDS, 0 corrections both profiles; evidence in `docs/DESIGN.md`.
   As built: legs in `examples/course/legs.rs`; lane walled 11 wide from x 48 to 95 with a flat corridor
   (z 4..5) beside every obstacle, so profiles that cannot leap walk round; that section is built on reaching
   it (a `/fill` touching an unloaded chunk fails whole). Flags also `--shortcuts off`, `--seed`, `--lift`
   (levitation mid-leap: the live `Diverged` path). Navigator gained `GotoOpts::seed` and `divergences()`.
   Deviations: Optimal 0 corrections on every leg, Human 0 on the parkour legs but 2–9 on sprint40 (and a
   few on classic legs, macros off too): strict Linux BDS corrects one-ulp position differences off the axes,
   not caused by the per-tick yaw change itself; open (DESIGN.md "Movement vs the servers"). Shortcuts now hold their heading once faced, which removed them for
   Optimal. Live, macros on lost to off on gap3 (36 vs 32: the corridor detour beats the leap with its
   run-up) and sprint40 (161 vs 147); not chased. A divergence in mid-air finds no start node and spends
   one counted re-plan before the next search succeeds.
   Fixed after: on a fresh world Human timed out in a pit (sprint40 every seed, later diag and gap4 too). Cause:
   the default rollout budget (400k ticks) takes ~1.7 s, so on the loaded testbox the 2 s search ran out of time
   and the best partial path leapt or walked into a pit bottom, a dead end (seen in navigator logs for diag and gap4; the
   sprint40 trace walks into the pit at z ≈ 2.9, too far in for an edge graze, so most likely the same). Rollouts now stop at half the time budget (`macros::MACRO_TIME_SHARE`) and
   the classic rest completes. Two Human weaknesses found on the way: a shortcut could pass the diag pit's corner
   with its centre over air (box overlap 0.03), so Human shortcuts keep the feet centre over the floor
   (`MovementProfile::centred_shortcuts`); and classic steering walked an arc while the eased aim turned round,
   off the stones pit, so it stops walking while >30° off on every node, not only the last. Tests (release,
   ignored): `tests::course` (the parkour section in the sim, both profiles, Human seeds 1–5; plans complete
   with a 1 s budget: 8 of 14 cut short before) and `shortcuts::human_shortcuts_keep_off_pit_edges` (default run).
   Live (fresh world per run): all legs ok, Optimal 0 corrections, Human seeds 1–3 bursts as above.
11. Stretch: neo jumps (Optimal), ladder jumps.

## C1 design (2026-10-15): continuous long gotos

- Core moves out of the async `Navigator` into a network-free `Pilot` (`pathfind/pilot.rs`) driven with a
  `Movement`, a `Terrain` and a search spawner; the navigator only adds work (`execute`) and the tick wait. The
  sim drives the same `Pilot`, so joins, late splices and backoff are tested offline.
- Searches run off the tick as a `SearchJob` (`pathfind/job.rs`) answered on a `tokio::sync::oneshot`; the
  navigator spawns them with `spawn_blocking` on a `WorldBlocks` snapshot, tests inline with an optional delay.
- Plan ahead (`pathfind/ahead.rs`): on a partial path, once the planned ticks left (`PathNode::cost` sum) drop
  below `PLAN_AHEAD_TICKS` (100, Baritone's lookahead is 150), search from the last node's planned momentum
  (`Momentum::synth`). Splice = `Follower::splice` appends the nodes, so the follower never sees a final node at
  the join. A result is dropped when the follower was replaced or spliced since (generation check). Arriving at the
  partial end with the search still running: hold there and wait for it (it starts from here); a failed or empty
  one falls back to a plan from the live state.
- Favouring (`pathfind/favour.rs`): a re-plan after Stuck/OffPath/Diverged/world change favours the dropped
  path's cells: an edge into one costs `floor + 0.5·(cost − floor)`, `floor` = the heuristic between its ends
  (non-rest). Each edge stays ≥ its heuristic floor, so h stays a lower bound except the from-rest startup term
  (≤ `MACRO_STARTUP` = 4 ticks): bounded suboptimality, accepted. Not applied to plan-ahead (old cells are behind).
  `PathNode::cost`/`Path::cost` stay undiscounted (shortcuts compare them with simulated ticks).
- Dead ends (`pathfind/exit.rs`): a partial path ends on its candidate's deepest ancestor from which a classic
  flood (same dig/bridge/doors options, no macros) reaches `EXIT_NODES` (64) distinct cells; a 3-deep pit fails it.
  Heuristic, documented: a one-way drop into a ≥64-cell basin passes.
- Unreachable: `Path::unreachable` = open set exhausted and no unloaded cell read. The pilot fails at once
  (`NotPossible`) on it; with dig/bridge allowed the search itself plans the way out. An empty partial path (no
  progress, e.g. at the edge of loaded terrain) counts a re-plan and backs off `BACKOFF_TICKS` (20, doubling to
  160) before searching again, instead of re-searching every tick.
- Failure timeout: `GotoOpts::give_up_ticks` (default 600 = 30 s): no new closest approach to the goal (classic
  heuristic, by ≥1 block) for that long fails with `NotPossible`. Per-progress, not wall-clock, so long gotos live.
- Done bar test: `tests::segments` drives a 500-block lane through a moving load horizon (`Horizon`, 5 chunks: the
  planner sees chunks near the bot only, as on a server), ≥3 segments, horizontal speed sampled ±10 ticks around
  each join node.
- As built, deviations: (1) the plan-ahead trigger is `min(ticks left, straight line to the end × per-block ticks)`:
  a running shortcut holds the follower's index at its start, so the planned ticks overstated what was left and the
  search started one tick before the join (Optimal slowed to 0.087 b/t there, then diverged). (2) A re-plan while
  moving (> 0.1 b/t) takes the quick inline search's partial path instead of stopping for the full search; plan-ahead
  extends it. (3) Favouring only has slack where edges cost more than the heuristic: with macros on, classic edges
  (3.56 t/b vs 2.80 Optimal / 3.27 Human); with macros off it only breaks ties. (4) `tests::pilot_sim` takes
  `PILOT_TRACE=a..b` for per-tick pilot state.
- Measured (sim, release, `tests::segments`): 500 blocks, lane with two 2-gaps and a raised stretch. Optimal 1443
  ticks, 9 joins, 0 diverged, slowest near a join 0.286 b/t; Human seed 1 1558 ticks, 7 joins, 1 diverged (not at a
  join), slowest 0.307. Late splice (answers 140 ticks late): waits at each join, arrives, 0 re-plans.
- Live (testbox strict BDS, fresh flat world per run, `examples/goto.rs` +500 x, seed 1, saturation): Optimal 1483
  ticks, 9 joins, slowest near a join 0.286 b/t, 0 re-plans, 3 diverged (two in the first segment, one 1 block past
  a join at 0.300 b/t), 1 correction (first segment, mid-air). Human 1577 ticks, 7 joins, slowest 0.309, 0 diverged,
  321 corrections: all on off-axis stretches (z drifting off a block centre), none on the on-axis stretch x 95–351,
  i.e. the known one-ulp off-axis issue (step 10), not the joins.
- Human's off-axis corrections, after BDS's sine table and the correction box fit (DESIGN.md "Off-axis ulp
  corrections"): the same goto drew 10 / 10 / 5 for seeds 1–3. Optimal's goto from a fresh spawn then stopped at
  once (`no closer for 601 ticks`): fixed, see "Core fixes" (starved macro starts).
- Fixed after (see "Core fixes"): Human's turn-rounds inside shortcuts, the plan-ahead holding the rollout cache.

## Core fixes (2026-10-15, `pathfind/core-fixes`, sim only)

- Shared rollout cache: every search of a goto, running ones included, shares one `Arc<RolloutCache>`. Each move
  family's memo is 16 `Mutex` shards (by key hash), locked for a lookup or an insert, never while a result is
  computed; a hit's check reads terrain under the shard lock (only chunk read locks below it). `claim` swaps in a
  fresh store for another player/profile or past 50k results; a search on the old store keeps it. Hit counts are per
  search. Two searches missing one key at once both compute it (both kept). Single-search timings unchanged.
  Tests: `tests::cache_shared` (a re-plan while a plan-ahead is under way hits; 4 concurrent cold searches plan as one).
- Human turn-round, cause: a shortcut's sprint-jump came down ~0.5 block past its target cell; the held heading is
  dropped whenever the yaw is >1° off it (Human jitter 1.5°), so the aim re-pointed at the cell behind and the bot
  circled back into it (5 times on the 500 lane: x 146, 296, 325, 388, 472). Fix: `ShortcutCtl` is done on landing
  at the target's level up to 1 block past its centre, within 0.5 of the line. Human 500 lane 1558 → 1446 ticks;
  segments assert no turning back; `segments::human_runs_on_past_a_shortcut_target` failed at x 146 before.
- Course-leg plan cost: on the course's 3-deep pits most rollouts were leaps into the pit floor that tried every
  takeoff × coast. A jumped leap down 3 counts ≥ 4.25 − 0.8 (the landing tick's drop is not counted) > 3 fallen, so
  it can never land; `solve` now only runs off the edge there (`falls_too_far`; jumped down-2 still lands).
  Test: `leaps_shaped::a_jump_down_three_blocks_always_falls_too_far`. Optimal plan costs unchanged wherever both
  complete. Course plans (release, `tests::course`, testbox load 7–11): Optimal gap3 434 → 52 ms, gap4 691 → 74,
  stones 877 → 64, sprint40 1193 → 667 ms and now within the rollout budget (driven 142 → 130 ticks); Human gap3
  1179 → 185 ms, stones now plans in full (driven 61 → 37–43 ticks), sprint40 driven 124–150 (was 135–157).
- Live macros-on losing to off (gap3, sprint40; step 10) does not reproduce in the sim (`COURSE_MACROS=off`
  `tests::course::parkour_course_legs`): gap3 33 vs 51 ticks both profiles, Optimal sprint40 130 vs 147, Human
  sprint40 mean 134 vs 141 (seed 5 alone loses, 150 vs 135). Live, the plans came from a 2 s search on a loaded
  testbox, so suspect plans cut by the time budget before the fix above.
- Tried, no gain: ending the takeoff loop once every coast overshoots (identical rollout counts; reverted).
- Unchanged (release): `macros_never_slow` table, segments Optimal 1443 ticks, `perf_report` cold (gap course
  Optimal 6.4 ms, Human 39 ms; flat 80 13.5 / 28.8; diagonal 15.8 / 2.6; 5-gap refusal Human 18.8), cost skew
  (classic under-runs 7–8 ticks, macro 5–8).
- Lazy leaps (`frontier.rs`, `reach.rs`): a node's leap targets (`leap_moves::candidates`: the geometric prefilters)
  are queued unsolved at a floor on their cost and solved when popped. Floor = ticks for the feet to reach the target
  cell at the fastest straight run from the node's speed (sprinting, jumping after 0/1/2/3/5/8 ticks or never; simulated
  once per start speed on flat default-friction ground; one tick of slack; `LEAP_VMAX` 0.7 b/t on other floors) +
  `Human`'s jump hesitation + the hazard term. Not the heuristic between the ends: one running leap beats steady hops
  (a 0.9 × heuristic floor planned gap3 at 35.6 instead of 30.1). Debug builds assert each solved leap costs at least
  its floor. "Fell short" pruning is per (node, line), only for leaps as far or farther (a far leap popped first had
  hidden a nearer one). A hop and a leap landing alike tie; whichever is relaxed first stays (tests accept either).
- Macro moves start only on the ground (`follow_macro.rs`): the planner plans them from standing on the node before,
  but an Ascend counts as reached mid-air, so a Run after a step-up started in the air and failed at once. This was
  Human's divergence at x 200 on the 500 lane (the raised step) and an `Optimal` one on the speed test's stairs that
  lazy leaps' better stairs plan exposed. Test: `macro_start::a_run_after_a_step_up_starts_on_landing`.
- Shortcuts: once past the target's centre the held heading is never re-aimed (the eased aim turned round mid-air:
  Human x 472, landing 1.1 past); overshoot up to 2 blocks ends a shortcut only when 3 classic flat nodes follow (past a
  macro's start or a leap's takeoff the follower would come back, or start the macro elsewhere: Optimal diverged in
  the late-splice run, Human got stuck past a down2 leap); a `Human` shortcut onto the goal (`Follower::set_ends_at_goal`,
  not a segment end) brakes and stops on it instead of arriving at a run and circling back (`Optimal` turns on the spot
  and keeps arriving at speed: braking cost it 5 ticks on the diagonal).
- Rollout cache eviction: per key most recently used first; a full key drops the least recently used (it dropped the
  most recently hit one). Evictions are rare: stairs Optimal plan 69 → 49 evicted, 1623 → 1600 misses, 16413 → 16282
  rollouts; stairs Human 609 → 591 misses; course legs within ±3 rollouts.
- Classic cost skew, cause (`cost_skew_report` now prints when the feet first come within the tolerance): classic
  under-ran by ~4.3 ticks of run-up from standing (near the goal after cost + 4.2–4.7, 5–60 blocks) plus ~3 of
  settling at the goal; macro plans simulate their run-up and under-run by ~3 (settling only). Calibrated: classic
  moves out of a standing search start cost `CLASSIC_STARTUP` (4) more, so they compete fairly with macro moves from
  standing; the heuristic is unchanged (costs only rise). Settling is the same for every plan, not costed. Effect:
  Human course legs pick macros from rest: gap4 58 → 49–51 ticks driven, up1 34 → 27–32, down2 32 → 29–31, stones
  45–49 → 36–49, sprint40 125–159 → 125–136; speed test diagonal off 149 → 146 / 149.8 → 147. Plan costs from a
  standing start are 4 higher (same paths).
- Measured after (release, testbox, `tests::course::parkour_course_legs` plans, ms / rollouts, before → after):
  Optimal gap3 74 → 10.5 (439 → 190), gap4 88 → 7.9, up1 22 → 2.2, down2 38 → 7.5, diag 1051 → 59 (11083 → 1197), stones
  111 → 18.6, sprint40 759 → 425 (12146 → 8149); Human gap3 179 → 70, gap4 983 → 529, up1 469 → 42, down2 79 → 66,
  stones 767 → 867 (8170 → 15100: plans in full, cost 51.1), diag 1929 → 1116 and sprint40 1712 → 1257 still at the
  400k budget. Optimal plan costs equal wherever both searches finished (+4 from a standing start, above).
  `perf_report` cold: gap course Human 39 → 31.4 ms (587 rollouts, was 760), Optimal 6.4 → 6.1, others unchanged.
  Segments 500 lane: Optimal 1443 → 1444, Human 1446 → 1439 ticks, 0 diverged (was 1), no turning back; late splice
  839 → 840. Speed test: all rows on ≤ off; stairs Human with shortcuts 143 → 134.
- Then (same day): 85% of Human diag's rollouts were diagonal leaps down 2 from the rim into the pit floor (235
  solves, ~80 rollouts each with the 5-yaw wide retry); their floors left them under the goal's f, so laziness could not
  skip them. Leaps down onto a dry floor are now queued only if the landing has a way on (`exit::Exits`: the classic
  flood of `has_exit`, open at `EXIT_NODES` cells or a goal cell; a closed flood marks its whole basin closed, as every
  cell in it reaches only cells the start reaches; cached per search). Same-level and water landings are not
  checked (the course's stones are islands only leaps reach). Heuristic like `has_exit`: a basin left only by a leap
  wider than the classic gap jump counts as closed. Test: `dead_ends::leap_landings_into_a_closed_pit_are_skipped`.
  Course-leg plans (release, from the same starts): costs equal; Optimal diag 59 → 5.7 ms (1197 → 104 rollouts),
  sprint40 425 → 243; Human diag 1116 ms at the budget → 146 ms (3253 rollouts), sprint40 1257 ms at the budget →
  818 ms within it (15136), stones 867 → 244, gap4 529 → 184. 1 s-budget plans (`course_plans_complete`): every Human
  leg but sprint40 now finishes in the 0.5 s macro share (diag cost 64.3 → 61.7).
- The search start plans its leaps from the bot's real facing (`macros::leap(.., real_yaw)`, part of the cache key);
  other rest nodes are still planned facing their leap (they are reached moving that way). A `Human` start facing
  back or sideways was planned as if facing the leap and re-solved live (or diverged, as on up1 after a settling change
  left the bot facing back); now the plan includes the eased turn and runs as planned (backward start onto a 2-gap +1:
  40 → 38 ticks). `Optimal` snaps its aim, unaffected. Test: `macro_start::a_leap_from_the_start_plans_from_the_real_facing`
  (Optimal and Human facing back and both sides: no live re-solve, no divergence; failed for Human without the fix).
  Human course-leg costs from a start facing away rise accordingly (down2 21 → 24, diag 57.9 → 60.9).
- Unchanged by these two: perf_report (gap course Human 31.7 ms), segments (Optimal 1444, Human 1439 ticks, 0
  diverged), late splice 840, speed table.
- Open: gap-course Human 31.7 ms vs the 30 ms target; Human sprint40 0.8 s.
- Starved macro starts (live, testbox strict BDS): Optimal's 500-block goto from a fresh flat spawn never moved. The
  first node was a planned `Run`; that tick's shortcut search (spawn facing south: every shortcut lost to the
  planned hops) spent 1470 of `ROLLOUT_BUDGET`'s 1500 ticks, so the run could not be predicted and the follower
  reported `Diverged` (spend short, no inline search); the pilot re-planned the same path, 19 times, until the
  failure timeout. With the test pack on, its scene forced a leap start (no shortcut search), which is why the C1
  runs worked and why it looked older than the sine-table fix. A start that fails for want of budget after this
  tick's shortcut search (once per node) now waits a tick (`MacroStep::Wait`, jump released) instead. Test:
  `macro_start::a_macro_starved_by_the_shortcut_search_starts_next_tick` (diverged at tick 0, `Spend` 1470/1500, before).
  Live after (fresh world per run, pack off, docs/testing.md), 500-block goto ticks / corrections / joins / diverged:
  Optimal 1454 / 0 / 8 / 1, Human seeds 1–3 1455 / 10 / 7 / 0, 1446 / 3 / 7 / 0, 1453 / 7 / 7 / 0; slowest near a
  join ≥ 0.281 b/t. Course (all legs ok, 0 diverged): corrections Optimal 3 (stair2, gap, ladder: classic legs),
  Human 2 / 4 / 5; parkour legs Optimal 0, Human 0 / 1 (diag) + 1 (sprint40) / 4 (gap4 1, diag 3).

## M3: boats (2026-10-28, `pathfind/boats`)

Design, decisions and the wave RNG finding: `../research/pathfind-boats-design.md`. Still water only; the done bar
(decision 5') is 0 horizontal corrections (x, z or yaw beyond 1e-5) per route, height-only wave corrections
counted but allowed: BDS draws its big waves from a level-wide, entropy-seeded MT19937 no client can predict
(`docs/research/riding-fishing-elytra.md` "Boat").

- `Bot::boat_to(target, BoatOpts)` (`pathfind/boat/`): boards (`launch.rs`: the hotbar boat placed 2 out, else an
  empty boat in reach), routes over cells a boat clears (`route.rs`, `grid.rs`), steers by rolling out key sets on
  `acacia-physics` `boat_tick` (`pilot.rs`, `solve.rs`, `stop.rs`), dismounts at the far bank, breaks the boat and
  walks to its drop until the boat is back in the inventory (`keep_boat` skips that). Human: placeholder
  key-hold, reaction and look-lead constants in `human.rs`.
- In `goto` (`boat/legs.rs`): with `SearchOpts::boats` and a boat carried or seen, a dry bank cell beside a body of
  floating water gets one `MoveKind::Boat` edge to the body's landing bank (the one nearest the goal); cost
  80/120 ticks (Optimal/Human) + 2.7/3.2 a block of water; legs under 8 blocks are walked or swum. The follower
  stops at the bank and `Step::Boat` runs `boat_to`. The heuristic floor drops to 2.64/1.0824 t/b then (the
  octile distance overstates a line by up to 8.24%), so searches expand more; with boats off nothing changes.
- Tests: `pathfind/boat/tests.rs` (routes round islands; the pilot on rivers and a 2-wide canal, both
  profiles), `pathfind/tests/boat_legs.rs` (search, the
  heuristic under the edge, no boat without one, walked ponds, the follower), `acacia-physics` boat shapes.
- Live (strict testbox BDS `work/bds-boats`, flat, `examples/boat.rs`), boat corrections height / horizontal:
  - 500-block river `boat_to`: Optimal 47, 40, 47, 47 / 0; Human 55, 51, 38 / 0. All collected the boat.
  - `GOTO=1 CHUNK_RADIUS=8` (12 walk, a 100 x 81 lake, 12 walk): Optimal 12, 16 / 0; Human 9, 11 / 0 (one
    earlier Human run had 1 horizontal: the boarding correction, no longer counted); every run took one boat
    leg, collected the boat and ended within 0.18 of the target.
  - After the received-yaw wrap (3bbab13): river Optimal 67, 43 / 0, Human 54, 50 / 0; lake Optimal 14, 13, 16,
    14, 8, 6 / 0, Human 14, 16 / 0; player corrections in the goto 0 except one Human run's 1. One earlier
    Optimal lake run drew 17 walking corrections; six runs since drew none (its log was not kept).
- Limits: legs are planned only over loaded water (the far bank must be in view: the example asks for 8 chunks;
  the default radius of 2 walked round); bodies are cells a boat centres in (3 x 3 floating water), so 2-wide
  canals are walked; flowing water, ice and chest boats' cargo are not handled.

## M4: entity avoidance (2026-10-28, `pathfind/entities`)

Design and the user's decisions: `../research/pathfind-entities-design.md`. Physics findings: acacia-physics README
"Entities".
- Tracking: `Trackers::hazards` (lean: kind, position, motion, box, flags). `hazard::Threats::near` snapshots
  the entities within `FIELD_RADIUS` (32) each tick; `hazard/kinds.rs` classes them (Melee, Neutral, Creeper,
  Ranged, Passive, Player, Solid, Projectile; lit creepers and angry neutrals flagged).
- Field (`hazard/field.rs`): a sphere per threat, cost `w·(1 − d/r)²` at the cell centre (y + 0.9), radius grown by
  speed × 10 ticks; per class and profile in `shape()`. Added to `g` only (`Frontier::relax`, every cell a leap
  crosses); `raw` ticks and the rollout cache stay true. Solid entities (boats) close the cells their box reaches
  into (`Cell::ENTITY | AVOID`, cell id bit 63 so cached rollouts don't match).
- Shortcuts may not pass nearer the threats than the nodes they skip (`shortcut::no_nearer`): without it Optimal cut
  straight through a field of zombies.
- Alarm (`pilot/alarm.rs`): re-plans when the field's score of the 8 nodes ahead of the bot rises 6 over its lowest
  since the path was accepted, at most every 10 ticks. "Ahead of the bot": from the node nearest it, since a running
  shortcut holds the follower's index at its start (Human hopped into a zombie dead ahead otherwise).
- Perception (`hazard/perceive.rs`, decision 4): Optimal knows all at once. Human knows a threat once heard (within
  `EARSHOT` 4, a lit fuse within `FUSE_EARSHOT` 10, or within 3 when hurt; 3–5 ticks later) or seen (55° half
  cone, 48 blocks, clear line; 5–8 ticks later), and keeps it in mind 200 ticks. Constants in `human.rs`.
- Blocked (decision 5): a search that finds the goal unreachable while a solid entity is in the field waits the
  back-off (20, 40, 80 ticks; Human up to 15 more) up to 3 times, then fails. Mobs never block (they don't collide).
- `Evade` (`process/evade.rs`, priority above the goto): while a lit creeper is within 8, drives `Goal::Away` from
  where it was (re-targets when it moves 2). The field is perceived every tick before `Arbiter::decide`
  (`goto::sense`), not only while driving: a held route never refreshed it, and Evade held control forever live.
- Entity data freshness (BDS, from `sapling-end-dragon/research/bds-actor-move-sync.md`, read with bdsre
  2026-10-26): a viewer at distance d gets a mob's MoveActorDelta at most every `trunc(10·min(d/80, 1))` ticks
  (arrows 7, boats and minecarts 20 at rest and every tick moving); SetActorMotion only when the motion changed by
  more than 0.2 b/t or stopped. Within the 32-block field a mob's position is at most 4 ticks old (1 within 8).
- Not done: projectile sidestep in Evade (arrows only cost through the skeleton's sphere), the ranged line-of-sight
  penalty (step 8: the skeleton bar passed without it).
- Sim (`tests/entity_sim.rs` crowd model, `entity_scenarios.rs`, `evade_sim.rs`, `hazards.rs`), both profiles: 20
  chasing-zombie runs 0 hits (`zombie_runs_20`, release, ignored); lit creeper behind, beside, where it stands: 0
  blast; skeleton ≤ 2 arrows; cow corridor arrives; boat corridor waits then goes on, or fails after the waits. Guards:
  a crowd off the way visits the bare walk's nodes; 50 mobs across the way 501 nodes (bare 34); 8 bots among 50
  mobs 0 hits at 0.36 ms a tick (release, `eight_bots_among_fifty_mobs`).
- Live (testbox strict BDS :19188, `examples/mobs.rs`, normal difficulty, midnight, 20 runs a scenario and profile):
  creeper (lit by `start_exploding_forced` 6–9 blocks beside the way) 20/20 both, 0 blast; skeleton 20/20 both
  (at most 2 arrows); cow corridor 20/20 both. Zombie: Optimal 17/20 then 20/20, Human 17/20 then 19/20 (two
  batches). Every hit came from a zombie that targeted the bot: it closes at 0.264 b/t (box 1.96 tall), against
  0.114 for the strolling ones that never do, and hits from 2.3–2.5 blocks as tracked. Each such run took one hit.
- Striking back (decision 5', `hazard/strike.rs`, `Pilot::strike` after each steering tick): a perceived melee
  hostile that targets the bot (`TargetEid` = the bot's unique id; BDS sends it while a mob chases) or approaches
  at over 0.18 b/t, and closes in or keeps pace within 3 blocks, is aimed at from 5 blocks and clicked
  (`Bot::click_entity_aimed`) a tick after the crosshair is on its box, within `ENTITY_REACH` (3, eye to box) and a
  clear line. On the course only the pitch turns; otherwise the keys keep the course (`keep_course`). Optimal clicks
  once per 10-tick invulnerability; Human eases its aim, reacts 2–5 ticks, clicks every 4–7 (`human.rs`
  placeholders). Strollers are never struck. BDS's `ItemUseOnActorInventoryTransaction::handle` checks the sent
  player position within 6 of the server's (bdsre); reach and line of sight are ours to keep.
- Sim zombie calibrated to live: chases at 0.264 b/t, hits from 2.3 (centre), cooldown 20, Java knockback from
  the bot's strikes (0.4 push and lift, 10-tick invulnerability). Before striking, it hit in 3–4 of most runs.
- Live zombie (made to chase by `/damage ... entity @s`, so every run's zombie targets the bot; hits counted until
  arrival, the zombie killed before the late-hit wait), 7385dfd, 20 runs each: Human 20/20 (0 hits, 0–8 strikes a
  run), Optimal 19/20 (seed 14: one hit with 2 strikes sent, cause not traced). The batch before the on-course
  pitch-only aim (01ea98e): Optimal 0 hits in 18 runs seen. Sim (calibrated): 20/20 both profiles.
- Corrections in zombie and creeper runs are the open off-axis ulp drift (DESIGN.md "Off-axis ulp corrections"),
  not entities: every one logged is 1–2 ulps on an air or ground tick of an off-axis walk or hop, none follows a
  knockback (hits taken drew 0), creeper runs drew no explosion knockback at all. Yaw changes mid-walk (Human
  easing, strike aims, Evade's off-axis hops) raise them. `prev + wrap(sent − prev)` as the walk yaw was tried on
  `course-human-linux` at tolerance 0: 6 → 28 mismatches, rejected.

## Running
- Tests: `REMOTE_DIR=work/bc-pathfind tools/remote-test.sh -p acacia-bot` (testbox; never workspace-wide locally).
- Risks: synth fidelity (fallback: store full `PlayerState` per macro node); in-air yaw swings on strict BDS; node/rollout blowup on open maps.
