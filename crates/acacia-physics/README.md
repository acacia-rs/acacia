# acacia-physics

Bedrock player movement simulation: a network-free, dependency-free Rust port of
[oomph-ac/bedsim](https://github.com/oomph-ac/bedsim) (MIT), commit
`34d11dc576aeb56a7f7f0541dbb5a600cc443488` (2026-09-16). The f32 operation order of bedsim is
kept, down to Go's `math.Sin` sine table and chewxy/math32 `Sin`/`Cos`. Module names follow the
bedsim files (`movement.rs` = `simulateMovement`, `collide.rs` = `tryCollisions`, `liquid*.rs` = `liquid.go`).

## API
- `tick(&mut PlayerState, &Input, &impl WorldView) -> TickOutput`: one 50 ms client tick. The
  output holds the feet/eye position, `delta` (velocity at the end of the tick, i.e. `PlayerAuthInput.delta`),
  `on_ground`, `vertical_collision`, `horizontal_collision`, `jumped`, `teleported`, and the processed `move_vector`.
- `apply_current(&mut PlayerState, &impl WorldView)`: one tick of liquid current added to the velocity
  without moving (BDS pushes a player it holds after a teleport).
- `Input`: held controls (raw move vector, yaw/pitch, jump, sneak, want_down, sprint, swim, glide, using_item).
  Start and stop edges are derived from the state, the same way as `toInputState` in the harness.
- `PlayerState`: `queue_teleport(feet)` (applied on the next tick, velocity reset to zero),
  `queue_knockback(vel)`, `apply_correction(feet, delta, on_ground)`, `set_movement_attribute(v)`,
  `eye_position()`. Effects and equipment are set on `state.effects` and `state.equipment`.
- `WorldView`: `block_collisions(pos)` (block-local boxes), `block(pos) -> BlockPhysics`, plus the defaulted
  `collisions(area)` and `is_area_loaded(area)`. `test_world::TestWorld` is a synthetic grid.

## Ported
Gravity and drag, per-block friction (ice, blue ice, slime, honey) and soul sand acceleration, walk, sprint and
sneak speeds with the swift-sneak and item-use slowdowns, and sneak edge avoidance. Jumping includes the sprint
boost, the 10-tick jump delay and honey's reduced jump. Collisions sweep Y, then X, then Z, with a 0.5625 auto-step,
depenetration and one-way stuck handling. Also ported: supporting-block lookup; ladders and vines; cobweb and
Weaving; powder snow and berry-bush stuck multipliers; scaffolding and powder-snow traversal; slime and bed bounce;
honey wall slide; and bubble columns. Liquids cover water and lava travel, swimming (with the swim hitbox and
water grace), currents including falling water, Depth Strider and the dolphin multiplier, and the ledge-exit
boost. Effects: jump boost, levitation, slow falling. Also ported: elytra gliding (including the firework boost
ticks), sneak, crawl and swim pose fitting under ceilings, teleports, knockback, and loaded-area freezing.

## Not ported
Riptide, vehicles, creative flight and no-clip (bedsim does not simulate these either; it resets to the client),
bedsim's client-drift correction and reconciliation, the step tie-breaker (always accepted, as with
`IgnoreClientStepTiebreaker`), slide offset, legacy sprint timing, server-forced sprint, crawl input flags,
the `AutoJumpingInWater` input flag, and powder snow's player-dependent collision shape (leather boots, a
long fall) — the world adapter must resolve that itself. Speed and Slowness come in
through `set_movement_attribute`, which takes the server's `minecraft:movement` without its sprint and freeze
modifiers (the simulation adds both itself).

## Deliberate deviations from bedsim
Each was found by fuzzing against strict BDS (`acacia-bot` examples `fuzz` and `replay`) and fixes
mismatches there; `tests/bedsim_diff.rs` lists the bedsim scenarios that diverge because of them.
- The sneak slowdown lasts through the tick sneaking stops, and a sprint cannot start on that tick.
- No sneak slowdown in water (contact this tick), where sneak means sink. Sinking needs `want_down`
  (the vanilla client's `WantDown`, sent with every sneak) or else the top of a standing head (feet +
  1.8, whatever the pose) under water (a still source's surface is 8/9 high) and not swimming. Without
  `want_down` it loses to a held jump; with it, both apply and cancel out.
- In water (not swimming), a jump held on the previous tick ends a sprint and blocks one (the vanilla
  client's rule, seen in captures; it is what makes BDS drag sprinters heavily after a jump);
  sprinting with the breathing point under water swims (see below), and movement input with the sprint input
  held, or a ceiling too low to stand under, keeps the swim going; sneaking neither blocks a sprint (or sprint swim) start nor ends a sprint; a swimming jump lifts only with
  feet + 0.3 under the surface (else vertical speed is zeroed); and cobwebs slow travel as on land.
- Sprint swim start (from BDS 1.26.52's `SwimTriggerSystem` and `UnderWaterSensingSystem`): the breathing
  point (the eyes, feet + 1.62 standing) must be under its water cell's surface, cell + 1 - d/9 for liquid
  depth d (sources and falling water are full). Looking up, look y >= 0.15 from the pitch two inputs back (BDS reads
  the previous rotation before applying this input's),
  the breathing point's block and the block above the box centre's must also be non-air. A swim ends when its
  breathing point (feet + 0.4) is in an air block while looking up past acos(cos²(pitch)) > 45° (`SwimTriggerSystem`).
- Swimming up, the steering stops (vertical speed zeroed) only without liquid at the swim eye, feet + 0.4
  (bedsim: feet + 0.42).
- The supporting block is the colliding block whose centre is closest to the feet (bedsim: to the centre of
  the feet's cell).
- A cobweb slows only a box that overlaps it by more than 0.001.
- Sneak edge avoidance applies on the ground in liquid too.
- A crawl ends on any tick standing (else sneaking) fits, not only when a swim or glide ends, and its
  slowdown starts the tick after it begins.
- Ending a swim blocks jumping for 10 ticks, whether or not jump is held meanwhile.
- Liquid contact during a tick uses the box from before that tick's pose change (a swim ending at a
  ledge still gets water physics on that tick).
- While swimming, the sprint ends with the swim (on the same tick, with StopSwimming): when the player leaves
  the water or releases the sprint input, unless a ceiling is too low to stand. A swim start sees the sprint before the jump-in-water rule
  cancels it (the vanilla client then sends StartSprinting, StopSprinting and StartSwimming together).
- The slime and honey slowdown applies in water too and on landing ticks, keyed on the vertical speed at the
  end of the tick (any vy < 0.1, so a fast fall too), and only while the box is over the supporting block's collision
  shape (not one just walked off, nor honey's 1/16 inset rim). The block that slows is BDS
  `CurrentlyStandingOnBlockSystem`'s, not the supporting one: of the collision boxes crossing the plane 0.2 below
  the feet, the highest-topped, ties going to the centre closest to the feet; so slime under a carpet slows, but
  only while the player's centre is over it.
- A current takes no flow towards a dry neighbour that blocks motion (a fence or wall too, whatever its
  solid faces, but not a carpet); bedsim only stops it at a closed face.
- Climbing against a wall sets the vertical speed to 0.2 after the move and gravity, from that tick's
  collision; a held jump still sets it before the move, as in bedsim.
- Slime bounces with the speed at impact, `sqrt(v² + 2·g·fallen)` over the distance fallen that tick, also
  under liquid (where g is the liquid's gravity, 0.005 in water), but only from a downward speed of 0.08000012
  (BDS `ComputeBlockRestitutionSystem`): not a step down at the standing 0.0784, nor a slow sink in water.
- Water contact counts any liquid cell the shrunk box overlaps, however shallow, and so does the
  ledge-exit probe (but only by more than 1e-5: a box flush against the cell does not count).
- `apply_current` adds liquid currents while BDS holds a teleported player.
- Inside-block effects (powder snow, berry bushes, bubble columns) visit BDS's entity-inside cells,
  floor(min + 0.001)..=floor(max - 0.001), so a cell the box only grazes does nothing.
- A box overlapping a collider is never moved out of it (bedsim depenetrates until stuck for two ticks): along
  the shallowest axis it may move out but not further in, and BDS `MoveTowardsClosestSpaceSystem` pins the x/z
  velocity at 0.1 away from the mean centre of the overlapped boxes (+x+z when centred; an axis whose next block
  is taken turns round, or drops out if both sides are). Seen after a teleport into snow layers or a block
  placed on the player.
- Scaffolding collides only from above its block and not while descending (bedsim leaves this to the world).
- The honey wall slowdown (x/z × 0.4 per touched honey side) applies on the ground and rising too, and in lava,
  but not in water.
- The standing eye offset is 1.62001 (BDS; bedsim: 1.62), so `PlayerAuthInput.position` matches the server's
  to the bit; with 1.62 it is an f32 ulp low at most heights and strict BDS corrects nearly every tick.
- Powder snow freezes (BDS `FreezingComponent`): +1/140 a tick inside, -1/70 outside, and the movement speed
  takes freeze × -0.05; bedsim has no freezing. BDS steps it per world tick, not per input, so a client whose
  inputs fall behind the server's clock sees the server's freeze run ahead (a ~0.001 drift per tick in the snow).

## WorldView contract
- `block_collisions`: block-local boxes (0..1, taller for fences and walls) of layer 0. They are used for collisions,
  support lookup and falling-liquid barriers. Liquids have none.
- `collisions(area)`: every non-empty world-space box with `Aabb::intersects(area)` (strict overlap greater than 1e-5
  on each axis). The default scans cells `floor(min)..=floor(max)`, starting y one cell lower.
- `block`: `BlockPhysics` holds air, friction, the acceleration multiplier, the climbable, cobweb, honey,
  bounce, inside and traversal flags, fence_or_wall, `liquid` from either layer (Bedrock `liquid_depth` d:
  `depth = 8 - (d & 7)`, `falling = d >= 8`), a solid-faces bitmask for liquid flow, and `bubble_column`.

## Differential tests
`tests/bedsim_diff.rs` replays `tests/fixtures/bedsim.json`: 40 scenarios and 2231 ticks, driven through the Go
original. Every tick must match position, velocity and fall distance within 1e-5, and all flags must match exactly.
The current maximum deviation is 0 (bit-exact). To regenerate after changing
`tools/diffharness/scenarios.go` or bumping bedsim in `tools/diffharness/go.mod`:

    go -C crates/acacia-physics/tools/diffharness run . ../../tests/fixtures/bedsim.json

`tools/diffharness/world.go` must stay in sync with `src/test_world.rs`.
