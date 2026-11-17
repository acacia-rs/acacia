# Camel (BDS 1.26.52, static only: bdsre + the vanilla behaviour pack; nothing checked live yet)

Sources: `behavior_packs/vanilla_1.26.30/__brarchive/entities.brarchive` `camel.json` (newest copy), bdsre on
`.testserver/bds-blocks/bedrock_server.exe`. bdsre labels added: `Camel::normalTick` 142a423b0, `Camel::Camel`
142a41d10, `DashActionSystem::applyDash` 144831220, `DashSystem::countDownCooldown` 14484c160.

### Not client-predicted: BDS moves a camel itself
`VehicleInputIntentComponent::mIsClientPredicted` is written only by `SetIsClientPredictedHorseSystem` (1483c6d00:
player-controlled, TAMED and SADDLED, view requires `HorseFlagComponent`) and `SetIsClientPredictedBoatSystem`
(1483c6100, view requires `BoatFlagComponent`). `Camel : Animal` (not `Horse`); its ctor (142a41d10) emplaces
`CamelFlagComponent`, `CanVehicleSprintFlagComponent` and riding offsets, never `HorseFlagComponent` (only the horse
ctor 142a475f0 does). So BDS runs the camel's travel from the rider's input like a pig's. The camel entry in
`riding/input.rs` `PREDICTED` came from Geyser, not a capture, and is probably wrong for BDS (see Unknown 1).

### Data (`camel.json`)
- Box: standing 1.7 × 2.375; sitting 1.7 × 0.945 (baby: 2.11 × 3.11 / 2.11 × 0.944, scale 0.45).
- `minecraft:movement` 0.09 (fixed, no range). `jump.static`; no `horse.jump_strength`.
- `variable_max_auto_step`: base 1.5625, **controlled 1.5625**, jump_prevented 0.5625 (horse: 1.0625 controlled).
- Saddled group: `input_ground_controlled`, `behavior.player_ride_tamed`,
  `dash_action {cooldown_time 2.75, horizontal_momentum 20, vertical_momentum 0.6}` (`can_dash_underwater` and
  `direction` unset: false, entity).
- `rideable`: 2 seats, driver (0, 1.905, 0.5), back (0, 1.905, −0.5); `is_tamed` always.

### Travel (same systems as the horse)
- **Yaw/pitch/move vector:** `VehicleMoveFromIntent::yawFromRider_pitchHalf_strafeHalf_backQuarter` (14485d050), called
  by `VehicleDesiredMoveDirectionSystem` for any player-controlled ground vehicle: the horse's easing
  (`0.7·max(0.18, (45 − min(|d|,45))/90)`), pitch × 0.5, strafe × 0.5, backward × 0.25. Nothing camel-specific.
- **Speed** (`MobTravelUpdateSpeeds::airSpeed_tenthOfSpeed_vehicleSprint` 142f17890, runs when WASD_CONTROLLED):
```
air   = speed · 0.1                       (speed: last value of this field, see Unknown 4)
speed = movement (0.09)
        + 0.1  if the controlling rider is SPRINTING and the vehicle has CanVehicleSprintFlag (camel)
               and the camel has no HAS_DASH_COOLDOWN (flag 108)
```
  `SetRequestSprintTriggerSystem::fn0` (1483ea470): the driver of a `CanVehicleSprint` vehicle may sprint with no
  food check; other passengers may not.
- **Step:** 1.5625 with a controlling rider.

### Dash (charged by the horse's jump key logic, released as an impulse)
1. Charge: `PassengerJumpTrigger::chargeJumpRidingScale` (143b722e0), the horse's formula, but it **does nothing
   while the camel has HAS_DASH_COOLDOWN** (vehicle flag byte 0xd & 0x10 = 108): no charging, no release.
2. Power: `MobOnPlayerJumpServerSystem::fn0` (14665b7f0): `p = amount >= 90 ? 1 : 0.4 + 0.4·amount/90` into
   `JumpPendingScaleComponent`.
3. Trigger: `TriggerJumpSystem::fn0` (145eef9b0; view 145ef2e20). Needs pending `p > 0`, the vehicle WASD_CONTROLLED,
   CAN_DASH (46), not HAS_DASH_COOLDOWN, `DashActionComponent`, **on the ground** (or in water with
   `can_dash_underwater`, false for camels), not `WasInLava`. CAN_POWER_JUMP (45, horses) is checked first. Emplaces
   `TriggerJumpRequest` (its byte = the honey-type block check of `JumpFromGroundSystem::fn1` 14661fbb0) and
   `DashJumpFlag`. No 0.42 ground jump is added.
4. Impulse: `DashActionSystem::applyDash` (144831220), view (144846330) TriggerJumpRequest, DashAction,
   MovementAttributes, ActorRotation, MobEffects, StateVector, JumpPendingScale, ActorDataFlag, dirty flags,
   DashJumpFlag:
```
f  = block under reduces jumps (honey) ? 0.6 : 1.0
jb = jump boost ? (amplifier + 1)·0.1 : 0
h  = 20 · p · movement            (MovementAttributes.mMovementSpeed = attribute current value, 142c20180;
                                   sprint bonus not included; 1.8 at full power)
v  = 0.6 · p · f · (1 + jb)
yaw = camel ActorRotation.y;  i = int(yaw·0.017453292·10430.378)
delta.x += −h·SIN[i & 0xffff];  delta.z += h·SIN[(i + 16384) & 0xffff];  delta.y += v     (added, not set)
pending p = 0;  set HAS_DASH_COOLDOWN (108);  DashCooldownTimer = cooldown
```
   No forward key needed (unlike the horse leap): always along the camel's facing.
5. Cooldown: `DashSystem::countDownCooldown` (14484c160) decrements the timer each tick; when it was < 2 it removes
   it, plays sound 0x1bc and clears 108. `Camel::normalTick` (142a423b0) stamps `mDashCooldown` = now + 55, length 55
   (0x37) when 108 rises: cooldown 2.75 s = 55 ticks.

### Sitting
- `Camel::normalTick`: state = SITTING (24) | BABY (11)·2; on change sets actor flag 118 (likely the dash/rider
  lock, unconfirmed), swaps the riding offsets (`root_standing`/`driver_seat`/`back_seat`), and fires
  `minecraft:start_sitting`/`minecraft:stop_sitting`, which swap the collision box (2.375 ↔ 0.945) at once. No pose
  transition ticks seen server-side.
- `MobTravelImmobileFilterSystem` (142535070) checks NOAI, not SITTING: sitting by itself does not stop travel.
  Whether a driven camel can sit, or stands when mounted, was not found (see Unknown 3).

### Seated PlayerAuthInput (inferred from the server, not captured)
- Probably the pig shape: no `ClientPredictedVehicle`/`vehicle_rotation`/`predicted_vehicle`, position = seat, WASD
  in the move vector, plus `Jumping`/`JumpDown`/`JumpReleased` (the dash charge reads the rider's input flag
  0x10 at +0x60) and the sprint flags (sprint speed bonus).

### Unknown
1. What the vanilla client sends while driving a camel (does it set `ClientPredictedVehicle`?) and whether BDS
   tolerates a reported camel position. Capture one drive with a dash.
2. System order of `DashActionSystem` against gravity/travel in the tick, hence the start `delta.y` the 0.6·p is
   added to (−0.0784 if gravity already ran on the ground), and whether the release tick or the next one leaves the
   ground; also whether the dash yaw is the eased yaw of that tick.
3. Sitting while ridden: the `random_look_around_and_sit` goal's rider check (goal class not located; flag 24 is not
   set through the `142535f40` setter outside `Actor::load`).
4. Air speed: `speed·0.1` reads the travel-speed field before this tick's update, so a sprinting camel may get
   0.019 in the air instead of 0.009.
5. `cooldown_time` → ticks conversion in the schema (14245f730) not traced; 55 is from `Camel::normalTick`.
6. `chargeJumpRidingScale`'s `*(param_5 + 0x15) == 1` gate and actor flag 118's meaning.
