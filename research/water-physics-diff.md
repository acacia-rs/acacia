# Water physics: Boar / bedsim vs acacia-physics

Sources, read 2026-09-30:
- Boar `oryxel1/Boar` @ `949debaab17d57c997a5c6fd7f087edf40335e6e`. Paths below are relative to `common/src/main/java/ac/boar/anticheat/`.
- bedsim `oomph-ac/bedsim` @ `34d11dc576aeb56a7f7f0541dbb5a600cc443488`.
- Ours: `crates/acacia-physics/src` @ `f52cc49`.

Our liquid code is a faithful port of bedsim. The two differ from Boar in one main place: **Boar's water-contact test ignores the liquid surface height.** Any water cell overlapped by the shrunk box counts.

---

## 1. Water contact box and flow push (answers mismatch 1)

### Boar: `prediction/ticker/base/EntityTicker.java:62-114`
```java
player.touchingWater = this.updateFluidHeightAndDoFluidPushing(0.014F, Fluid.WATER);   // :59
...
Box box = player.boundingBox.expand(0, -0.4F, 0).contract(0.001F);                      // :67
int k = GenericMath.floor(box.minY);
int l = GenericMath.floor(box.maxY + 1.0D);
...
if (fluidState.fluid() == tag) {
    float d0 = l1 + 1 - fluidState.height();
    maxFluidHeight = Math.max(maxFluidHeight, d0);
    if (l >= d0) {                     // l = floor(maxY+1) (int), l1 < l
        found = true;
        vec3 = vec3.add(fluidState.getFlow(player, Vector3i.from(k1, l1, i2)));
    }
}
...
if (vec3.lengthSquared() > 0.0D) {
    vec3 = vec3.normalize().multiply(speed);
    player.velocity = player.velocity.add(vec3);
}
```
`Box.expand(x,y,z)` subtracts from min and adds to max (`util/math/Box.java:224-232`), and `contract` negates it. The box is therefore `[minX+0.001, minY+0.401, minZ+0.001] .. [maxX-0.001, maxY-0.401, maxZ-0.001]`, the same size as ours and bedsim's.

**The height test is effectively a no-op.** For any `height` in [0,1], `d0 = l1+1-height <= l1+1 <= l`, so `l >= d0` always holds. Contact is simply "a water cell lies in `floor(minY)..floor(maxY)` of the shrunk box" (x/z likewise). The flow is summed over those same cells.
- Exception (unverified in practice): Boar's height is `(8-level)/9` for `level` 1..7 and `8/9` for 0 or 8 (`data/block/AbstractBoarBlockState.java:239-244`). For Java falling levels 9..15 the height goes negative. That makes `d0 > l1+1`, so such a cell is **excluded only when it sits in the topmost scanned row** (`l1 == l-1`).
- The flow push happens in `baseTick` (before `aiStep`/travel). `touchingWater` then selects `WaterPredictionEngine` in `LivingTicker.travel()` (`prediction/ticker/impl/LivingTicker.java:188-190`). The push then goes through water drag: `0.014 * 0.8 = 0.0112`, which matches the observed predictedDelta.
- Boar reads layer 1 (waterlogged) first and treats it as `FluidState(WATER, 8/9, 8)` (`compensated/world/CompensatedWorldImpl.java:37-45`). Level 8 also triggers the "falling" `-6` branch in `getFlow` (`data/FluidState.java:58-67`).

### bedsim: `liquid.go:212-249, 311-322`
```go
offset := mgl32.Vec3{0.001, 0.401, 0.001}
...
if !liquidIntersects(box, pos, liquid) { continue }
...
func liquidHeight(liquid world.Liquid) float32 {
    if liquid.LiquidFalling() { return 1 }
    return float32(liquid.LiquidDepth()+1) / 9          // dragonfly depth: 8 = source
}
func liquidIntersects(box cube.BBox32, pos cube.Pos, liquid world.Liquid) bool {
    surface := float32(pos[1]) + liquidHeight(liquid)
    return box.Max().Y() > float32(pos[1]) && box.Min().Y() < surface
}
```
The flow strength is `0.014` (water) or `0.0035` (lava), applied as `flow * (strength/len)` (`liquid.go:356-361`).

### Ours
- `liquid.rs:40-43` `liquid_intersects` and `liquid.rs:60-62` in `touching_liquid_blocks`: the same surface test as bedsim.
- `world.rs:55-57` `height()`: falling is 1.0, otherwise `(depth+1)/9`. `depth = 8 - (liquid_depth & 7)` (`crates/acacia-bot/src/world/adapter.rs:25-26`). In Bedrock raw terms that is `(9 - raw)/9`: source 1.0, raw 7 = 2/9. Boar's `(8-raw)/9` is 1/9 lower, but Boar never uses it for contact.

**Worked case (feet exactly on a block top, y=64.0):** the shrunk minY is 64.401, so `floor` gives 64 and the puddle cell (64) is scanned by both.
- Ours requires `64.401 < 64 + (9-raw)/9`, i.e. raw ≤ 5. Thin water at raw 6 (height 3/9) and raw 7 (height 2/9) is missed.
- Boar counts it, so it puts the player in water travel and pushes 0.014 along the flow.

**Proposed change.** In `touching_liquid_blocks` (`liquid.rs:60-65`), drop the `liquid_intersects` condition, keeping `l.kind == kind` only (cell overlap of the shrunk box). This also makes `apply_liquid_flow` include those cells, because it consumes the same list (`movement.rs:25, 67`). Optionally mirror Boar's negative-height exclusion for raw falling levels 9..15 in the top row (low value).

Caveat: bedsim/oomph and Boar disagree here, and neither is authoritative for the real client. Boar is tuned against real clients, and a false push on every player standing in a puddle would be very visible. That points to cell-only being the real-client behaviour, but this is **inferred, not verified**. A BDS A/B test (stand still in raw-7 flowing water and watch for server corrections of about 0.0112/tick) would settle it.

---

## 2. Surface bobbing, jump release (mismatch 2)

### Boar rules, all quoted
- **Water jump** (`player/BoarPlayer.java`, `jump()`):
  ```java
  boolean canJumpInWater = this.getFluidHeight(Fluid.WATER) != 0, canJumpInLava = this.isInLava();
  if ((jumping || autoJumping) && (canJumpInWater || canJumpInLava)) {
      vec3 = vec3.add(0, 0.04F, 0);
  } else if (this.onGround && this.getInputData().contains(PlayerAuthInputData.START_JUMPING)) {
      vec3 = this.jumpFromGround(vec3);
  }
  ```
  `jumping` is the held `JUMPING` flag and `autoJumping` is `AUTO_JUMPING_IN_WATER`. `fluidHeight != 0` is the same as `touchingWater`.
- **Water drag and gravity** (`prediction/engine/impl/fluid/WaterPredictionEngine.java:43-74`): `velocity.multiply(f, 0.8F, f)` with `f` = 0.8, or 0.9 when sprinting or `STOP_SWIMMING`. Then `y -= gravity/16` unless SWIMMING. `gravity` = 0.08, or `min(0.08, 0.01)` when `vy<0` with slow falling (`player/data/PlayerData.java:201-203`). The result is 0.005, or 0.000625 with slow falling.
- **Jump out of fluid** (`prediction/ticker/impl/LivingTicker.java:212-228`):
  ```java
  float d = player.position.y;
  ... travel; doSelfMove; finalizeMovement ...
  if (player.horizontalCollision && player.doesNotCollide(vec33.x, vec33.y + 0.6f - player.position.y + d, vec33.z)) {
      player.velocity.y = 0.3F;
  }
  ```
  `doesNotCollide` is `noCollision(box) && !containsFluid(box)`. `containsFluid` (`BoarPlayer.java`) scans `floor(min)..ceil(max)` for **any non-empty fluid cell, with no height test**.
- **wasTouchingWater vs isInWater:** Boar has only `touchingWater`, recomputed at the start of each tick (`EntityTicker.baseTick`). `updateSubmergedInWaterState` is commented out (`EntityTicker.java:35`). There is no lagged or "was" state.
- **After releasing jump:** there is nothing special for a non-swimming player. The only jump-related surface logic is for SWIMMING (`prediction/ticker/impl/PlayerTicker.java:56-76`). Pitch-steer is skipped while `JUMPING`, and `velocity.y = 0` is set if the fluid 0.4 above the feet is empty and `0 < lookY < 0.55`. There is also a hack that zeroes y while `JUMPING` and `ticksSinceSwimming` is in 1..9.
- Sneak in water: `velocity.y -= 0.04` when `touchingWater && SNEAKING` (`PlayerTicker.java:48-50`). bedsim uses `WantDown || WantDownSlow || PressingDescend`.

### Ours vs Boar
The vertical formula matches: `+0.04` before moveRelative, then `*0.8 - 0.005` (`liquid_travel.rs:200-208, 254-270`). The exit boost matches (`liquid_travel.rs:272-281`). **For a pool whose surface cells are sources, ours and Boar give identical results.** Our source height is 1.0, which equals Boar's "whole cell". So Boar's rules do not explain a +0.5 server lead in a source pool.

Differences that could matter:
1. **Surface cells that are not sources** (flowing edge of a pool, a river): ours leaves water when `feet+0.401 >= y + (9-raw)/9`, while Boar (cell-only) leaves when `feet+0.401 >= y+1`. Ours then gets air gravity (0.08, x0.98) earlier. It gains roughly -0.2 to -0.3 downward speed before it re-enters, and water drag then carries it about `v*0.8/0.2` ≈ 0.8-1.2 deeper. This produces the observed sign ("server higher") and a plausible magnitude. The fix is the same as in §1.
2. **Exit-probe liquid test**: `contains_any_liquid` (`liquid.rs:72-75`) uses `liquid_intersects`, while Boar's `containsFluid` is cell-only. With partial-height water in the raised box, ours grants the 0.3 boost where Boar does not (ours ends higher, the opposite sign). Align it anyway.
3. **`effective_jumping`** (`input.rs:177`) = `f.jumping` only. bedsim (`simulation.go:426`) uses `Jumping || AutoJumpingInWater || AscendBlock`, and Boar uses `JUMPING || AUTO_JUMPING_IN_WATER`. Our client never sends either extra flag (`crates/acacia-bot/src/movement/auth_input.rs:48-52`), so this is not the cause.
4. Slow-falling water gravity: ours uses a constant 0.005; Boar uses 0.000625 while `vy<0`.

**Unverified (BDS is closed source).** If the pool is all source blocks, none of the above explains it. The next step is a diagnostic, not a guess: log `CorrectPlayerMovePrediction` (server pos, delta, onGround) on the release ticks and compare delta.y:
- about 0.42 means a ground jump was triggered;
- 0.3 means the exit boost;
- `v*0.8-0.005` continuing while ours shows air gravity means a contact-rule difference;

Also record the `liquid_depth` of the surface cells (for example with the `goto` example's liquid dump, `crates/acacia-bot/examples/goto.rs:125`).

---

## 3. Waterfall edge (mismatch 3)

- Falling height: ours 1.0 (`world.rs:56`), bedsim 1.0 (`liquid.go:312-314`). Boar gives 8/9 for level 8 and negative values for 9..15, but ignores height for contact apart from the top-row quirk in §1. **For falling cells, our contact equals Boar's**: cell overlap of the 0.001/0.401-shrunk box. That box has the same x/z range: `floor(min)..floor(max+1)` exclusive (`liquid.rs:53-54`, `EntityTicker.java:72-77`).
- Cells where ours and Boar do differ near a waterfall: the **partial-height flowing water spreading at the base or around the column**. Ours needs the surface above feet+0.401, which excludes raw ≥ 6 at foot level; Boar takes any cell. This is the same root cause as §1 and the same fix.
- Flowing height: ours `(depth+1)/9` = `(9-raw)/9`; Boar `(8-raw)/9`; Java `(8-raw)/9` with 1.0 when the same fluid is above (`FluidState.getHeight`, `data/FluidState.java:19-21`, not used by the contact test).
- Flow for falling cells: Boar's `-6` branch triggers on `level >= 8` if a horizontal neighbour, or the cell above it, is a sturdy face (ice excluded, same fluid excluded) (`FluidState.java:58-89`). Ours uses "any collision box" (`liquid.rs:129-146`), like bedsim `liquidFlowBarrier`. Ours also marks only full cubes as closing faces (`adapter.rs` `solid_faces`). These are minor direction differences, not contact differences.
- If BDS keeps drag in a column of pure falling water where ours exits, the rules are identical. Suspect world state instead: missed `UpdateBlock` or layer-1 updates for flowing water, or `liquid_depth` decoding. Verify by dumping the cells around the player when contact flips.

---

## Proposed edits (ours)
| Where | Change |
|---|---|
| `liquid.rs:60-65` `touching_liquid_blocks` | Remove `liquid_intersects(&bb, pos, &l)`; keep the kind match. This fixes §1, and likely §2 (partial surface) and §3 (spread at the base). |
| `liquid.rs:72-75` `contains_any_liquid` | Cell-only (Boar's `containsFluid` uses `floor(min)..ceil(max)` and any fluid). |
| `liquid_travel.rs:180-186, 268-270` | Water gravity `gravity/16` with slow falling (`0.01` when `vy<0`), to match Boar. |
| `input.rs:177` | `effective_jumping = jumping \|\| auto_jumping_in_water \|\| ascend_block` if those flags are ever sent (bedsim parity). |
| Diagnostics for §2/§3 | Log BDS `CorrectPlayerMovePrediction` and the surrounding liquid cells before changing anything BDS-specific. |

Keep the height test behind a flag if bedsim parity must be preserved. bedsim's tests are synthetic unit tests. `TestFallingLiquidDecayAndHeight` (`liquid_test.go:1322`) pins the heights: source 1, falling 1, `Depth:1` = 2/9. No bedsim test found here checks thin-water contact against a real-client recording.
