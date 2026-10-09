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
  PaddlingLeft only with Left, PaddlingRight only with Right. Horses the same, with (pitch, yaw) and the jump
  key's flags; camels: **UNKNOWN**.
- Vehicle position source: `Trackers::entities` moves when enabled, else the spawn pose kept by `state::Riding`.
- Never send MoveActorAbsolute, PlayerInput, row Animates or PassengerJump (legacy; Geyser rejects them).
- Physics bots simulate the horse or boat they drive ("Horse", "Boat" below) and stop simulating on anything else.

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

### Elytra physics (`acacia-physics` `motion.rs::simulate_glide`; BDS 1.26.52, verified live 2026-10-03)
Per tick while gliding. Drag and boost apply before the move:
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

### Horse (`acacia-physics` `vehicle.rs`, bot `riding/horse.rs`; BDS 1.26.52, strict movement, 2026-10-09)
Client-predicted when tamed, saddled and driven by a player. Per tick:
- **Yaw:** eases toward the rider's. With `d = wrap180(riderYaw − yaw)`, `yaw += d·0.7·max(0.18, (45 − min(|d|, 45))/90)`.
- **Pitch:** rider pitch × 0.5.
- **Move vector:** the keys, normalised when two are held, then strafe × 0.5, backward × 0.25. It is **not**
  scaled by the player's 0.98, on the ground or in the air.
- **Travel:** the player's ground travel with the horse's `minecraft:movement`. Air speed is movement × 0.1.
- **Box and step:** 1.4 × 1.6; step height 1.0625 with a controlling rider.

**Charged jump** (bdsre: `PassengerJumpTrigger` 143b722e0, `MobOnPlayerJumpServerSystem` 14665b7f0, `ApplyJumpModifier`
145e36020). The server charges it from the rider's `Jumping` flag; nothing else is sent:
```
held this tick and the tick before: scale = t < 9 ? (t+1)·0.1 : 0.8 + 0.2/(t−8); t += 1
released:  amount = int(scale·100); power = amount >= 90 ? 1 : 0.4 + 0.4·amount/90; t = −10 (counts back up to 0)
on the ground with power > 0:  vy = jump_strength·power (+ jump boost, × 0.6 on honey)
                               forward key held: vx −= sin(yaw)·0.4·power; vz += cos(yaw)·0.4·power (sine table)
```
`jump_strength` is the horse's `minecraft:horse.jump_strength` attribute. The jump leaves the ground on the release tick.

Corrections: BDS corrects a yaw gap on its own (rotation is pitch, yaw) and ignores the reported delta.

Live on strict BDS (viewer, natural horses at movement 0.125–0.21): walking, easing to the rider's yaw, strafing,
backing, all four diagonals, a standing and two running jumps: 2 corrections in about 500 ticks, both 1e-5 apart.
The 2026-10-03 fit (keys × 0.98, a drift above movement 0.25) came from the action-test BDS, whose default
0.5-block acceptance lets the server follow the client: only a strict server shows the rule.

### Boat (`acacia-physics` `boat.rs`, bot `riding/boat.rs`; BDS 1.26.52, strict movement, 2026-10-09)
Client-predicted for its driver. The position is 0.375 above the box's bottom (1.4 × 0.455), the bow points 90° left
of the yaw (`a = (90 − yaw)·π/180`, direction `(sin a, cos a)`), the yaw is never wrapped. Constants from bdsre
(`PassengerBoatPaddleInputSystem` 1483a31c0, `VehicleBoatPaddleInputSystem` 1483a4a10, `BoatMoveFrictionSystem`
1424dffa0, `BoatMoveControlServerSystem` 14250c800, buoyancy 1483a9b40/1483ac050/1483a91e0); order and inputs fitted
to the server's corrections. Per tick:
```
x, y = keys, normalised when two are held; a held forward key makes y = 1 all the same
pull = sqrt(x² + y²); y < 0: x = −x, pull ·= −0.15
left = (x > 0 ? 1 − x : 1)·pull; right = (x <= 0 ? x + 1 : 1)·pull         (x + is the left key)
force = 3·paddle on a stroke's first tick, sign·(|3·paddle| − 0.1) on the next nine; f = force·0.01375
fr = 0.9 in or on water, the block's friction on the ground, 1 in the air (no thrust there)
vx, vz, yawVel ·= fr
forward = fL + fR; torque = 3·fL − 3·fR;  vx² + vz² < 0.01 and torque ≠ 0: forward ·= fr, torque ·= 1.6
yawVel = (torque·10 + yawVel)·fr; yaw += yawVel
vx = (forward·sin a + vx)·fr; vz = (forward·cos a + vz)·fr;  move (Y, X, Z), motion = what was moved
wave += (sqrt(vx² + vz²)·30 + 1)·0.05, ten times that with probability 0.03 (the server's own random)
vy = (vy − 0.04)·0.98
floating (in a water block with none above, below its top): depth = clamp((1 − frac(y))·0.9 + 0.1, 0, 1)
    vy = min(vy·0.7 + 0.05, (depth − (sin(wave) + 1)·0.035 − 0.1)·0.15)
```
- The big waves cannot be predicted, so the vertical position drifts and BDS corrects (a vanilla client too: most
  ticks in the capture). A correction's vertical motion gives the wave's sine back; of the two phases with that
  sine the bot takes the one a correction up to 8 ticks before agrees with. It then replays the inputs sent since.
- The correction carries position, motion, yaw and the yaw's speed (`angular_velocity`).
- Inputs: position and delta are the boat's, `vehicle_rotation = (0, yaw)`, WASD flags, PaddlingLeft/Right with
  Left/Right. The paddling flags are only read for touch with the classic interaction model (bdsre).

Live on strict BDS (viewer), two runs:
- W, a coast, A, D, S, W+A over 262 ticks: 29 corrections. In all 28 with a traced input x, z and yaw were equal
  and only the height was off (the waves: two or three corrections per big wave, more while the speed changes).
- S+D, W+A, W, W+D and a coast over 371 ticks: 14 corrections; 10 height only, one 5e-6 in y, two 1–2e-5 in x or z.
**Not checked:** ice and land friction, collisions with banks, flowing water, the 25-tick underwater ejection.

### Other vehicles
Pigs and minecarts are server-driven (no `ClientPredictedVehicle`): the seated input reports the seat and the
rider's WASD. Riders' seats come with `SetEntityData`: `RiderSeatPosition` (the rider's wire position against the
vehicle, in its frame) and `RiderSeatRotationOffset` (a boat's −90).

## Still open
1. Exit spot: `dismount_mode on_top_center` vehicles and exempt blocks (`blockIgnoredForExit`) are not modelled.
2. Seated inputs for camels, striders and back seats; sneak-to-dismount (not captured).
3. Right-click elytra equip; recast pacing after a catch (n=1).
