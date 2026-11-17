# Riptide, elytra wall impact and durability, dismount exit (BDS 1.26.52, static only)

Sources: bdsre on `.testserver/bds-blocks/bedrock_server.exe` (Windows 1.26.52), LeviLamina headers, vanilla behaviour
packs (`behavior_packs/vanilla_*/__brarchive/entities.brarchive`). Nothing here is checked live yet; each section ends
with the drill that settles it. bdsre labels added: `TridentItem::releaseUsing` 141fa6b10,
`ActorMobilityUtils::startSpinAttack` 141c644b0, `Actor::isInWaterOrRain` 140d2a130, `Actor::getVehicle_isRiding`
140d26680, `StartSpinAttackActionSystem::{vtable 14a92a370, tick 1483dfbf0, applyRiptideImpulse 1483df410}`,
`SpinAttackSystem::_stopSpinAttack` 1483c3070, `glidingCollisionDamage_calc` 148430e20, `Mob::updateGlidingDurability`
1429e8df0, `ItemStackBase::isFlyEnabledElytra` 141a8d060, `armorChanged_updateFlyEnabled` 142f56d90,
`ExitVehicle::onTopCenter_setPos` 14469ea10, `ExitVehicle::onTopCenter_teleport` 14469e670.

## 1. Riptide

### Start (`TridentItem::releaseUsing` 141fa6b10, on the `ReleaseItem` transaction)
- Riptide level `L` = enchant 30 on the trident. `L >= 1` takes the riptide branch (no throw) only if:
  `Actor::isInWaterOrRain` (WasInWaterFlag, or rain at the feet block with a level > 0.15), **not riding**
  (140d26680), and charged: `useDuration − durationLeft > 9` (held >= 10 server ticks).
- Then `startSpinAttack(L)`: `PlayerActionComponent` bit 23 (= `PlayerActionType::StartSpinAttack`) and
  `RiptideTridentSpinAttackComponent = L`; sound `TridentRiptide1/2/3` (180/181/182). **No trident durability use**
  in this branch (the throw branch calls hurtAndBreak 141a56c80; Java damages the trident on riptide).
- The component is a one-tick request: `RemoveSpinAttackResultsComponentSystem` (1483e89a0) clears it and
  `SpinAttackResultsComponent` each tick. Both are in `ReplayableActorInput` (`ComponentBits` 12, 13): part of the
  server-auth movement replay, i.e. the client is expected to predict the launch.

### Impulse (`StartSpinAttackActionSystem` 1483df410)
View: ActorRotation, RiptideTridentSpinAttack, ActorDataFlag, ActorDataDirtyFlags, DamageNearbyMobs, StateVector,
ActorMovementTickNeeded (+ refs OnGround, WasInWater, ActorHeadInWater). Skipped entirely when `142535070` says
immobile (NOAI/immobile/sleeping family). Sine table `SIN` at 0x14cc495f8, 65536 f32:
```
iy = int(yaw·0.017453292·10430.378);  ip = int(pitch·0.017453292·10430.378)     // + yaw, not −yaw−π as glide
cp = SIN[(int(ip_f + 16384.0)) & 0xffff]                                          // ip_f: the float before int()
x = −SIN[iy & 0xffff]·cp;  y = SIN[ip & 0xffff];  z = cp·SIN[int(iy_f + 16384.0) & 0xffff]
len = sqrt(z² + y² + x²);  len < 0.0001: x = y = z = 0 else x/=len, z/=len, y = −y/len
f = (L + 1.0)·0.25·3.0                    // 1.5, 2.25, 3.0
vy = y·f;  if OnGround: (WasInWater && !HeadInWater) ? vy = vy/0.8·0.98 : vy += 0.08
posDelta += (x·f, vy, z·f)                // added, not set
DamageNearbyMobs.ticks = 0; set actor flag 56 (DAMAGENEARBYMOBS); dirty; ShouldUpdateBoundingBox
```
No Java `move(0, 1.1999999, 0)` push: the ground case is the `+0.08` (or water-drag undo) on vy.

### During the spin
- Box: `UpdateHorizontalPoseSystem` (142c13980) makes the box horizontal (height = width, 0.6) for actor flags
  GLIDING 32, DAMAGENEARBYMOBS 56, SWIMMING 57, CRAWLING 114. acacia `collision_dims` (pose.rs) needs the spin too.
- No special travel: the normal air/water travel, drag and collisions run. Mob hits: `SpinAttackFetchNearbyMobs`
  (box inflated 1 each way), `DamageNearbyMobsServerSystem`.
- `PostDamageNearbyMobsSystem::fn0` (1483e5ab0), per tick while flag 56:
  ```
  hit n mobs this tick: posDelta *= −0.2 (n times!), fallDistance = 0, stop
  else HorizontalCollision: stop
  t = ticks++;  t >= 19 → stop (20 ticks);  t >= 5 && OnGround → stop
  not spinning: ticks = 0
  ```
  Stop (`_stopSpinAttack` 1483c3070): ticks 0, clear flag 56, box update, `PlayerAction` bit 24 (StopSpinAttack).

### Client
- Wire: select trident, `ItemUse ClickAir` (+ `StartUsingItem` flag next input), hold >= 10 ticks (send the release
  on tick 11+ to be safe against arrival jitter), `InventoryTransaction ReleaseItem Release` (`Bot::release_item`).
- `PlayerAuthInput` flags `StartSpinAttack` (56) / `StopSpinAttack` (57) exist (acacia-proto `InputData`). No
  server reader was found; like StartGliding, BDS likely runs the spin from the transaction alone. **UNKNOWN**:
  whether the vanilla client sends them and on which tick, and which input tick the impulse lands on (release is
  handled between inputs, so the impulse should land on the next input's movement tick, with that input's rotation).
- **UNKNOWN**: order of `StartSpinAttackActionSystem` against travel. The `+0.08`/`/0.8·0.98` look like undoing this
  tick's gravity/water drag, i.e. it runs before travel's drag; confirm by the drill.

### Port
- acacia-physics: `Input::riptide: Option<u8>` (level); `PlayerState::spin_ticks: Option<u8>`. In `simulate_movement`
  (movement.rs) add the impulse before `travel` on the release tick (or after, per the drill); `collision_dims`
  horizontal while spinning; after the move in `sim.rs::simulate_core`: stop on `collide_x || collide_z`, on
  `ticks >= 19`, on `ticks >= 5 && on_ground`. Entity hits: not simulated (correction).
  Use `sinf.rs`'s table index rules (index from the float product, `& 0xffff`).
- acacia-bot: `Bot::riptide()` in `interact/item_use.rs` next to `release_item`: checks trident + level from the
  stack's enchants, water/rain, not riding; `use_item` + `StartUsingItem`, wait 11 ticks, `release_item`, set the
  physics input; `movement/auth_input.rs` sets `StartSpinAttack`/`StopSpinAttack` if the capture shows them.

### Drill
Strict BDS, flat pool 2 deep, bot standing in water and on dry ground in `/weather rain`, tridents L1–L3 via
`/give` + `/enchant`. Pitch {−90, −30, 0, 30}, yaw {0, 45, 137}. Log every correction and the server position for 25
ticks after the release; fit (a) the tick the impulse lands, (b) before/after-travel order, (c) ground `+0.08`
case (on ground, dry, rain) vs `/0.8·0.98` (feet in water, head out), (d) the 20-tick and ground-after-5 stops,
(e) wall stop (launch at a wall 3 blocks away), (f) trident damage unchanged. Capture a vanilla client riptide
(`capdump --pai all`) for the input flags.

## 2. Elytra

### Wall impact (`GlidingCollisionDamageCalculateSystem` 148442e10 → 148430e20; hurt: 148444650)
View: MoveRequest, StateVector, GlidingTravelFlag, HorizontalCollisionFlag, MobFlag. Per tick after the move:
```
pre  = MoveRequest.mPreviousHorizontalDelta     // = posDelta.xz handed to the move (TravelMoveRequestSystem 145eec7f0)
post = StateVector.posDelta.xz                  // after collision (blocked axis 0)
d = (|pre| − |post|)·10 − 3;   d > 0: damage d (float), cause 21 FlyIntoWall, sound 0x18 if d > 4 else 0x19
```
It does **not** change movement (no velocity write). Client predicts nothing but health; the server's d comes from
its own replay, so it equals the client's when the glide is exact. Avoid it in planning: `pathfind/flight/solve.rs`
already prices walls; it could price `d` directly.

### Durability (`Mob::updateGlidingDurability` 1429e8df0, from the actor tick)
- Server only, while actor flag GLIDING, chest item is the elytra: `hurtAndBreak(1)` when
  `FallFlyTicks % 20 == 19` (19th glide tick, then every 20), then `setArmor(chest)`. Unbreaking applies (server
  random): not predictable with Unbreaking.
- `FallFlyTicks` (`glideFallFlyTicks` 148430db0, GlideInputSystem): +1 each gliding tick, 0 otherwise.
- Break point: `isFlyEnabledElytra` 141a8d060 = is elytra and `damage < maxDamage − 1` (431 of 432), evaluated on
  armor change (`armorChanged_updateFlyEnabled` 142f56d90 → `ArmorFlyEnabledFlagComponent`).
- Stop: `StopGlidingIntentSystem::fn0` (1483fd990 args) keeps gliding only while: not OnGround, ArmorFlyEnabled,
  not WasInWater, not PlayerInputRequest+0x22 (flying), not a passenger, not (jump held && FallFlyTicks >= 11, unless
  VanillaClientGameplay+0x15 / PlayerInputRequest+0x24), and a powder-snow rule (flags 18/19 + feet block
  powder_snow). Any failure: `PlayerAction` bit 16 (StopGlide). So reaching damage 431 stops the glide on the
  server's next StopGlidingIntent run, with no client message. The client sees the damage via the armor slot update
  (PlayerArmorDamage / InventorySlot).
- Port: `PlayerState::fall_fly_ticks` (state.rs) counted in `movement.rs::glide`; `Equipment::elytra_damage` +
  `elytra_unbreaking`. Predict: on the tick `fall_fly_ticks % 20 == 19` with damage 430 and no Unbreaking, glide
  ends on the next tick (`stop_gliding`). With Unbreaking: stop when the armor update says 431. Bot: `elytra.rs`
  refuses `start_gliding` at damage >= 431.
- Drill: elytra `/give` at damage 425 (`/replaceitem ... elytra 1 0 {}` + anvil-free: `/give` data value 425),
  glide off a 100-block tower, log the tick of each PlayerArmorDamage, the first server tick without GLIDING, and
  corrections around it; repeat at FallFlyTicks phases. Wall impact: glide at 1.0 b/t into a wall at 0°/45°, log
  health against `d`.

## 3. Dismount exit

### Default mode (boat, horse, camel, minecart, pig, strider, …: all but happy_ghast and cushion)
- `ServerPlayerStopRidingInteractSystem::fn0` (1457db120), on `Interact LeaveVehicle` (action 3) with the vehicle id
  matching: `stopRiding(exitFromPassenger)`, then for a default-mode vehicle sets
  `ServerPlayerMovementComponent::mAcceptClientPosIfWithinDistanceSq = 225` (15 blocks), unless the tick policy
  overrides: **the server takes the client's next reported position if within 15 blocks of its own**. That is why
  the 2026-10-02 pig/boat exits had no correction.
- `ExitVehicleSystemImpl::_tickExitVehicle` (1446c1260) still computes the server's spot (ActorSetPositionRequest,
  feet + 0.001): seat = vehicle pos + `RiderSeatPosition` rotated by vehicle yaw; search picked by
  `ExitVehicle::pickSearchByVersion` (142cb7130: `search24_modern` for current versions, `searchUp3_legacy` for old
  base game versions); minecart ejected by an activator rail: `search3_railEject_legacy`/`search24_modern`. No spot:
  passenger position + `OffsetsComponent.mHeightOffset`. Rotation is wrapped to [−180, 180).
  Order of the 8 horizontal offsets from the vehicle's motion (still if |v|² <= 9.801e-5): as `riding/exit.rs`.
- `floorHeightAt` (142cb67a0) per candidate cell:
  ```
  cell shape non-empty, top−y < 1        → floor = top − y           (slab, carpet, snow)
  cell shape full height:  blockIgnoredForExit(cell) ? check below : blocked
  cell empty / ignored → below shape non-empty: floor = top_below − y, valid if >= 0 and !blockIgnoredForExit(below)
  ```
  `blockIgnoredForExit` (142cb6980): `BlockProperty::Climbable` (bit 33: ladder, vines, …), a `Trapdoor` (bit 13)
  whose state 0x14cd25550 (open bit) is set, `minecraft:scaffolding` (0x14ccdd430), `minecraft:powder_snow`
  (0x14ccdb6e0). acacia `riding/exit.rs::free_spot` lacks this: ignored blocks are neither floor nor obstacle there.
- Server message: none expected (position request, then acceptance window). **UNKNOWN**: whether a MovePlayer
  follows when the client's report is > 15 blocks off.

### on_top_center (`DismountMode` 1; vanilla: `minecraft:happy_ghast`, `minecraft:cushion`)
- Sets actor flag 123 `DoesServerAuthOnlyDismount` and `DoesServerAuthOnlyDismountFlagComponent` (14241f400); the
  ExitVehicle view excludes such vehicles and the acceptance window is **not** opened.
- Spot (14469ea10 / 14469e670): `(centre x of vehicle AABB, AABB max y [+ passenger height offset], centre z)` +
  the Vec3 global at 0x14ccb0c0c (likely zero). One path writes ActorSetPositionRequest, the other a
  `TeleportToRequest` (→ expect a MovePlayer teleport). Client should assume feet = vehicle box top centre.

### Port
- `riding/exit.rs`: add `blockIgnoredForExit` to `free_spot`/`top` (needs `WorldView` block flags: climbable,
  trapdoor+open, scaffolding, powder snow); `Mount::exit_feet` returns the box top centre for on_top_center vehicles
  (dismount mode from the entity type: happy_ghast, cushion).
- Since BDS accepts the client within 15 blocks, the bot can keep reporting its own spot; the exact search only
  matters for prediction of where the server *would* put it (and for > 15 blocks).

### Drill
Per vehicle (boat on land and water, horse, camel both seats, minecart on rails, pig): dismount next to a ladder,
open trapdoor, scaffolding, powder snow, slab, and fully boxed in; log the first post-exit server position and any
MovePlayer. Acceptance: report an exit 3 blocks off the spot (and 16 blocks off) and log corrections. Happy ghast:
log the MovePlayer and its position against the box top centre.
