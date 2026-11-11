# Java 26.3 client-side mount simulation

Source root for every citation: `/home/agent/java-truth/sources/client/net/minecraft/` (decompiled 26.3, Mojang names).
`LE` = `world/entity/LivingEntity.java`, `E` = `world/entity/Entity.java`, `LP` = `client/player/LocalPlayer.java`,
`AH` = `world/entity/animal/equine/AbstractHorse.java`, `CPL` = `client/multiplayer/ClientPacketListener.java`.

## 0. Shared machinery (applies to every mount)

### Authority
- `E:3703` `isLocalInstanceAuthoritative()` → on client `isLocalClientAuthoritative()`; `E:3707-3710`:
  `LivingEntity passenger = this.getControllingPassenger(); return passenger != null && passenger.isLocalClientAuthoritative();`
  `Player:1270` → `isLocalPlayer()`. So the mount is simulated on the client exactly when `getControllingPassenger()` returns the local player.
- `E:3721` `canSimulateMovement()`: default `MoveSimulationType.AUTHORITATIVE_SIDE` (`E:3717`) → `isLocalInstanceAuthoritative()`. No mount overrides it.
- `E:3624` base `getControllingPassenger()` = null; `Mob:222` returns the first passenger only if it is a `Mob` that `canControlVehicle()` and `!isNoAi()` (mob jockeys; never a player).

### Tick order on the client (important for bit-exactness)
- `Minecraft:1797-1805`: `level.tickEntities()` → `tickBlockEntities()` → `player.sendChanges()`.
- `ClientLevel:468-490` `tickNonPassenger`: `entity.commonTick(); entity.tick();` **then** each passenger `commonTick(); rideTick();`.
  So the **mount ticks before the local player** each tick. The mount's `getRiddenInput/tickRidden/getRiddenSpeed` read
  `controller.xxa/zza/jumping/isSprinting()` as the player left them in the **previous** tick's `aiStep`. `controller.getYRot()/getXRot()`
  are whatever the mouse turned to by then (turning happens per frame).
- `E:517-528` `commonTick`: `setOldPosAndRot()`; on client `getInterpolation().interpolate()`; `tickCount++`.
- `E:2433-2439` `rideTick`: `setDeltaMovement(Vec3.ZERO); tick(); getVehicle().positionRider(this)`. The player's own `aiStep` still runs
  (`Player.travel` with `isPassenger()` → `super.travel`, `Player:1389-1391`), but `positionRider` overwrites the position afterwards. The
  player's `onGround`/`horizontalCollision` from that throwaway move are what goes in the `MovePlayer.Rot` packet and feed `shouldStopRunSprinting`.
- `LE:2788-2790` `tick` → `aiStep()`, then body/head turn (`LE:2792-2812`, `Mob:360` `bodyRotationControl.clientTick()`), then rotation wrap of `yRotO`/`yBodyRotO`.

### LivingEntity.aiStep path (mount side) — `LE:3031-3135`
1. `noJumpDelay--`; `if (!isInterpolating() && !canSimulateMovement()) deltaMovement *= 0.98` (not taken when authoritative).
2. Tiny-velocity zeroing for non-players: `|x|<0.003→0`, `|z|<0.003→0`, `|y|<0.003→0` (`LE:3055-3066`).
3. `applyInput()` → `xxa *= 0.98F; zza *= 0.98F` (`LE:3166-3168`). `isImmobile()` zeroes the mount's own `jumping/xxa/zza` (harmless — ridden input ignores them).
   Server AI (`serverAiStep`) is skipped on the client.
4. Jump block (`LE:3085-3112`): uses the **mount's** `jumping`, which is only ever set by server AI (`JumpControl`); on the client it stays false → `noJumpDelay = 0`.
5. `Vec3 input = new Vec3(xxa, yya, zza)`; `if (getControllingPassenger() instanceof Player controller && isAlive()) travelRidden(controller, input)` (`LE:3123-3124`).
6. `applyEffectsFromBlocks()` when authoritative (`LE:3129-3131`); `calculateEntityAnimation` on client.

### travelRidden — `LE:2624-2644`
```java
Vec3 riddenInput = this.getRiddenInput(controller, selfInput);
this.tickRidden(controller, riddenInput);
if (this.canSimulateMovement()) {
   this.setSpeed(this.getRiddenSpeed(controller));
   this.travel(riddenInput);
} else {
   this.setDeltaMovement(Vec3.ZERO);
}
```
Defaults: `tickRidden` no-op, `getRiddenInput` returns `selfInput`, `getRiddenSpeed` returns `getSpeed()`. Every mount below overrides `getRiddenInput`.
Gotcha: `Mob.setSpeed` (`Mob:441-444`) also does `setZza(speed)` — the mount's own `zza` becomes the ridden speed and is then decayed by
`applyInput` next tick. No mount reads `selfInput`, so this only matters if you port `Mob` state literally.

### Standard travel (horses, camels, pig, strider, nautilus on land)
- `LE:2412-2420` `travel`: `shouldTravelInFluid(fluidAtBlockPos)` → `travelInFluid`; else `isFallFlying` → elytra; else `travelInAir`.
  `LE:2426-2428` `shouldTravelInFluid = isInLiquid() && isAffectedByFluids() && !canStandOnFluid(fluidState)`.
- `LE:2450-2482` `travelInAir`: block friction `computeModifiedFriction(block.getFriction(), FRICTION_MODIFIER)` when `onGround`, else `1.0F`;
  `handleRelativeFrictionAndCalculateMovement` (moveRelative, climbable clamp, `move(SELF)`); gravity `movementY -= getEffectiveGravity()`
  (or levitation / unloaded-chunk branch `-0.1`/`0`); drag `friction = blockFriction * computeModifiedFriction(0.91F, AIR_DRAG_MODIFIER)`,
  vertical `computeModifiedFriction(0.98F, AIR_DRAG_MODIFIER)`.
- `LE:2704-2714` speed used by moveRelative:
  ```java
  if (this.onGround()) return blockFriction > 0.6 ? this.getSpeed() * (0.21600002F / (blockFriction*blockFriction*blockFriction)) : this.getSpeed();
  else return this.getFlyingSpeed();
  // getFlyingSpeed:
  return this.getControllingPassenger() instanceof Player ? this.getSpeed() * 0.1F : 0.02F;
  ```
  (Player-ridden mounts get 10 % of ridden speed in the air, not 0.02.)
- `LE:2484-2521` `travelInFluid`/`travelInWater`: `slowDown = isSprinting() ? 0.9F : getWaterSlowDown()` (0.8 default), speed `0.02F`,
  `WATER_MOVEMENT_EFFICIENCY` lerp (halved off ground), dolphin's grace 0.96, `moveRelative`, `move`, climbable 0.2,
  `multiply(slowDown, 0.8F, slowDown)`, `getFluidFallingAdjustedMovement`, `jumpOutOfFluid`. Then `floatInLiquidWhileRidden` (`LE:2556-2560`):
  ```java
  if (this.isVehicle() && this.is(EntityTypeTags.CAN_FLOAT_WHILE_RIDDEN) && this.isInFluidDeeperThan(this.getFluidJumpThreshold(), fluidTag))
     this.setDeltaMovement(this.getDeltaMovement().add(0.0, 0.04F, 0.0));
  ```
  `CAN_FLOAT_WHILE_RIDDEN` = horse, zombie_horse, mule, donkey, camel, camel_husk (`data/tags/EntityTypeTagsProvider.java:272-273`); fluid tag `ENTITY_FLOATABLE` = water.
  Not skeleton_horse, pig, strider, nautilus.
- `LE:2531-2546` `travelInLava`: `moveRelative(0.02F)`, shallow-lava `multiply(0.5, 0.8F, 0.5)` + fluid-falling adjust else `scale(0.5)`, then `-gravity/4`.
- `E:1781-1791` `getInputVector`: `lengthSqr < 1e-7 → 0`; normalize if `lengthSqr > 1`; scale by speed; rotate by `yRot`.

### Step height — `LE:4026-4029`
```java
float maxUpStep = (float)this.getAttributeValue(Attributes.STEP_HEIGHT);
return this.getControllingPassenger() instanceof Player ? Math.max(maxUpStep, 1.0F) : maxUpStep;
```
So every player-driven mount steps at least 1.0 (camel 1.5).

### Jump power helpers — `LE:2358-2369`
```java
protected float getJumpPower(final float multiplier) {
   return (float)this.getAttributeValue(Attributes.JUMP_STRENGTH) * multiplier * this.getBlockJumpFactor() + this.getJumpBoostPower();
}
public float getJumpBoostPower() {
   return this.hasEffect(MobEffects.JUMP_BOOST) ? 0.1F * (this.getEffect(MobEffects.JUMP_BOOST).getAmplifier() + 1.0F) : 0.0F;
}
```
`getBlockJumpFactor` `E:1112-1116` (here, else below); `getBlockSpeedFactor` `LE:514-516` = `Mth.lerp(MOVEMENT_EFFICIENCY, Entity.getBlockSpeedFactor(), 1.0F)`, `E:1118-1126`.
The jump boost is the **mount's** effect.

### setRot — `E:469-472`, `E:4022-4027`
`setYRot(yRot % 360.0F)`, `setXRot(xRot % 360.0F)` which clamps to [-90, 90].

### Seat position — `E:2447-2471`, `LE:4032-4034`, `EntityAttachments:68-80`
```java
// Entity.positionRider
Vec3 position = this.getPassengerRidingPosition(passenger);
Vec3 offset = passenger.getVehicleAttachmentPoint(this);
moveFunction.accept(passenger, position.x - offset.x, position.y - offset.y, position.z - offset.z);
// LivingEntity.getPassengerRidingPosition
return this.position().add(this.getPassengerAttachmentPoint(passenger, this.getDimensions(this.getPose()), this.getScale() * this.getAgeScale()));
// default getPassengerAttachmentPoint → attachments.getClamped(PASSENGER, passengerIndex, vehicle.yRot) = point.yRot(-yRot * PI/180)
```
Player vehicle attachment: `Avatar.DEFAULT_VEHICLE_ATTACHMENT = (0, 0.6, 0)` (`world/entity/Avatar.java:17,23`). So rider feet = mount pos + seat − (0, 0.6·playerScale, 0).
`PASSENGER` fallback when a type declares none is `(0, height, 0)` (`EntityAttachment:7,25`).

## 1. Horse family (horse, donkey, mule, skeleton_horse, zombie_horse) — `AbstractHorse`

### Control condition
`AH:961-964`: `isSaddled() && getFirstPassenger() instanceof Player`.
`Mob:847-857` `isSaddled = hasItemInSlot(SADDLE) && isEquippableInSlot(item, SADDLE)`; saddle's equippable allows `CAN_EQUIP_SADDLE`
(`item/equipment/Equippable.java:106`; tag `EntityTypeTagsProvider.java:209-222`: horse, skeleton_horse, zombie_horse, donkey, mule, pig,
strider, camel, camel_husk, nautilus, zombie_nautilus — **not llama/trader_llama**). `AH:225-227` `canUseSlot(SADDLE)` = alive && !baby && tamed;
`SkeletonHorse:179`, `ZombieHorse:171`, `Horse:228` override `canUseSlot` → `true`. Client learns the saddle via SetEquipment (SADDLE slot), tame via `DATA_ID_FLAGS` bit 2.

### getRiddenInput — `AH:752-765`
```java
if (this.onGround() && this.playerJumpPendingScale == 0.0F && this.isStanding() && !this.allowStandSliding) return Vec3.ZERO;
float sideways = controller.xxa * 0.5F;
float forward = controller.zza;
if (forward <= 0.0F) forward *= 0.25F;
return new Vec3(sideways, 0.0, forward);
```
`controller.xxa/zza` come from `LP:705-718` `applyInput`: `modifyInput(input.getMoveVector())` = `scale(0.98F)`, sneaking factor only when
`isMovingSlowly()` (crouching is forced false while a passenger, `LP:795-799`), item-use slowdown skipped for passengers (`LP:726`), then
`modifyInputSpeedForSquareMovement` (`LP:738-755`).

### tickRidden — `AH:721-740`
```java
Vec2 rotation = this.getRiddenRotation(controller);          // AH:742-744: new Vec2(controller.getXRot() * 0.5F, controller.getYRot())
this.setRot(rotation.y, rotation.x);
this.yRotO = this.yBodyRot = this.yHeadRot = this.getYRot();
if (this.isLocalInstanceAuthoritative()) {
   if (riddenInput.z <= 0.0) this.gallopSoundCounter = 0;
   if (this.onGround()) {
      if (this.playerJumpPendingScale > 0.0F && !this.isJumping()) this.executeRidersJump(this.playerJumpPendingScale, riddenInput);
      this.playerJumpPendingScale = 0.0F;
   }
}
```
Mount yaw snaps to the rider's yaw every tick; pitch = rider pitch × 0.5. Note `yRotO` is overwritten too (no yaw interpolation for the mount).
`isJumping()` (`LE:3175`) is the mount's own `jumping`, always false on the client. A pending jump is discarded if the horse is airborne on the tick it is consumed.

### executeRidersJump — `AH:772-782`
```java
double impulse = this.getJumpPower(amount);                 // JUMP_STRENGTH * amount * blockJumpFactor + jumpBoost
Vec3 movement = this.getDeltaMovement();
this.setDeltaMovement(movement.x, impulse, movement.z);     // sets y outright (no max with current y)
this.needsSync = true;
if (input.z > 0.0) {
   float sin = Mth.sin(this.getYRot() * (float) (Math.PI / 180.0));
   float cos = Mth.cos(this.getYRot() * (float) (Math.PI / 180.0));
   this.setDeltaMovement(this.getDeltaMovement().add(-0.4F * sin * amount, 0.0, 0.4F * cos * amount));
}
```
Runs inside `tickRidden`, i.e. before `travel` the same tick (so gravity/drag apply to it this tick).

### Jump charge → pending scale
- `PlayerRideableJumping:16-18`: `jumpAmount >= 90 ? 1.0F : 0.4F + 0.4F * jumpAmount / 90.0F`.
- `AH:878-890` `onPlayerJump(jumpAmount)`: only if saddled; `jumpAmount < 0 → 0` else `allowStandSliding = true; standIfPossible();`; then
  `playerJumpPendingScale = getPlayerJumpPendingScale(jumpAmount)`. A tap (amount 0) still gives scale 0.4.
- `AH:893-895` `canJump()` = `isSaddled()`.
- `AH:697-701` `standIfPossible`: `canPerformRearing() && (isEffectiveAi() || !isClientSide)` → `setStanding(20)` (client-side too, since the
  ridden horse is effective AI). `Llama:260` `canPerformRearing=false`.

### Standing / rearing
- `AH:682-691` `setStanding(ticks)`: eating=false, flag 32 set, `standCounter = ticks`; `clearStanding` clears both.
- `AH:578-623` `tick()`: `super.tick()` (which runs aiStep→travelRidden) **first**, then `if (standCounter > 0 && --standCounter <= 0) clearStanding();`,
  and `if (!isStanding()) allowStandSliding = false;` Stand anim: `standAnim += (1 - standAnim) * 0.4F + 0.05F` (cap 1) while standing, else
  `standAnim += (0.8F*s^3 - s) * 0.6F - 0.05F` (floor 0); `standAnimO` = previous.
- Server also rears on `START_RIDING_JUMP` (`AH:897-902` `handleStartJump`: `allowStandSliding = true; standIfPossible(); playJumpSound()`), only if data > 0
  (`server/network/ServerGamePacketListenerImpl.java:1790-1796`), and syncs the flag back. A server-initiated rear (hurt / `makeMad`) with
  `allowStandSliding=false` freezes ridden input on the ground (`getRiddenInput` → ZERO).
- `AH:505-508` `isImmobile` includes standing/eating — affects only the mount's own xxa/zza.

### getRiddenSpeed — `AH:767-770`
`(float)this.getAttributeValue(Attributes.MOVEMENT_SPEED)`. Horses cannot sprint (`E:4043` `canSprint=false`).

### Travel
Standard `LivingEntity.travel`. `onClimbable()` overridden to false (`AH:951-953`). Skeleton horse `getWaterSlowDown = 0.96F` (`SkeletonHorse:149-151`)
and is not in `CAN_FLOAT_WHILE_RIDDEN`; the other four float (+0.04 y).

### Attributes (defaults; server randomises → client needs UpdateAttributes)
`AH:378-386` base: `JUMP_STRENGTH 0.7, MAX_HEALTH 53, MOVEMENT_SPEED 0.225F, STEP_HEIGHT 1.0, SAFE_FALL_DISTANCE 6, FALL_DAMAGE_MULTIPLIER 0.5`
(on top of `LE:333-361` living defaults: gravity 0.08, scale 1, friction/air-drag modifiers 1, movement/water efficiency 0).
- horse: base (`DefaultAttributes:133`); randomised speed `(0.45F + 0.3r1 + 0.3r2 + 0.3r3) * 0.25`, jump `0.4F + 0.2r1 + 0.2r2 + 0.2r3` (`AH:942-948`, `Horse:53-57`).
- donkey/mule: `createBaseChestedHorseAttributes` = base + `MOVEMENT_SPEED 0.175F, JUMP_STRENGTH 0.5` (`AbstractChestedHorse:55-57`), not randomised except health.
- skeleton_horse: base + `MAX_HEALTH 15, MOVEMENT_SPEED 0.2F` (`SkeletonHorse:47-49`); jump randomised like horse (`:60-62`).
- zombie_horse: base + `MAX_HEALTH 25` (`ZombieHorse:53-55`); jump `0.5 + 3·r/15`, speed `(9 + r1 + r2 + r3) / 42.16F` (`ZombieHorse:74-89`).
Values with `Attribute.setSyncable(true)` (`ai/attributes/Attributes.java`: movement_speed `:77`, jump_strength `:58`, step_height `:98`, gravity `:55`,
scale `:89`, flying_speed `:48`, friction_modifier `:52`, air_drag_modifier `:10`, movement_efficiency `:74`, water_movement_efficiency `:108`) arrive
via `ClientboundUpdateAttributesPacket` (`CPL:2358`).

### Seat — `AH:1038-1042`, `EntityTypes`
```java
return super.getPassengerAttachmentPoint(passenger, dimensions, scale)
   .add(new Vec3(0.0, 0.15 * this.standAnimO * scale, -0.7 * this.standAnimO * scale).yRot(-this.getYRot() * (float) (Math.PI / 180.0)));
```
Base seat y: horse 1.44375 (`EntityTypes:548`), donkey 1.1125 (`:348`), mule 1.2125 (`:694`), skeleton/zombie horse 1.31875 (`:888`, `:1141`).
`AH:930-936` `positionRider` also sets `passenger.yBodyRot = this.yBodyRot`. `AH:747-750` `addPassenger` → `passenger.absSnapRotationTo(getViewYRot(0), getViewXRot(0))`
(mount snaps the rider's view to its own on mount).

## 2. Llama / trader llama — not drivable
Not in `CAN_EQUIP_SADDLE`, so `isSaddled()` is always false and `AH:963` falls through to `Mob.getControllingPassenger` (mob only). A player can sit on
a tamed llama but the client never simulates it (server-moved, interpolated), `jumpableVehicle()` is null. Seat `(0, 1.37, -0.3)` (`EntityTypes:634`,
`Llama:455-457` default attachment, no stand offset). Ravager (`monster/Ravager.java:90`) likewise only mob-controlled.

## 3. Camel and camel_husk — `animal/camel/Camel.java` (extends AbstractHorse)

### Control
Inherits `AH:962-964`. `Camel.isTamed()` always true (`Camel:650-652`), so saddle is enough. `CamelHusk` adds nothing movement-related (`CamelHusk.java`; only sounds,
`isMobControlled`, `chargeSpeedModifier`). Both register `Camel.createAttributes()` (`DefaultAttributes:106-107`).

### Sitting gate
- `Camel:267-269` `refuseToMove() = isCamelSitting() || isInPoseTransition()`.
- `Camel:569-580`: `isCamelSitting = LAST_POSE_CHANGE_TICK < 0`; `getPoseTime = level.getGameTime() - |LAST_POSE_CHANGE_TICK|` (`:627-629`);
  `isInPoseTransition = poseTime < (sitting ? 40 : 52)`. Uses the **client** game time.
- `getRiddenInput` `Camel:282-285`: `refuseToMove() ? Vec3.ZERO : super.getRiddenInput(...)` (horse mapping: sideways 0.5, backwards 0.25).
- `getRiddenRotation` `Camel:277-280`: when refusing, keeps own `(xRot, yRot)`; else horse rule.
- `tickRidden` `Camel:259-265`: `super.tickRidden` (rotation + dash), then `if (controller.zza > 0.0F && isCamelSitting() && !isInPoseTransition()) standUp();`
  `standUp` (`Camel:595-601`) sets `LAST_POSE_CHANGE_TICK = gameTime` locally → 52 ticks of transition before it can move.
- `travel` `Camel:249-257`: `if (refuseToMove() && onGround()) { deltaMovement = (0, y, 0); input = (0, input.y, 0); } super.travel(input);`
- `Camel:206-208` sitting camel in water stands instantly.

### getRiddenSpeed — `Camel:271-275`
```java
float movementBonus = controller.isSprinting() && this.getJumpCooldown() == 0 ? 0.1F : 0.0F;
return (float)this.getAttributeValue(Attributes.MOVEMENT_SPEED) + movementBonus;
```
`Camel:299-302` `canSprint() = true` — the only mount the rider can sprint on (`LP:1164-1181` `vehicleCanSprint = vehicle.canSprint() && vehicle.isLocalInstanceAuthoritative()`).

### Dash (the camel's "jump")
- `Camel:287-290` `canJump = !refuseToMove() && super.canJump()` (saddled).
- `Camel:292-297` `onPlayerJump`: only `if (isSaddled() && dashCooldown <= 0 && onGround())` → horse `onPlayerJump` (sets pending scale, `allowStandSliding`,
  `standIfPossible` — rearing does nothing visible for camels since `canPerformRearing` is inherited true; standing then only zeroes input when
  `!allowStandSliding`).
- `Camel:304-317` `executeRidersJump` (called from horse `tickRidden` when on ground):
  ```java
  double jumpMomentum = this.getJumpPower();               // JUMP_STRENGTH * 1 * blockJumpFactor + jumpBoost
  this.addDeltaMovement(
     this.getLookAngle().multiply(1.0, 0.0, 1.0).normalize()
        .scale(22.2222F * amount * this.getAttributeValue(Attributes.MOVEMENT_SPEED) * this.getBlockSpeedFactor())
        .add(0.0, 1.4285F * amount * jumpMomentum, 0.0));
  this.dashCooldown = 55;
  this.setDashing(true);
  ```
  Additive (unlike horse). `getLookAngle` uses the camel's rotation already set this tick (`yRot` = rider yaw, pitch irrelevant after flattening).
- Cooldown `Camel:185-199` in `tick()` after `super.tick()`: `if (isDashing() && dashCooldown < 50 && (onGround() || isInLiquid() || isPassenger())) setDashing(false);`
  then `dashCooldown--`. `Camel:640-645` a synced DASH change sets `dashCooldown = 55` if it was 0. `getJumpCooldown()` = `dashCooldown` (`:346-349`).
- `handleStartJump` (server) `Camel:327-332`: sound + `setDashing(true)`.

### Attributes — `Camel:120-126`
base horse + `MAX_HEALTH 32, MOVEMENT_SPEED 0.09F, JUMP_STRENGTH 0.42F, STEP_HEIGHT 1.5`.

### Seat — `Camel:492-560`
```java
int index = Math.max(this.getPassengers().indexOf(passenger), 0);
boolean driver = index == 0;
float offset = 0.5F;
float height = (float)(this.isRemoved() ? 0.01F : this.getBodyAnchorAnimationYOffset(driver, 0.0F, dimensions, scale));
if (this.getPassengers().size() > 1) { if (!driver) offset = -0.7F; if (passenger instanceof Animal) offset += 0.2F; }
return new Vec3(0.0, height, offset * scale).yRot(-this.getYRot() * (float) (Math.PI / 180.0));
```
`getBodyAnchorAnimationYOffset` (`:516-549`): `base = dimensions.height() - (baby ? 0.09375 : 0.375)`; `sittingHeightDifference = scale*1.43F`,
`verticalDrop = shd - scale*0.2F`, `bottomPoint = shd - verticalDrop`; during transition, piecewise lerp with halfPoint 28 (sit) / 24 front, 32 back (stand),
flexPointOffset 0.5/0.1 (sit) 0.6/0.35 (stand), over 40 (sit) / 52 (stand) ticks; fully sitting adds `bottomPoint`. Dimensions: standing 1.7×2.375
(`EntityTypes:255-260`), adult sitting height `2.375 - 1.43` (`Camel:87-90`). `getAgeScale` baby 0.6 (`:512-514`). `canAddPassenger: getPassengers().size() <= 2` (`:565-567`) → up to 3 riders; driver is index 0.

## 4. Pig — `animal/pig/Pig.java`

- Control `Pig:96-101`: `isSaddled() && getFirstPassenger() instanceof Player player && player.isHolding(Items.CARROT_ON_A_STICK)` (either hand, `LE:2234-2240`).
  `canUseSlot(SADDLE)` = alive && !baby (`Pig:185-187`). Switching away from the stick instantly drops authority (client stops simulating/sending MoveVehicle).
- `getRiddenInput` `Pig:222-225`: `new Vec3(0.0, 0.0, 1.0)` — always full forward, player WASD ignored.
- `tickRidden` `Pig:214-220`:
  ```java
  this.setRot(controller.getYRot(), controller.getXRot() * 0.5F);
  this.yRotO = this.yBodyRot = this.yHeadRot = this.getYRot();
  this.steering.tickBoost();
  ```
- `getRiddenSpeed` `Pig:227-230`: `(float)(MOVEMENT_SPEED * 0.225 * this.steering.boostFactor())` (double math, then cast).
- `ItemBasedSteering` (`world/entity/ItemBasedSteering.java`):
  ```java
  public void onSynced() { this.boosting = true; this.boostTime = 0; }
  public void tickBoost() { if (this.boosting && this.boostTime++ > this.boostTimeTotal()) this.boosting = false; }
  public float boostFactor() {
     return this.boosting ? 1.0F + 1.15F * Mth.sin((float)this.boostTime / this.boostTimeTotal() * (float) Math.PI) : 1.0F;
  }
  // boost(random) (server): entityData.set(boostTime, random.nextInt(841) + 140)
  ```
  Client gets boosts only through `DATA_BOOST_TIME` sync: `Pig:103-110` `onSyncedDataUpdated` → `steering.onSynced()` (client side only).
  `FoodOnAStickItem.use` returns PASS on client (`world/item/FoodOnAStickItem.java:24-27`); the boost is decided on the server after the
  UseItem packet. Order per tick: `tickBoost` (post-increment compare) in `tickRidden`, then `boostFactor()` in `getRiddenSpeed`.
- Travel: standard. Attributes `Pig:92-94`: `MAX_HEALTH 10, MOVEMENT_SPEED 0.25`; step 0.6 → 1.0 when ridden. Gravity 0.08.
- Seat: adult `(0, 0.86875, 0)` (`EntityTypes:774`), baby `(0, 0.5, 0)` (`Pig:72`).

## 5. Strider — `monster/Strider.java`

- Control `Strider:206-211`: saddled && first passenger player holding `WARPED_FUNGUS_ON_A_STICK`. `canUseSlot(SADDLE)` = alive && !baby (`:135-137`).
- `getRiddenInput` `:264-267`: `new Vec3(0.0, 0.0, 1.0)`.
- `tickRidden` `:256-262`: `setRot(controller.getYRot(), controller.getXRot() * 0.5F); yRotO = yBodyRot = yHeadRot = getYRot(); steering.tickBoost();`
- `getRiddenSpeed` `:269-272`:
  `(float)(MOVEMENT_SPEED * (this.isSuffocating() ? 0.35F : 0.55F) * this.steering.boostFactor())`.
- Suffocation is recomputed on the **client** every tick in `tick()` (`:298-318`), before `super.tick()`:
  ```java
  boolean inWarmBlocks = stateInside.is(BlockTags.STRIDER_WARM_BLOCKS) || stateOn.is(BlockTags.STRIDER_WARM_BLOCKS) || this.getFluidHeight(FluidTags.LAVA) > 0.0;
  boolean onWarmStrider = this.getVehicle() instanceof Strider strider && !strider.isSuffocating();
  this.setSuffocating(!inWarmBlocks && !onWarmStrider);
  ```
  `setSuffocating` (`:163-173`) adds/removes transient `SUFFOCATING_MODIFIER` = `-0.34F ADD_MULTIPLIED_BASE` on MOVEMENT_SPEED (`:79-82`). So out of
  warm blocks: `speed = base*0.66 * 0.35`, in lava/warm: `base * 0.55`. `stateOn` is `getBlockStateOnLegacy()`. Gated by `!isNoAi()`.
- Lava walking: `canStandOnFluid(fluid) = fluid.is(FluidTags.LAVA)` (`:179-182`) → `shouldTravelInFluid` false in lava → `travelInAir`.
  Collision: `LiquidBlock.getCollisionShape` (`world/level/block/LiquidBlock.java:62-75`) gives source lava (LEVEL 0 only) the shape
  `getLiquidCollisionShape() = Block.column(16.0, 0.0, 8.0)` (`Strider:341-344`, i.e. y 0..0.5) when `context.isAbove(shape, pos, true)`
  (`entityBottom > pos.y + 0.5 - 1.0E-5F`, `phys/shapes/EntityCollisionContext.java:77-80`) and the block above is not lava
  (`EntityCollisionContext:61-65`). Flowing lava gives no collision.
- `floatStrider()` after `super.tick()` (`:328-339`):
  ```java
  if (this.isInLava()) {
     if (context.isAbove(this.getLiquidCollisionShape(), this.blockPosition(), true) && !level.getFluidState(blockPosition().above()).is(FluidTags.LAVA))
        this.setOnGround(true);
     else
        this.setDeltaMovement(this.getDeltaMovement().scale(0.5).add(0.0, 0.05, 0.0));
  }
  ```
  Runs after travel, so it affects next tick's velocity and onGround (block friction for lava = default 0.6).
- In water: normal `travelInFluid` (strider is `isSensitiveToWater`, `:371`, damage only).
- Attributes `:346-348`: `MOVEMENT_SPEED 0.175F` (+animal defaults); step 1.0 when ridden. Fire immune.
- Seat: no declared passenger attachment → fallback `(0, height=1.7, 0)` (`EntityTypes:976-978`); baby `(0, 0.65625, 0)` (`:89`). Client adds a bob (`:189-199`):
  ```java
  float animSpeed = Math.min(0.25F, this.walkAnimation.speed());
  float animPos = this.walkAnimation.position();
  float offset = 0.12F * Mth.cos(animPos * 1.5F) * 2.0F * animSpeed;
  return super.getPassengerAttachmentPoint(passenger, dimensions, scale).add(0.0, offset * scale, 0.0);
  ```
  (server uses no bob — the rider's position differs client vs server.)
- Boost: same `ItemBasedSteering` as pig (`:90`, `:119-123`).

## 6. Happy ghast — `animal/happyghast/HappyGhast.java`

- Control `:342-347`: `isWearingBodyArmor() && !isOnStillTimeout() && getFirstPassenger() instanceof Player`. Body slot = harness
  (`CAN_EQUIP_HARNESS` = happy_ghast); `canUseSlot(BODY)` = alive && !baby (`:270-273`). `isOnStillTimeout() = staysStill() || serverStillTimeout > 0`
  (`:543-545`); on the client only the synced `STAYS_STILL` matters (`:485-491`). Server sets still-timeout 10 when a non-ghast-riding player stands
  on top (`:416-433`, `scanPlayerAboveGhast` `:547-568`) → control (and client authority) is lost while it is set.
- `getRiddenInput` `:349-371`:
  ```java
  float strafe = controller.xxa;
  float forward = 0.0F; float up = 0.0F;
  if (controller.zza != 0.0F) {
     float forwardLook = Mth.cos(controller.getXRot() * (float) (Math.PI / 180.0));
     float upLook = -Mth.sin(controller.getXRot() * (float) (Math.PI / 180.0));
     if (controller.zza < 0.0F) { forwardLook *= -0.5F; upLook *= -0.5F; }
     up = upLook; forward = forwardLook;
  }
  if (controller.isJumping()) up += 0.5F;
  return new Vec3(strafe, up, forward).scale(3.9F * this.getAttributeValue(Attributes.FLYING_SPEED));
  ```
  Note: forward magnitude is not proportional to `zza` (only its sign). No descend key: sneaking dismounts (server). `controller.isJumping()` =
  player's `jumping` from `applyInput` (`LP:710`).
- `tickRidden` `:377-387`:
  ```java
  Vec2 rotation = this.getRiddenRotation(controller);         // (controller.getXRot() * 0.5F, controller.getYRot())
  float yRot = this.getYRot();
  float diff = Mth.wrapDegrees(rotation.y - yRot);
  yRot += diff * 0.08F;
  this.setRot(yRot, rotation.x);
  this.yRotO = this.yBodyRot = this.yHeadRot = yRot;
  ```
  Yaw eases at 8 %/tick; pitch snaps to half the rider's.
- `getRiddenSpeed`: not overridden (= `getSpeed()`), and unused because `travel` ignores speed.
- `travel` `:175-179`:
  ```java
  float speed = (float)this.getAttributeValue(Attributes.FLYING_SPEED) * 5.0F / 3.0F;
  this.travelFlying(input, speed, speed, speed);
  ```
  `LE:2434-2448` `travelFlying`: water → `moveRelative(waterSpeed)`, move, `scale(0.8F)`; lava → move, `scale(0.5)`; air → move, `scale(0.91F)`.
  **No gravity, no block friction.** `onClimbable` false (`:170-173`); no fall damage.
- Attributes `:151-159`: `MAX_HEALTH 20, TEMPT_RANGE 16, FLYING_SPEED 0.05, MOVEMENT_SPEED 0.05, FOLLOW_RANGE 16, CAMERA_DISTANCE 8`. Scale capped at 1
  (`sanitizeScale`, `:161-164`). Step 0.6 → 1.0 when ridden (moves with `move(SELF)`, so step-up applies).
  Effective: input scale `3.9*0.05 = 0.195`, accel `0.05*5/3 ≈ 0.0833`, terminal ≈ `0.195*0.0833/0.09` per axis.
- Seat: four points `(0,4,1.7)`, `(-1.7,4,0)`, `(0,4,-1.7)`, `(1.7,4,0)` by passenger index (`EntityTypes:499-507`); controller is index 0 → front.
  `ridingOffset(0.5F)` is the ghast's own vehicle attachment (irrelevant here). `isFlyingVehicle() = !isBaby()` (`:588-591`) exempts it from the server's
  vehicle-floating kick (`ServerGamePacketListenerImpl:523`).
- `canBeCollidedWith` (`:575-586`): on the client, players at/above `maxY` collide with an adult ghast (you can stand on it).

## 7. Nautilus / zombie nautilus — `animal/nautilus/AbstractNautilus.java`

- Control `:169-172`: `isSaddled() && getFirstPassenger() instanceof Player`. `canUseSlot(SADDLE/BODY)` = alive && !baby && tame (`:155-157`).
  Single seat (`canAddPassenger = !isVehicle()`, `:165-167`).
- `getRiddenInput` `:174-192`: same look-pitch mapping as the happy ghast (`strafe = xxa`; if `zza != 0`: `forward = cos(xRot)`, `up = -sin(xRot)`, both ×−0.5 when
  `zza < 0`), **no jump term, no scaling**: `return new Vec3(strafe, up, forward);`
- `tickRidden` `:198-215`: rotation `(controller.getXRot()*0.5F, controller.getYRot())`; `yRot += Mth.wrapDegrees(target - yRot) * 0.5F`; `setRot(yRot, pitch)`;
  `yRotO = yBodyRot = yHeadRot = yRot`; then (authoritative) `if (playerJumpPendingScale > 0 && !isJumping()) executeRidersJump(scale, controller); playerJumpPendingScale = 0;`
  — **no onGround requirement** (unlike horse).
- `getRiddenSpeed` `:225-230`: `isInWater() ? 0.0325F * (float)MOVEMENT_SPEED : 0.02F * (float)MOVEMENT_SPEED`.
- `travelInWater` override `:217-223` (reached via `LE:2484` when `shouldTravelInFluid`):
  ```java
  float speed = this.getSpeed();
  this.moveRelative(speed, input);
  this.move(MoverType.SELF, this.getDeltaMovement());
  this.setDeltaMovement(this.getDeltaMovement().scale(0.9));
  ```
  No gravity, no `jumpOutOfFluid`, no fluid-falling adjust. Not in `CAN_FLOAT_WHILE_RIDDEN`. `isPushedByFluid() = false` (`:119-121`).
  Out of water: normal `travelInAir` with gravity 0.08 and `getSpeed()` = `0.02*MS` (air: ×0.1). In lava: vanilla `travelInLava`.
- Dash: `canJump = isSaddled()` (`:317-320`); `onPlayerJump` `:322-327`: `if (isSaddled() && dashCooldown <= 0) playerJumpPendingScale = getPlayerJumpPendingScale(jumpAmount)`.
  `executeRidersJump` `:343-351`:
  ```java
  this.addDeltaMovement(controller.getLookAngle()
     .scale((this.isInWater() ? 1.2F : 0.5F) * amount * this.getAttributeValue(Attributes.MOVEMENT_SPEED) * this.getBlockSpeedFactor()));
  this.dashCooldown = 40;
  this.setDashing(true);
  ```
  Uses the **rider's** full look vector (pitch included). Cooldown in `tick()` `:298-315`: `if (isDashing() && dashCooldown < 35) setDashing(false); if (dashCooldown > 0) dashCooldown--;`
  synced DASH → `dashCooldown = 40` if 0 (`:365-371`).
- Attributes `:111-117`: `MAX_HEALTH 15, MOVEMENT_SPEED 1.0, ATTACK_DAMAGE 3, KNOCKBACK_RESISTANCE 0.3F`; zombie nautilus `MOVEMENT_SPEED 1.1F`
  (`ZombieNautilus:51-53`). Step 0.6 → 1.0 when ridden.
- Seat `(0, 1.1375, 0)` (`EntityTypes:696-703`, `:1144-1151`).

## 8. LocalPlayer riding logic — `LP`

### Jump bar (`LP:900-926`, inside `aiStep`, after `input.tick()`)
`wasJumping = input.keyPresses.jump()` is captured **before** `input.tick()` (`LP:792`).
```java
PlayerRideableJumping jumpableVehicle = this.jumpableVehicle();   // LP:613-617: controlled vehicle that is PlayerRideableJumping && canJump()
if (jumpableVehicle != null && jumpableVehicle.getJumpCooldown() == 0) {
   if (this.jumpRidingTicks < 0) { this.jumpRidingTicks++; if (this.jumpRidingTicks == 0) this.jumpRidingScale = 0.0F; }
   if (wasJumping && !this.input.keyPresses.jump()) {
      this.jumpRidingTicks = -10;
      jumpableVehicle.onPlayerJump(Mth.floor(this.getJumpRidingScale() * 100.0F));
      this.sendRidingJump();
   } else if (!wasJumping && this.input.keyPresses.jump()) {
      this.jumpRidingTicks = 0; this.jumpRidingScale = 0.0F;
   } else if (wasJumping) {
      this.jumpRidingTicks++;
      if (this.jumpRidingTicks < 10) this.jumpRidingScale = this.jumpRidingTicks * 0.1F;
      else this.jumpRidingScale = 0.8F + 2.0F / (this.jumpRidingTicks - 9) * 0.1F;
   }
} else {
   this.jumpRidingScale = 0.0F;
}
```
Charge: ticks 1..9 → 0.1..0.9, tick 10 → 1.0, then decays toward 0.8 (0.9, 0.867, 0.85 …). Release sends
`ServerboundPlayerCommandPacket(START_RIDING_JUMP, Mth.floor(jumpRidingScale * 100.0F))` (`LP:407-410`) with the same integer passed to
`onPlayerJump`. The mount consumes the pending scale in **its next tick** (it ticks before the player). Jump-key state does not otherwise affect horses;
the player's own `jumping` is used only by the happy ghast.
Applies to horse family, camel (when `canJump`, cooldown 0), nautilus. Pig/strider/ghast are not `PlayerRideableJumping`.
Gotcha: creative flight double-tap still toggles while on a jumpable vehicle (`LP:855`: `getVehicle() == null || jumpableVehicle() != null`).

### Sprint
`LP:1162-1181`: while a passenger, `isSprintingPossible` requires `vehicleCanSprint(vehicle) = vehicle.canSprint() && vehicle.isLocalInstanceAuthoritative()` → camel only.
`shouldStopRunSprinting` (`LP:935-939`) uses the player's own `horizontalCollision`. The player's sprint flag feeds the camel's +0.1 speed.

### rideTick / handsBusy
`LP:999-1008`: `super.rideTick(); handsBusy = false;` only boats set input/handsBusy. Nothing mount-specific. `removeVehicle` clears `handsBusy` (`LP:219-223`).
There is no `isRidingJumpable` in 26.3 — replaced by `jumpableVehicle()`. `startRiding` only plays sounds (minecart, happy ghast, nautilus) (`LP:195-217`).

### Dismount
Client does not dismount itself: `Player.rideTick` stops riding only server-side when `wantsToStopRiding() = isShiftKeyDown()` (`Player:298-300`, `:435-441`).
The server learns shift via `ServerboundPlayerInputPacket` (`ServerGamePacketListenerImpl:430-437`).

## 9. Packets sent while riding

### `LocalPlayer.sendChanges` (`LP:263-280`), once per tick after all entities ticked
```java
if (!this.lastSentInput.equals(this.input.keyPresses)) {
   this.connection.send(new ServerboundPlayerInputPacket(this.input.keyPresses));
   this.lastSentInput = this.input.keyPresses;
}
if (this.isPassenger()) {
   this.connection.send(new ServerboundMovePlayerPacket.Rot(this.getYRot(), this.getXRot(), this.onGround(), this.horizontalCollision));
   Entity vehicle = this.getRootVehicle();
   if (vehicle != this && vehicle.isLocalInstanceAuthoritative()) {
      this.connection.send(ServerboundMoveVehiclePacket.fromEntity(vehicle));
      this.sendIsSprintingIfNeeded();
   }
} else {
   this.sendPosition();
}
```
Per tick order: **[START_RIDING_JUMP if released this tick — sent earlier, during tickEntities]** → PlayerInput (on change) →
MovePlayer.Rot (**every tick**, unconditionally) → MoveVehicle (every tick when authoritative) → PlayerCommand START/STOP_SPRINTING (on change, only when authoritative).
`sendPosition()` (and its position reminder) is not used while riding.

### MoveVehicle contents — `network/protocol/game/ServerboundMoveVehiclePacket.java:11-22`
`(PositionAndRotation movingTo, boolean onGround)`, codec `Vec3 (3 doubles), float yRot, float xRot, bool onGround` (`core/PositionAndRotation.java:10-18`).
`fromEntity` uses `entity.getClientPositionAndRotation()` = interpolation target if interpolating, else current pos/yRot/xRot (`E:4034-4036`), and `entity.onGround()`.
No velocity is sent.

### PlayerCommand actions — `ServerboundPlayerCommandPacket.java:60-68`
`STOP_SLEEPING, START_SPRINTING, STOP_SPRINTING, START_RIDING_JUMP, STOP_RIDING_JUMP, OPEN_INVENTORY, START_FALL_FLYING`.
`STOP_RIDING_JUMP` is never sent by the 26.3 client (only referenced by the server handler). `OPEN_INVENTORY` via `sendOpenInventory()` (`LP:412-414`) when the
inventory key is pressed on a `HasCustomInventoryScreen` vehicle. Server ignores `START_RIDING_JUMP` data ≤ 0 (no `handleStartJump`, no server rear/dash sound),
but the client already applied the jump.

### Server → client while authoritative
- `CPL:2158-2174` `handleMoveVehicle`: if root vehicle is local-authoritative and the target differs from `getClientPosition()` by > `1.0E-5F`, cancel interpolation and
  `absSnapTo(pos, yRot, xRot)`; **always** echo `ServerboundMoveVehiclePacket.fromEntity(vehicle)` immediately.
- `CPL:641-666` `handleEntityPositionSync` and `CPL:721-745` `handleMoveEntity`: for authoritative entities only update the position codec base; no move.
- `CPL:669-690` `handleTeleportEntity`: applies (interpolating if ticking); if not interpolated and the player rides it, re-seat the player and, when authoritative,
  send MoveVehicle.
- `CPL:624-630` `handleSetEntityMotion` → `lerpMotion` = `setDeltaMovement` (`E:2725-2727`) even for an authoritative mount (knockback etc.).
- `CPL:1140-1170` `handleSetEntityPassengersPacket`: `startRiding(vehicle, true, false)`; yaw snap only for boats (horses snap the rider via `addPassenger`, `AH:747-750`).

### What the client needs from the server per mount
Entity data: horse flags (tame 2, eating 16, standing 32), age/baby, equipment (SADDLE, BODY, hands of the rider for pig/strider), `Camel.DASH`,
`Camel.LAST_POSE_CHANGE_TICK`, pig/strider `DATA_BOOST_TIME`, strider `DATA_SUFFOCATING` (recomputed locally anyway), nautilus `DASH`, happy ghast `STAYS_STILL`,
`NoAI` (strider suffocation gate, Mob control). Attributes (UpdateAttributes): movement_speed (randomised horses; strider modifier), jump_strength (randomised),
step_height, gravity, scale, flying_speed (ghast), movement_efficiency, water_movement_efficiency, friction_modifier, air_drag_modifier. Effects: jump_boost,
slow_falling, levitation, dolphins_grace on the mount.

## 10. Minecarts (and other non-controlled vehicles)

- `AbstractMinecart` (and all of `world/entity/vehicle/minecart/`) does not override `getControllingPassenger()` → null → `isLocalInstanceAuthoritative()` false on the
  client. The client never simulates a ridden minecart; it interpolates from server packets (`ClientboundMoveMinecartPacket` lerp steps for the new behaviour, `CPL:751-758`;
  ordinary move/position-sync otherwise).
- The passenger client sends: `ServerboundPlayerInputPacket` on input change, `ServerboundMovePlayerPacket.Rot(yRot, xRot, onGround, horizontalCollision)` every tick,
  **no** MoveVehicle, **no** sprint commands (the sprint branch is inside the authoritative check). Shift in PlayerInput → server dismount.
- Server pushes the cart from the rider's input: `ServerPlayer:2267-2271`
  ```java
  float leftIntent = lastClientInput.left() == lastClientInput.right() ? 0.0F : (lastClientInput.left() ? 1.0F : -1.0F);
  float forwardIntent = lastClientInput.forward() == lastClientInput.backward() ? 0.0F : (lastClientInput.forward() ? 1.0F : -1.0F);
  return getInputVector(new Vec3(leftIntent, 0.0, forwardIntent), 1.0F, this.getYRot());
  ```
  Old behaviour adds `moveIntent * 0.001` when cart horizontal speed² < 0.01 (`OldMinecartBehavior:153-167`); new behaviour adds
  `normalize(riderMovement.xz) * 0.001` (`NewMinecartBehavior:370-385`). So the yaw in the Rot packet matters.
- Same "no client simulation" applies to a pig/strider whose rider is not holding the stick, an unsaddled horse, a llama, or a happy ghast on still-timeout.

## 11. Ambiguities / things to verify against a live client
- Mount-before-player ordering means ridden input lags player input by one tick; confirm with a capture (first MoveVehicle after pressing W should show no movement).
- The player's own passenger `travel` (discarded by `positionRider`) still sets `onGround`/`horizontalCollision` that go into the Rot packet and the camel sprint-stop check;
  bit-exact Rot packets need that throwaway move ported too.
- Pig/strider boost relies on `DATA_BOOST_TIME` changing: if the server rolls the same total as the previous boost, `SynchedEntityData.set` will not mark it dirty and the client
  never boosts (`SynchedEntityData:108-116` only fires on received items). Also, entity spawn data carrying a non-zero boost time fires `onSynced()` and starts a phantom boost on the client.
- Horse rearing: client `onPlayerJump` calls `setStanding(20)` locally, and the server also rears on `START_RIDING_JUMP`; the synced flag can arrive mid-countdown. Ridden input is only
  zeroed when `allowStandSliding` is false, which after a rider jump happens only once the horse stops standing; server-initiated rears (hurt) do freeze input.
- Camel pose times use the client's game time (`ClientLevel` time, from SetTime packets + local increment); a drifted client clock shifts the 40/52-tick transition gate.
- Strider rider seat bob is client-only, so client and server rider positions differ by up to 0.06·scale in y.
