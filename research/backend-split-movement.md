# Backend split: movement, pathfinding and physics

Recon of `crates/acacia-bot` (branch java/edition, 2026-11-10) for build-order step 4 of docs/java-edition.md.
Paths are relative to `crates/acacia-bot/src` unless they name a crate.

## 1. The Bedrock movement loop

**Who runs physics.** `Bot::physics_tick` (bot/tick.rs:113-149) runs once per client tick (timing from
`cadence.rs`): it sets elytra, worn armour (`movement/equipment.rs:15`), abilities (`movement/flight.rs:31`),
solid entity boxes (`Movement::set_solids`, movement/mod.rs:171), then calls `Movement::tick(&PhysicsWorld{view,
registry})` (bot/tick.rs:136) and sends the returned `PlayerAuthInput` with block actions attached
(`attach_actions`, controls.rs:67) plus a `PlayerAction(StartFlying)` when needed (bot/tick.rs:143-147).

**`Movement::tick`** (movement/mod.rs:176-242), in order:
1. `controls` (yaw/pitch) get rotation noise (`RotationNoise::apply`, rotation_noise.rs:26; only live bots,
   `rotation_noise: Option`, mod.rs:78) and the glide look guard (`physics::steady_glide_look`, mod.rs:184).
2. `still_ticks` zeroes movement keys (Oomph rule around ItemStackRequest, mod.rs:186-189).
3. `History::apply` rewinds and replays (rewind.rs:150-209): a pending correction/teleport/knockback stamped
   with an already-simulated input tick restores the saved `PlayerState` of that tick, applies
   `apply_correction`/`apply_small_correction`, then re-runs `physics::tick` on every later recorded `Input`
   with that tick's solid boxes, movement attribute, effects and freeze timelines.
4. `KeyLatch::advance` (keys.rs:27-67) turns `Controls` into `acacia_physics::Input`: wire-rounded rotation,
   double-tap sprint, mouse-vs-touch sprint latch, flight double-tap jump, diagonal normalisation.
5. `physics::tick(st, &input, &WithEntities{world, boxes})` (mod.rs:204); BDS teleport landing applies
   liquid current (mod.rs:211-214).
6. Edge flags (sprint/sneak/jump/swim/glide/fly start/stop) from state transitions (mod.rs:216-229),
   `history.record` (mod.rs:232), `auth_input::build` (auth_input.rs) into `PlayerAuthInput`.

**Server packets** (`Movement::apply`, packets.rs:54ff): `MovePlayer` → `queue_teleport` or a scheduled
rewind correction (packets.rs:73-93); `CorrectPlayerMovePrediction` → player correction via History, or
vehicle correction for boat/horse sims (packets.rs:96-116); also MobEffect, attributes, knockback, freeze,
ChangeDimension. `Movement::fork` (fork.rs:31) clones `PlayerState` + `KeyLatch` into a `Rollout`
(no history, no noise) for planning.

**Split.**

| Generic (keys → physics → position) | Bedrock protocol / BDS-specific |
|---|---|
| `Controls` (controls.rs:20-64), `Rollout` (fork.rs:10-27) | `auth_input.rs`, `packets.rs`, `rewind.rs` (tick-stamped server-authoritative rewind) |
| Rotation noise idea (rotation_noise.rs) — anticheat-motivated, applies to Java too | `KeyLatch` rules are BDS's (Java's equivalents live inside `acacia-java-physics` `Player::tick`: `sprint_trigger`, flight toggle, glide start) |
| `cadence.rs`, `human.rs` (timing/delays; capture-fitted on Bedrock but edition-free) | `idle.rs` (standing `PlayerAuthInput`), `prediction_sync.rs` (`ClientMovementPredictionSync`) |
| `glide.rs`/`flight.rs` state readers | `still_ticks` (Oomph), `current_on_landing`, BDS teleport hold, `steady_glide_look` |
| | `reflex.rs` (Animate/InventoryContent replies), `riding/**` (seat `PlayerAuthInput`, BDS boat/horse sims) |

Java has no tick-stamped corrections: the server teleports (`PlayerPosition` + accept id) and the client
snaps; `acacia-java-client/src/movement.rs:68` already handles that. So `History`/rewind has no Java twin.

## 2. How pathfinding uses physics

**Both**: rollouts with the real simulation, plus Baritone-style constant tables.

**Rollouts (simulation).** Everything goes through `movement::Rollout` (`st: PlayerState` + `KeyLatch`) and
`Rollout::step(&Controls, &W: WorldView)`:
- `pathfind/rollout.rs:19` `run` (controller loop), `:60` `peek` (clone + step), `:82` `Body::of(&PlayerState)`
  — the controller-facing view (`pos, vel, yaw, on_ground, h_collision, bonk, fall`).
- `control.rs:22-25` `Controller::step(&Body, peek: &mut dyn FnMut(&Controls)->Body, &mut Controls)` — the
  controllers (leaps, hops, macros, shortcuts) already see only `Body` + `Controls`: edition-neutral.
- `solve.rs:123-131` `Solver::run`, `:161` `solve`, `:211-218` hop, `:281` `falls_too_far` (reads
  `st.effects`, `st.pos`); `judge.rs:34-45` perturbs `st.pos`; `retry.rs:21`.
- `node.rs:100-115` `Momentum::snap(&PlayerState)` (pos, vel, yaw, on_ground, sprinting, sneaking,
  pressing_jump, jump_delay) and `node.rs:133-153` `Momentum::synth` builds a fresh `PlayerState::new` and
  copies effects/equipment/flight/jump_strength/freeze/movement_attribute from the live state.
- `macros.rs:68-117` simulate from `Rollout::new(mom.synth(..))`; `hop.rs:39,74`; `leap_moves.rs:116-185`
  (reads `effects.jump_boost`, `movement_attribute`, `vel`; sets `yaw`).
- `reach.rs:24-69` simulates sprint/jump on a private `Flat` `WorldView` to bound leap costs.
- `follow_macro.rs:79-187`, `follow/steer.rs:73-84` (coast end), `follow/shortcuts.rs:32`,
  `shortcut.rs:115-180` — live follower rollouts from `movement.fork()` (`pilot/mod.rs:180`).
- `flight/solve.rs:111-196`, `flight/pilot.rs:96-184` — elytra rollouts; read `gliding`,
  `glide_boost_ticks`, `collide_x/z`, set `glide_boost_ticks`.
- Boats: `pathfind/boat/solve.rs:201` `boat_tick`, `boat/stop.rs`, `boat/pilot.rs`, `boat/grid.rs`
  (`BoatState`, `LiquidKind`) — Bedrock-physics-only. Riding: `riding/boat.rs:58,69,75,124`
  (`boat_tick`, `wave_phases`, `wave_step`), `riding/horse.rs:41,52` (`horse`, `horse_tick`).
- `pilot/mod.rs:137,224` `drive(&Terrain, &mut Movement, ..)` / `plan(.., live: PlayerState, ..)`;
  `job.rs:5`, `cache.rs:12`, `shift.rs:4`, `pilot/ahead.rs:3`, `astar.rs:6` carry `PlayerState` as the
  search's base state.

**Constants (hard-coded envelope).**
- `costs.rs:8-82`: WALK 20/4.317, SPRINT 20/5.612, SWIM 20/1.96, SNEAK, `CLIMB = 1/CLIMB_SPEED`,
  `fall_ticks` from `NORMAL_GRAVITY`/`NORMAL_GRAVITY_MULTIPLIER`, `jump_one_block`, HOP_SPEED 0.3563,
  HOP_TICKS 12, COST_HEURISTIC 3.563 (header says Bedrock shares Java's — true for walk/sprint/gravity).
- `moves.rs:85-87,166` MAX_JUMP_RISE 1.2, MAX_SAFE_FALL 3, `STEP_HEIGHT`; `solve.rs:20-24` JUMP_RISE 1.25,
  LAST_TICK_FALL 0.8; `reach.rs:18-21` friction 0.6, LEAP_VMAX 0.7, `:90` `DEFAULT_MOVEMENT_SPEED`;
  `leap_moves.rs:29` RUNNING 0.12; `node.rs:7` `JUMP_DELAY_TICKS`, `SNEAKING_PLAYER_HEIGHT`;
  `terrain.rs:14,251` `DEFAULT_PLAYER_HEIGHT`; `goto.rs:142` `DEFAULT_PLAYER_HEIGHT_OFFSET` (eye);
  `follow/steer.rs:3,43` and `shortcut.rs:5,33` `DEFAULT_GROUND_DRAG` (coast distance).

**Plain type use** (no behaviour): `BlockPos = [i32;3]` and `Vec3 = [f32;3]` from acacia-physics in ~47
non-test pathfind files; `Aabb` in `hazard/mod.rs:121`, `shortcut.rs`, `reach.rs`.

**Physics API pathfinding actually needs:**
1. Clone a state; step one tick from it with held `Controls` (key latches included).
2. Read `Body` (pos, vel, yaw, on_ground, horizontal/vertical collision, fall distance).
3. Snap/synth a state at a node (pos, vel, yaw, on_ground, sprinting, sneaking/pose, jump held, jump delay),
   carrying attributes/effects/equipment from the live state.
4. Overwrite pos/vel/yaw (reach, judge perturbation, leap facing).
5. Read a few traits: jump boost / slow falling / levitation, speed attribute ratio, gliding, glide boost.
6. Envelope constants: step height, player height, eye height, sneak height, jump rise, gravity/drag
   (fall table), ground drag, climb speed, walk/sprint/swim speeds, jump delay.
7. A world adapter: the planner's `Terrain` must be a world the engine can step in (`physview.rs:10-32`
   implements `acacia_physics::WorldView` over memoized cells, using `world::block_physics`).

No direct collision queries or jump-height formulas are called; collision only runs inside `tick`.

## 3. Shared trait

**acacia-java-physics surface** (`lib.rs:17-20`, `player.rs:36-80`, `tick.rs:97`):
`Player { pos: [f64;3], delta: [f64;3], yaw, pitch, on_ground, horizontal_collision, vertical_collision,
sprinting, keys, flying, may_fly, fall_flying, effects, using_item, speed_add, solid_entities, in_water,
swimming, pub(crate) pose/fall_distance/no_jump_delay/crouching/sprint_trigger.. }`,
`Player::at(pos)`, `Player::tick(&mut self, world: &impl Blocks, keys: Keys)` (no output struct),
`Keys { forward, backward, left, right, jump, shift, sprint }` (bools), `Blocks::state(x,y,z) -> Option<u32>`
(Java state id = neutral `acacia_registry::BlockState` id, registry state.rs:8-9) + `min_y`.

**Mismatches**
- Precision: Java `f64` pos/delta vs Bedrock `f32`. Planner positions are `f32` everywhere; convert at
  `Body`/`place` (f32 is ample for planning; exactness lives inside the engine and live sending).
- Input: Bedrock `Controls` → `KeyLatch` → `Input{move_vector, yaw, pitch, jump, sneak, want_down, sprint,
  glide, using_item}`; Java takes bool `Keys` and yaw/pitch as `Player` fields. `Controls` maps cleanly
  (forward/strafe are already ±1; strafe +1 = left = `keys.left`). Java's double-tap sprint, flight toggle and
  glide start are inside `Player::tick`; `Controls::glide`/`fly` must become jump presses for Java.
- Rollout key memory: Bedrock carries `KeyLatch` beside the state; Java keeps latches in `Player` (`keys`,
  `sprint_trigger`, `jump_trigger`) — cloning the `Player` is the whole rollout.
- State fields: Java lacks `jump_delay` public (`no_jump_delay` pub(crate)), `fall_distance` and `pose`
  pub(crate), no `pressing_jump` (use `keys.jump`), no `movement_attribute` (use `speed_add`), `Effects`
  types differ (u8 amps vs Bedrock's). `acacia-java-physics` needs a few pub accessors/setters.
- World: Bedrock physics wants `BlockPhysics` + BDS boxes per cell (`WorldView`); Java wants neutral state
  ids and computes shapes itself (`acacia_registry` `java_collision`). `Player::tick` takes `&impl Blocks`
  (Sized) — a `&dyn` planner world needs a small Sized wrapper.
- Bedrock-only: `WithEntities` one-tick-late boxes, `want_down`, freeze/server_freeze timelines,
  `apply_current`, `steady_glide_look`, `glide_boost_ticks` (firework boost — Java fireworks are entities),
  boats/horses (`boat_tick`, `horse_tick`), dolphin/flight details.
- No `TickOutput` on Java; the planner only uses `Body`, so that is fine.

**Recommended trait** (in acacia-bot, e.g. `movement/sim.rs`; one impl per edition):

```rust
/// One edition's player simulation, as planning and following step it (see docs/java-edition.md).
pub trait Sim: Clone + Send + Sync + 'static {
    /// One tick with `c` held, key latches included, exactly as the live bot would.
    fn step(&mut self, c: &Controls, w: &dyn SimWorld);
    fn body(&self) -> Body;                       // f32 view: pos, vel, yaw, on_ground, h_collision, bonk, fall
    fn momentum(&self, at: BlockPos) -> Momentum; // node.rs snap
    /// A rest-or-moving state at `at`; attributes, effects, equipment, flight from `self`.
    fn synth(&self, m: &Momentum, at: BlockPos) -> Self;
    fn set_motion(&mut self, pos: Vec3, vel: Vec3, yaw: f32);
    fn traits(&self) -> SimTraits;                // jump_boost, slow_fall|levitation, speed_ratio, gliding, sneaking
    fn envelope(&self) -> &'static Envelope;      // step/height/eye/sneak height, jump rise, gravity+drag,
                                                  // ground drag, climb, walk/sprint/swim/hop speeds, jump delay
}

/// The planner's memoized cells as each engine reads them; `Terrain` implements it.
pub trait SimWorld {
    fn cell(&self, p: BlockPos) -> Cell;          // native ids, edition boxes, flags; UNLOADED when absent
    fn neutral(&self, p: BlockPos) -> Option<acacia_registry::BlockState>; // Java steps on this directly
    fn bedrock(&self) -> Option<&dyn acacia_physics::WorldView> { None }  // Bedrock's block_physics path
    fn solids(&self) -> &[Aabb] { &[] }
}
```

`Rollout` becomes `impl Sim for BedrockSim { st: PlayerState, keys: KeyLatch, using_item }` (fork.rs moved
as is); `JavaSim(acacia_java_physics::Player)` implements `step` as `controls→Keys`, set yaw/pitch,
`player.tick(&JavaCells(w), keys)`. `costs.rs` constants become `Envelope` fields (Java values from the
jar: same walk/sprint/gravity; step 0.6; jump 0.42; climb 0.15 vs Bedrock's). Boats, horses and elytra
flight stay Bedrock-only behind `Option` (flight's boost model differs). Generic `S: Sim` through
`Solver`, `Macros`, `Reach`, follower and pilot is the mechanical part; `dyn` would cost the ~5 µs/tick
rollout budget (follow/mod.rs:21) little but generics match the existing `<B: Blocks>` style.

## 4. How pathfinding reads the world

- `terrain.rs:26-29` `trait Blocks { fn ids(&self, pos) -> Option<(u32, u32)> }` — **Bedrock-shaped**: layer-0
  and layer-1 (liquid layer) runtime ids. Impl for `ChunkView` (terrain.rs:31) and `snapshot::WorldBlocks`
  (off-thread jobs, job.rs:36-40 take `Arc<acacia_world::World>` + `Arc<BlockRegistry>`).
- `Terrain::classify` (terrain.rs ~146-156) maps ids through `acacia_world::BlockRegistry` (the Bedrock
  registry) to `Cell { boxes: state.boxes (BDS shapes), flags, block, liquid }`.
- `cell.rs:43-76` `state_flags` is already neutral: `BlockState` predicates + `neutral::Block`/`BlockTag`
  matches (Fire, Cactus, MagmaBlock, Doors...). `terrain.rs:78-83` toggles doors via `neutral.with(prop::Open)`
  then `registry.runtime_id_of` (Bedrock round-trip).
- `physview.rs:18-24` gives rollouts `world::block_physics(registry, ..)` — Bedrock `BlockPhysics`.
- String names: none in the planner core. Remaining Bedrock names: `riding/horse.rs:13,37,39`,
  `riding/input.rs:13-20`, `riding/boat.rs:18` (entity ids/attributes), `goto.rs:73` (`s.name` for dug-block
  bookkeeping), tests (`tests/neutral_kinds.rs`).
- For Java: `Blocks::ids` → one native id (Java has waterlogged props, not a liquid layer — return
  `(id, AIR)` or change the trait to a native id + neutral view), and `Terrain` needs an edition shape source
  (Bedrock `BlockRegistry` vs `acacia_registry::java_collision` + neutral flags). `Cell.block` as a native
  id still works as a memo key.

## 5. Size estimate (Java bot that walks/jumps/sprints/swims on the shared planner)

| Change | Files | Lines |
|---|---|---|
| `Sim` + `SimWorld` traits, `Envelope`; move `Rollout` to `BedrockSim` | movement/fork.rs, new movement/sim.rs | +150 |
| `JavaSim` impl (controls→Keys, f64↔f32, synth/snap, envelope) | new file (acacia-bot, java feature) | +200 |
| acacia-java-physics pub accessors (fall_distance, no_jump_delay, pose/crouch setter) | player.rs | +30 |
| Generic `S: Sim` through rollout/solve/judge/retry/macros/hop/leap_moves/reach/node/follow*/shortcut/pilot/job/cache/astar/shift | ~22 pathfind files | ~300 changed, mostly signatures; `PlayerState` → `S` |
| costs.rs + constant sites (moves, solve, reach, steer, shortcut, terrain, goto, node) read `Envelope` | ~10 | ~120 |
| `Blocks`/`Terrain` edition shape source; Java `Blocks` impl over the Java world; `physview` behind Bedrock | terrain.rs, cell.rs, physview.rs, snapshot.rs, job.rs, goto.rs | ~200 |
| Live movement backend: `Movement` behind an edition enum or trait (`fork`, `controls`, `position`, `on_ground`, `state`); Java tick wired to `acacia-java-client` movement | movement/mod.rs, bot/tick.rs, pilot/mod.rs, goto.rs | ~250 |
| Tests: a Java grid world + a few walk/jump/sprint/swim sims; existing 46 test files mostly untouched (12 use `Movement::fork`) | pathfind/tests | +300 |

Total roughly **1.5k lines touched, ~35 files**, of which ~900 are mechanical generics/constant plumbing.
Out of scope for that milestone: dig/place (execute.rs, `pending_actions` are Bedrock block actions),
boats/horses, elytra flight (glide boost model), rewind (no Java equivalent needed).

Risks: rollout cost per tick for Java (f64, block-state lookups through `SimWorld`) against the follower's
budget; `Momentum` quantisation was tuned on Bedrock (tests/rollout.rs SYNTH_TOLERANCE 1/512) and must be
re-checked for Java; anticheat rotation noise must be carried by the Java sender too (memory:
rotation noise breaks predictions).
