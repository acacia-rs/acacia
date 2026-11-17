# Bedrock physics: what is left

Working list for closing acacia-physics against strict BDS (branch `physics/gaps`). An item is ticked only with
evidence: a trace in `crates/acacia-bot/tests/traces` (its known mismatch count in `tests/trace_regression.rs`), or
a live check in `examples/actions`.

## Closed
- [x] Elytra: glide axis/off-axis/pitch, rocket boost, landings: `glide-phases`
- [x] Elytra wear (damage on glide tick %20 == 19, no slowdown on the stop tick): `glide-worn` 0
- [x] Elytra equip by right-click: `actions` `equip_use_elytra` PASS
- [x] Camel, boat and horse driving; boat back seat: `vehicle-camel` 0, `vehicle-boat-horse`, `vehicle-back-seat`
- [x] Strider seated on lava (summoned `minecraft:on_saddled`): `vehicle-strider` 0
- [x] Flight: survival may-fly, sprint, lava, water (floor swim start flies unboosted): `flight-*` 0
- [x] Levitation in water and lava: `levitation-liquids` 1 (the effect starting a tick before its packet)
- [x] Shulker box follows its peek (0.05 a tick, 0.98 shell, 0.0001 inset): `entity-shulker` 0
- [x] Riptide in water (launch, spin box, stop rules): `riptide`; in rain (weather ramp, biome, rain top):
      `riptide-rain` 0
- [x] Spectator no-clip; crawl entry and its input flags; powder snow sneak-on-top and stuck fall reset: `drills-gaps`
- [x] Dismount exit spot ignores ladders, vines, open trapdoors, scaffolding, powder snow (unit test)
- [x] `FlyingPlayerStuckOnGroundWorkaroundSystem` (`flight.rs`)

## Not physics gaps (explained, kept in the known counts)
- Teleport into chunks not yet received: the simulation freezes while BDS walks a tick (glide land_offaxis and
  land_water, the "teleport while walking" tick).
- Boat dismounts: the empty boat drifting into the player; BDS's push timing jitters ±1 tick per run (memory
  `bds-drifting-boat-pushes-player`). Blue ice: the boat slid off the drill pad and the exit was mid-air.
- A re-equipped spent elytra lasting twice as long: only seen swapping by command.
- Correction mismatches in `... build` marks: drill setup between phases.

## Residual
- [ ] Shulker opening under a standing player: BDS's solid-mob depenetration (|normal| × peek × 1.1) is not ported
      (`clip.rs` gives entities none); never seen in a drill.
- [ ] Riptide rain: temperature above y 80 and the frozen-ocean noise (base biome temperature used); the per-block
      rain obstruction table is sapling's worldgen fit, not drilled (docs/research/riptide-rain.md).
- [ ] One-ulp x after a flat riptide slides on the ground (`riptide-rain` rain_flat, dry_none; within tolerance).
- [ ] Powder snow in boots: ±0.00038 x velocity, positions exact (`drills-gaps` powderboots).
- [ ] horse_ice: a one-run ulp flake.

## Not ported (not worked on this branch)
- [ ] Step tie-breaker, slide offset
- [ ] Legacy sprint timing, server-forced sprint
- [ ] `AutoJumpingInWater` input flag
- [ ] Scaffolding's unsupported-bottom lip
