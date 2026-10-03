# Handoff to the physics session: elytra gliding and vehicle steering

Owner: the `physics-fuzz` session (acacia-physics / `movement/`). Formulas, packet fields and sources:
`docs/research/riding-fishing-elytra.md` ("Elytra physics", "Steering", "PlayerAuthInput while riding").

**Gliding is done** (feat/glide-physics, 2026-10-03). Item 1 and the gliding notes below are kept only for
context. In the bot it is `Controls::glide`, `movement/glide.rs` and the server boost via MovementEffect;
in physics, `simulate_glide` follows BDS. Vehicles (items 2-3) are still open.

## What exists on master
| Piece | Where | Behaviour now |
|---|---|---|
| `start_gliding` / `stop_gliding` | `acacia-bot/src/elytra.rs` | Physics bots only: jump press + `StartGliding` via `Bot.queued_flags`, waits for the server's GLIDING actor flag; `StopGliding` on request |
| `boost_with_firework` | `elytra.rs` | `Animate "useitem"` + `UseItem` ClickAir with a rocket; no boost is simulated |
| Seated input | `riding/tick.rs` (`seat_idle_input`, `ride_physics_tick`), `riding/input.rs` | Physics bots stop simulating while seated and send the vehicle's server-given position; on dismount the simulation resumes from the seat at the continued tick |
| Extra input flags | `Bot.queued_flags` (`bot.rs`) | Appended to the next `PlayerAuthInput`, idle and physics |

Until the physics lands, a gliding physics bot keeps walking physics (falls), so BDS corrects it every
tick; seated bots can't steer.

## Needed in acacia-physics
1. **Gliding**: the per-tick formula in the research doc (Bedrock applies drag and boost before the move),
   gliding pose box 0.6 x 0.6, stop on ground/water/levitation/durability < 2, 20-tick rocket boost.
   `Movement` must emit `StopGliding` itself on landing or water (vanilla does), and expose a "gliding"
   state the bot sets from the server's GLIDING flag (start is server-confirmed).
2. **Vehicles (driver seat of client-predicted vehicles: horses, camels, boats)**: boat (water float,
   paddle drag, ice), horse (walk, step height 1, jump charge 0.1/tick while held, fires on release),
   camel dash. Each tick `Position`/`Delta` describe the vehicle, plus `VehicleRotation` and
   `ClientPredictedVehicle` with flag 45 (not on mount/dismount ticks); boats also send PaddlingLeft/Right
   (swapped vs steering side). Controls: reuse `Controls` move vector + jump.
3. Replace `ride_physics_tick`'s "no simulation" branch with the vehicle simulation for driver seats;
   passengers and non-predicted vehicles keep the current behaviour.

## What the vanilla capture shows (2026-10-02, research/riding-fishing-elytra.md; per tick via `capdump --pai all`)
- **Gliding inputs**: no VerticalCollision while airborne, no persistent gliding flag; move vector/flags are plain
  WASD (Up held most of the glide). The start press held jump 3 ticks; every held-jump tick carries **WantUp**,
  which `movement/auth_input.rs` never sets (also missing on ordinary jumps: vanilla sends JumpDown, Jumping,
  JumpCurrentRaw, WantUp together). The rocket boost is client-predicted from the use tick (delta jumps from
  ~0.8 to ~1.1 blocks/tick within 2 ticks); BDS confirms with MovementEffect GLIDEBOOST (duration 56/42 ticks).
- **Landing**: the first ground tick has VerticalCollision and delta y 0; the next tick sends `StopGliding` alone
  (no VerticalCollision on it), then normal walking inputs. A jump press while gliding 0.2 s after the start did
  not stop the glide; one later in the air did (StopGliding + press flags).
- **ClientMovementPredictionSync** while gliding: bounding box height 0.6, data_flags gains bit 32.
- **Boat (driver)**: every seated tick, from the first, carries ClientPredictedVehicle, `vehicle_rotation (0, yaw)`
  with the yaw unwrapped (no wrap to ±180), position = boat wire position (y 70.95–70.99 on water), delta = boat
  velocity, VerticalCollision set. Paddling flags mirror the strafe key (Left → PaddlingLeft). BDS answers each
  boat input with CorrectPlayerMovePrediction in the capture (most ticks).
- **Pig**: server-driven; inputs report the seat (moving with the pig) with delta 0 and VerticalCollision.
- **Dismount**: the exit tick reports the exit point at feet = block top + 0.001 (boat exit onto the bank at
  x.30/x.70, i.e. pushed out of the bank's blocks; pig exit 1 block beside it). Choosing that point needs
  collision; the bot currently resumes at `Seat::leaving_feet`.

## Acceptance
Record drills on BDS strict movement (DESIGN.md "Checking physics") and replay offline:
- elytra: launch from a tower, glide straight and with pitch changes, one rocket boost, land on ground and
  in water (expect 0 corrections after the start tick);
- boat: paddle straight/turning on water and on ice; horse: walk, sprint, charged jumps over 1- and
  2-block steps.
Then a vanilla capture of the same moves through `tools/mitm` for the input-flag details still marked
UNKNOWN in the research doc.
