# Bedrock movement physics: source survey (2026-10-02)

Bottom line: no public project has Mojang's own swim-start decision. Every server, proxy and anticheat checked trusts the client's `START_SWIMMING`/`STOP_SWIMMING` input flags and simulates only what happens after them. That includes BDS itself: in server-auth mode the swim pose comes from client input. The ground truth is the BDS/client binary. LeviLamina headers give the system names to look for in it.

## 1. Open-source prediction code (ranked)

1. **oomph-ac/bedsim** (Go, active). https://github.com/oomph-ac/bedsim
   - `liquid.go`: `simulateLiquidTravel`, `updateSwimTravel`. Pitch-target Y uses `-sin(pitch)`, rate 0.06, or 0.085 when the target is below -0.2. When looking up with air above, it zeroes Y. Jump in water adds +0.04, or sets Y to 0 when SwimAmount is between 0 and 1 or the player is swimming and not touching liquid. Fast drag applies when sprinting or on `StoppedSwimmingThisTick`.
   - Swim flag: it trusts the client, gated by `SwimWaterGraceTicks` (recent water contact seen by the server).
   - Other files: `simulation.go` (pose/crawl/swim state), `sprint_intent.go`, `jump.go`, `block/{friction,bounce,ground,contact}.go` (slime/honey/supporting block), `bubble.go`. Recent PRs #28 (compact poses, boosted swim acceleration) and #27/#29/#30.
2. **Boar**, the Geyser prediction anticheat (Java). The active fork is https://github.com/opencollab-incubator/Boar (pushed 2026-09). The original, https://github.com/Oryxel/Boar, is stale since 2025-05. Paths under `common/src/main/java/ac/boar/anticheat/`:
   - `prediction/ticker/impl/PlayerTicker.java`
     - Swim travel uses the look vector `d`. It sets Y to 0 if there is no fluid at pos+0.4 and `0<d<0.55`. Otherwise it adds `(d-vy)*e`, with `e=0.085` if `d<-0.2`, else 0.06. This is skipped while jumping unless `d<=0` or there is fluid at the feet.
     - A comment notes that BE zeroes Y near the surface, where JE allows swimming up.
     - Sneak in water applies -0.04 to Y. The sneak/crawl input multiplier is 0.3, plus Swift Sneak (not applied for the first 2 ticks).
   - `prediction/ticker/base/EntityTicker.java`: `updateSwimming` clears SWIMMING when not touching water. Water flow push factor is 0.014. Eye-height submersion check (pose eye heights are in PlayerTicker: stand 1.62, sneak 1.27, swim 0.4).
   - `prediction/engine/impl/fluid/WaterPredictionEngine.java`: water drag. "Fast tick end" applies if sprinting or on STOP_SWIMMING. There is a version split at 1.21.80, and depth strider.
   - `prediction/engine/impl/fluid/LavaPredictionEngine.java`, `prediction/ticker/impl/LivingTicker.java` (fluid jump threshold, jumpFromGround).
   - `data/block/impl/{SlimeBlockState,HoneyBlockState}.java`.
   - `packets/input/legacy/LegacyAuthInputPackets.java:156-169`: START_SPRINTING is accepted only when `input.z > 0`. START_SWIMMING is trusted outright.
3. **oomph-ac/oomph**, https://github.com/oomph-ac/oomph. Uses bedsim. Swim metadata is in `anticheat/entity/metadata.go` and `anticheat/game/movement.go`.
4. **Trust the client, no physics:**
   - GeyserMC: `core/.../input/BedrockPlayerAuthInputTranslator.java:105-114`.
   - Dragonfly: `server/session/handler_player_auth_input.go`, `server/player/player.go`.
   - PocketMine-MP: `src/network/mcpe/handler/InGamePacketHandler.php:225`, which calls `toggleSwim`.
   - PowerNukkitX: `.../handler/PlayerAuthInputHandler.java`.

   These are useful only for the flag semantics.

## 2. BDS debug symbols

- **Mojang stopped shipping symbols.**
  - Windows `bedrock_server.pdb` was dropped from 1.21.10.22 preview / 1.21.20 release (mid-2024): https://feedback.minecraft.net/hc/en-us/community/posts/27425253557389
  - The Linux `bedrock_server_symbols.debug` is not public from 1.21.40 on (per PocketMine docs): https://doc.pmmp.io/en/rtfd/developers/internals-docs/updating-minecraft-protocol.html
  - Old Linux debug binaries are archived at https://github.com/Chew/BedrockDedicatedServer (`bedrock_server_symbols.debug`, Git LFS, about 115 MB).
- **LeviLamina** was archived in 2024, then resumed in 2025 after a community/Mojang agreement (https://www.minebbs.com/threads/levilamina-bds.32815/). Terms are not public.
  - The current release is v26.51.6 (BDS 26.51, 2026-09-29).
  - Symbol and RVA data ship as the binary release asset of https://github.com/LiteLDev/bedrock-runtime-data. The names are public through the headers.
- **Endstone** (https://github.com/EndstoneMC/endstone) uses `scripts/dump_symbols.py`. It byte-pattern scans PE/ELF using signatures in `scripts/configs/{windows,linux}.toml` (version 1.26.51), with optional `--pdb`. Output: `src/bedrock/symbols/{linux,windows}.h`.

### LeviLamina headers relevant to movement

These are signatures only: ECS system names and their component read/write sets, no function bodies. Base URL: https://github.com/LiteLDev/LeviLamina/tree/main/

- `src/mc/entity/systems/swim_control_system_impl/SwimControlSystemImpl.h`
  - Reads ActorDataFlag, MovementAbilities, ActorRotation, MoveInput, PlayerInputRequest.
  - Filters on WasInWaterFlag and MobIsJumpingFlag.
  - Writes StateVector.
- `src/mc/entity/systems/{SwimControlSystem,UnderWaterSensingSystem,InWaterSensingSystem,LiquidPhysicsSystem,LiquidPhysicsSystemImpl,ServerPlayerMovementSystem,ServerMoveInputHandlerSystem,InsideHoneyBlockSystem,SlimePreNormalTickSystem,JumpInputSystem,JumpControlSystem,TriggerJumpSystem}.h`
  - UnderWaterSensingSystem adds/removes ActorHeadInWater and ActorHeadWasInWater: the eye test.
- `src/mc/entity/systems/movement/mob/MobMovement{Climb,ClimbOutOfLiquid,Drag,Friction,Gravity,Levitate,Speed}.h`, `movement/collision/swept_movement/SweptMovement.h`
- `src/mc/entity/systems/sneak_trigger_system/{SneakTriggerSystem,PlayerStatusTransitions}.h`, `sneak_movement_system/SneakMovementSystem.h`
- `src/mc/entity/components/{SwimAmountComponent,SwimSpeedMultiplierComponent,WaterMovementComponent,WasInWaterFlagComponent,CanSprintResult,HoneyBlockFlag,IsHorizontalPoseFlagComponent,SneakingComponent,ServerPlayerMovementComponent,MovementCorrection,PredictedMovementComponent}.h`
- `src/mc/deps/vanilla_components/{ActorHeadInWaterFlagComponent,MovementAttributesComponent}.h`
- `src/mc/world/actor/Actor.h`: `isInWater`, `isImmersedInWater`, `isUnderLiquid`, `getLiquidAABB`, `startSwimming`/`stopSwimming`, `getSwimAmount`
- `src/mc/world/actor/Mob.h`: `_updateSprintingState`, `setSprinting`, `jumpFromGround`, `getSwimSpeedMultiplier`
- `src/mc/world/actor/player/Player.h`
- `src/mc/network/packet/PlayerAuthInputPacket.h`
- `src/mc/world/actor/player/PlayerMovementSettings.h`
- Client-side input: `src-client/mc/client/input/{VanillaMoveInputHandler,ClientMoveInputHandler}.h`

## 3. Swim-start rule writeups

No public document gives Bedrock's swim-start pitch or eye-depth rule. What exists:

- Minecraft Wiki (https://minecraft.wiki/w/Swimming) covers only the basics:
  - Sprint swimming is blocked at food 6 or below and under Blindness.
  - BE 1.14.0: "can no longer start swimming in 1-block deep water".
  - BE 1.5.0 improved surface swimming. BE 1.4.0 allows swimming through 1x1 gaps.
- Feedback threads describe the behavior: on BE you can sprint-swim at the surface, with Y snapping to 0 near the surface.
  - https://feedback.minecraft.net/hc/en-us/community/posts/360048331852
  - https://feedback.minecraft.net/hc/en-us/community/posts/360032416312
- The best practical source is the travel logic in Boar and bedsim (section 1). Getting the exact start condition means reversing `SwimControlSystemImpl` / `UnderWaterSensingSystem` / the client `VanillaMoveInputHandler`, or capturing it empirically with vanilla-client recordings.
