# Riding, fishing, elytra: vanilla packet sequences

Settled by the vanilla capture of 2026-10-02 (Windows 1.26.52 on BDS; `vanilla-actions-2026-10-02.md` sections Pig,
Boat, Fishing, Elytra; full per-tick inputs via `capdump --pai all`). Sample sizes: 2 pig mounts/dismounts, 1 boat,
4 casts with 1 catch, 2 glides, 2 rockets. Items still marked **UNKNOWN** were not in the capture.
Code: `acacia-bot/src/riding/`, `fishing.rs`, `elytra.rs`, trackers `state/riding.rs`, `state/fishing.rs`.

Server-side sources used where the capture is silent:
- GT: gophertunnel `minecraft/protocol` (player_auth_input.go, interact.go, actor_event.go, entity_link.go)
- MD: Mojang bedrock-protocol-docs @d702218e (`PlayerMovementOverview.md`, `changelog_776`)
- GE: Geyser `BedrockInteractTranslator`, `BedrockInventoryTransactionTranslator`, `BedrockPlayerAuthInputTranslator`
- NK/PNX: Nukkit / PowerNukkitX `EntityFishingHook`, `ItemFishingRod`, `PlayerAuthInputHandler`
- DF: Dragonfly `handler_player_auth_input.go`, `item/elytra.go`; BS: oomph-ac/bedsim `simulation.go`

## Riding
Mount (`Bot::mount`):
1. Hover `Interact MouseOverEntity {target, position = ray hit}` while the crosshair is on the mob, then
   `InventoryTransaction ItemUseOnEntity {Interact, held item, eye player_pos, world click_pos}` with an
   **empty hand** (pig and boat). The client's `Animate "interact"` comes after the server's echo (generic
   entity-click shape, owned by the interact code, not riding).
2. Server: `SetEntityLink` type 1 +95–110 ms with SetEntityData in the same ms (rider flags, RiderSeatPosition).
3. The client keeps sending standing inputs for 0–2 ticks after the link (pig 2, 0; boat 2), then seats. On
   the **tick after the first seated one**: `ClientCameraAimAssist Clear` + `Interact MouseOverEntity 0` (all 3
   mounts; 108–336 ms after the link). Held-button ClickBlock repeats may follow (generic).
- Saddling: the same ItemUseOnEntity holding the saddle (374); the server answers InventoryContent + PlayerHotbar
  select true, the client re-sends MobEquipment (generic hotbar rule). No saddle API; `interact_entity` with one held.

Dismount (`Bot::dismount`), both pig dismounts and the boat exit, **no sneak flags anywhere**:
`Interact LeaveVehicle {target = vehicle runtime id, has_position true, position}` + that tick's input, which
already stands at `position` (same value, eye level = dismount feet + 1.62; vanilla's feet were block top +
0.001), with no vehicle fields and no VerticalCollision (back the tick after); delta = standing (0,-0.078,0)
plus any walking. `ClientCameraAimAssist Clear` +0–32 ms. SetEntityLink type 0 +78–185 ms.
We send Interact, aim assist, then the input; physics bots step off at the server's exit spot
(`riding/exit.rs`, BDS's exit search: seat x/z + first free of 24 offsets, feet = floor + 0.001; falls back
to `Seat::leaving_feet`), idle bots at the position their inputs already report. Live on BDS (pig, boat; 2026-10-02)
the server's position on the first tick off the seat equalled ours exactly; BDS may then push the player off the
boat's box (x −0.075) without a correction. A sneak press was not captured; we no longer send one.
Mounting: BDS checks eye→vehicle ≤ 6.7 and optionally the look ray against the vehicle's box; the bot aims at
per-kind hitboxes (`interact/geometry.rs`), since aiming at a player-sized box above a boat made mounts flaky.

### PlayerAuthInput while riding (`riding/input.rs`)
- Every seated tick keeps `BlockBreakingDelayEnabled` + `VerticalCollision` (pig and boat).
- **Pig** (server-driven, also when steered with a carrot on a stick): no vehicle fields; position = the seat
  (vehicle + RiderSeatPosition), following the pig as it moves; delta (0,0,0) except the first seated tick, which
  still carries the standing (0,-0.078,0). Move vector/flags are the player's WASD.
- **Boat** (client-predicted): from the **first** seated tick `ClientPredictedVehicle` + `vehicle_rotation =
  (0, vehicle yaw)` (yaw unwrapped, 135 → 766) + `predicted_vehicle = unique id`; position/delta = the boat's.
  PaddlingLeft only with Left, PaddlingRight only with Right. Other predicted vehicles (horses, camels) assumed
  the same: **UNKNOWN**.
- Vehicle position source: `Trackers::entities` moves when enabled, else the spawn pose kept by `state::Riding`.
- Never send MoveActorAbsolute, PlayerInput, row Animates or PassengerJump (legacy; Geyser rejects them).
- Physics bots stop simulating while seated; vehicle physics is the physics session's
  (docs/physics-handoff-glide-vehicles.md).

## Fishing (`Bot::fish`)
- **Cast and reel are identical**: `Animate SwingArm "useitem"` then `ItemUse ClickAir {trigger UnknownValue,
  face 255, block (0,0,0), prediction Failure}`, and `StartUsingItem` on the next input only (4/4 casts, 4/4
  reels). No PlayerAction. Built by `Bot::use_rod` / `fishing::vanilla_click_air`.
- Server per use: CompletedUsingItem {395, -1} + InventoryTransaction ItemRelease Consume + PlayerHotbar; a cast
  also AddEntity `minecraft:fishing_hook` +96–186 ms.
- BDS hook events: many `FishHookTease`, then `FishHookPosition` (~1.5 s, the fish approaching), then one
  `FishHookHook` = the bite. Geyser sends none (the tracker also counts a bobber dip, `state/fishing.rs`).
- Timing: rod selected 1052 ms before the first cast; bite → reel **518 ms** (n=1). BDS's bite lasts 10–29 ticks,
  so `human::FISH_REACTION` is 240–350 ms (reaction + round trip under ~450 ms); auto-eat never switches slots
  while the own hook is out (it would remove the hook); recast 204 ms after the catch reel (not modelled: `fish` returns after the
  pickup and the caller recasts). Cast → early reel 1.6–5.4 s were the tester's choice, not a habit to fit.
- Catch on BDS: rod Damage +1 with a new stack id, `PlayerHotbar select true`, client MobEquipment +97 ms
  (generic hotbar rule), salmon AddItemEntity → TakeItemEntity +380 ms, 3 xp orbs. No slot update for the
  catch: `fish` reports the own `TakeItemEntity` of loot whose `AddItemEntity` had `is_from_fishing`.

## Elytra
- Equip (`Bot::equip_elytra`): vanilla dragged it in the inventory screen (`Take → Cursor`, `Place → Armor 1`);
  that is what we send. Right-click equip: not captured.
- Start (`Bot::start_gliding`): a jump press in the air; that input carries JumpPressedRaw, JumpDown, Jumping,
  JumpCurrentRaw, **WantUp**, `StartGliding` (no StartJumping, no PlayerAction StartGlide). There is no
  persistent gliding flag on later ticks. The server's SetEntityData (GLIDING, which the bot waits for) came +31 ms.
- Stop (`Bot::stop_gliding`): mid-air on a jump press (`StopGliding` + the press flags + WantUp); after landing
  `StopGliding` alone on the tick after the first ground tick (that tick has no VerticalCollision).
- Boost (`Bot::boost_with_firework`): MobEquipment selecting the rocket 608 ms before, then `Animate "useitem"`
  + the same ClickAir as the rod, **no StartUsingItem**. BDS: MovementEffect GLIDEBOOST (duration 56, 42)
  +83–241 ms, PlayerHotbar select true, AddEntity `fireworks_rocket`; the client re-sends MobEquipment with
  count−1 ~250–490 ms later (generic hotbar rule).
- PlayerArmorDamage about 1/s while gliding.

### Elytra physics (`acacia-physics` `motion.rs::simulate_glide`; BDS 1.26.52, bdsre 2026-10-03)
Per tick while gliding, BDS `glideTravelVelocity` (0x1484403b0). Drag and boost apply before the move:
```
G = -0.08 (-0.01 with slow falling); sin/cos from the 65536-entry table; p, y = current pitch, yaw (rad)
look = (sin(-y-PI) * -cos(-p), sin(-p), cos(-y-PI) * -cos(-p))    // from -p: table index can differ
hzSq = lookX^2 + lookZ^2; hz = sqrt(hzSq); c2 = cos(p)^2; velHz = sqrt(vx^2 + vz^2)
if vy > -0.5: fallDistance = 1
vy = vy - (0.75*c2 + -1) * G
if vy < 0 && hzSq > 0: a = vy * -0.1 * c2; vy += a; vx += lookX*a/hz; vz += lookZ*a/hz
if p < 0:              a = velHz * -sin(p) * 0.04; vy += a*3.2; vx -= lookX*a/hz; vz -= lookZ*a/hz
if hzSq > 0: vx += (lookX/hz*velHz - vx)*0.1; vz += (lookZ/hz*velHz - vz)*0.1
if boosting: v += look*0.1 + (look*1.5 - v)*0.5   (per axis)
vx *= 0.99; vy *= 0.98; vz *= 0.99; then move + collide
```
- **Boost**: the client predicts 20 ticks from the use tick. BDS then sends MovementEffect GLIDEBOOST,
  stamped with the input tick. Its `duration` is twice the boost: the rocket's lifetime
  `10*(flight+1) + rand(6) + rand(7)` ticks, so 20–31 for flight 1. Live: duration 44 boosted 22 inputs.
  The boost only counts down; landing or water don't clear it.
- **Start/stop**: the server runs the glide itself. It starts on the press for a player with a
  fly-enabled chest item who is airborne and not riding. It stops on ground, water, riding, flying or a
  wall climb. It does not wait for the client's StopGliding. No levitation check was found.
- **Live (actions `glide`, `glide_water`)**: 0 corrections from start to landing over 12 glides (level,
  boost, climbing turn, dive, water landing).
- **UNKNOWN**: wall-impact damage (`GlidingCollisionDamageCalculateSystem`, not decoded), the durability
  threshold, the glide box size.

### Horse (`acacia-physics` `vehicle.rs`, bot `riding/horse.rs`; BDS 1.26.52, bdsre 2026-10-03)
Client-predicted when tamed, saddled and driven by a player. Per tick:
- **Yaw:** eases toward the rider's. With `d = wrap180(riderYaw − yaw)`, `yaw += d·0.7·max(0.18, (45 − min(|d|, 45))/90)`.
- **Pitch:** rider pitch × 0.5.
- **Move vector:** strafe × 0.5, backward × 0.25, forward unchanged.
- **Travel:** the player's ground travel with the horse's `minecraft:movement`. Air speed is movement × 0.1.
- **Box and step:** 1.4 × 1.6; step height 1.0625 with a controlling rider.

Corrections:
- BDS corrects a yaw gap on its own; the correction's rotation is (pitch, yaw).
- It ignores the client's reported delta: sending 0 changed nothing.

Live (`ride_horse`, horse at 0.2): walking, turning, strafing, backing and a 1-block ledge, with 0 corrections.

**Open:** above about movement 0.25, BDS's steady speed drifts from the formula. At 0.3 it was 0.3547751 against our 0.35356, reproducible. At 0.35 two sweeps disagreed: +1% once, no corrections the next time. bdsre shows no extra term. Probe with `HORSE_SWEEP=… ONLY=horse_sweep tools/live-repeat.sh`, or compare against a vanilla capture of a fast horse. The jump charge is not modelled; its formulas are in the bdsre notes (horse jump `143b722e0`, `145e36020`).

## Still open
1. Exit spot: `dismount_mode on_top_center` vehicles and exempt blocks (`blockIgnoredForExit`) are not modelled.
2. Seated inputs for horses, camels, minecarts, striders and back seats; sneak-to-dismount (not captured).
3. Right-click elytra equip; recast pacing after a catch (n=1).
