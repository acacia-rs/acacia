# Shulker collision box vs peek (BDS)

Source: bdsre decompile of the Windows BDS 1.26.52 exe (`.testserver/bds-blocks/bedrock_server.exe`, bdsre build
6ab54bee) plus LeviLamina headers. Addresses below are that Windows build; the Linux ELF addresses were not
located (stripped; find by RTTI `7Shulker` vtable, slot of `postNormalTick`). Live facts that agree: base
top 0.98 (player stood at 0.98) and the 0.0001 x/z inset (walker stopped at 0.4999).

## Functions (Windows VA)

| VA | Name (inferred from header + body) |
|----|------|
| 142991370 | `Shulker::Shulker` ctor: sets data 0x41 AttachFace (byte 0), 0x43 AttachPos (BlockPos), 0x42 (short), 0x40 Peek (int 0); adds ShulkerFlag, ShulkerPeekAmount, PermanentSkipMobTravel |
| 1429933b0 | `Shulker::postNormalTick` (server path): attach checks → `_peekAmountTick` → snap position → `_calculateBB` |
| 142993b50 | `Shulker::_peekAmountTick` |
| 142992140 | `Shulker::_calculateBB` (writes AABBShapeComponent and IsSolidMobComponent depenetration) |
| 14848dfe0 | PeekSystem per-entity: sets data 64 to 30 (random peek, 20 + 20·rand(3) ticks) or 0 on close |
| 142f521a0 | IsSolidMobComponent setup: default depenetration magnitude (0.1, 0.1, 0.1) unless `CustomDepenetrationMagnitudeComponent` |
| 14251abe0 | `SolidMobSystem::flagSolidMobsFromNearbyEntity`: entities whose shape grown by 2 meets a solid mob get `IsSolidMobNearbyComponent` = per-axis max of the mobs' magnitudes + their AABBs |

## The rule (BDS-confirmed, Windows decompile)

```
// _peekAmountTick (142993b50); ShulkerPeekAmountComponent = InterpolationPair { cur, prev }
target = (data[64] is Int) ? data[64].int * 0.01 : 0.0     // key 64 ShulkerPeekId is an INT (type 2), not a byte
prev = cur
if cur < target: cur = min(max(cur + 0.05, 0.0), target)
elif cur > target: cur = min(max(cur - 0.05, target), 1.0)

// postNormalTick (1429933b0), after the peek tick:
attach = data[67] (BlockPos, type 6) else floor(pos)       // + attach checks / teleport, not needed here
pos = (attach.x + 0.5, attach.y, attach.z + 0.5)           // y is the block bottom

// _calculateBB (142992140)
face = (data[65] is Byte) ? data[65] : 0                   // key 65 ShulkerAttachFace
if face > 5: keep previous AABB, no change
phys = 0.5 - mc_sin((cur + 0.5) * PI) * 0.5                // Mth sine table 0x14cc495f8; cur is the NEW value
box  = [px-0.5, py, pz-0.5] .. [px+0.5, py+0.98, pz+0.5]   // base height 0.98, not 1.0
switch face:                     // grows away from the attached block; normal n
  0 (down, on floor):   max.y += phys      n = (0, 1,0)
  1 (up, on ceiling):   min.y -= phys      n = (0,-1,0)
  2 (north):            max.z += phys      n = (0,0, 1)
  3 (south):            min.z -= phys      n = (0,0,-1)
  4 (west):             max.x += phys      n = ( 1,0,0)
  5 (east):             min.x -= phys      n = (-1,0,0)
// inset x and z only (y untouched); if an axis would invert it collapses to its centre
box.min.x += 0.0001; box.max.x -= 0.0001; box.min.z += 0.0001; box.max.z -= 0.0001
AABBShape = box (width = max(dx, dz), height = dy)
IsSolidMob.depenetrationMagnitude = |n| * (phys * 1.1)     // zero on the other two axes; zero when closed
Offsets.y = height * (face <= 1 ? 0.4 : 0.5)               // not collision
```

All arithmetic f32. So the floor shulker's top is `py + 0.98 + phys`, x/z width 0.9998, at every peek.

Peek targets on the wire: 0 (closed), 30 (random peek, `0x1e` in PeekSystem), and presumably 100 while attacking
(not read here; Java uses 1.0). Steps are 0.05 a tick, so 30 takes 6 ticks, 100 takes 20.

| cur | 0.05 | 0.10 | 0.15 | 0.20 | 0.25 | 0.30 | 1.0 |
|---|---|---|---|---|---|---|---|
| phys | 0.0062 | 0.0245 | 0.0545 | 0.0955 | 0.1464 | 0.2061 | 1.0 |
| floor top | 0.986 | 1.005 | 1.035 | 1.075 | 1.126 | 1.186 | 1.98 |

The live 0.98 is a closed shell; the live 1.13 matches cur = 0.25, one tick short of a 30 peek (mid-animation).

## Why 0.98 / why 0.9998

Not a position offset: the position is the block bottom (snapped every tick). The base box is hard-coded
0.98 tall (`fVar22 + 0.98` in every case of the switch) and inset 0.0001 on x/z only. The pack's
`collision_box` does not set this box: the ctor removes the entity's `AABBRelativeSizeUpdateComponent` and
`_calculateBB` overwrites AABBShape every tick.

## How a player rides a rising shell (inferred, medium confidence)

`flagSolidMobsFromNearbyEntity` gives any mover within 2 blocks of a solid mob an `IsSolidMobNearbyComponent`
holding the max per-axis magnitude of those mobs and their boxes; `storeNearbyMobsOnMoveRequest` (header) moves
it into the mover's `DepenetrationComponent` and its `MoveRequest`. So a player near an opening floor shulker
may depenetrate up to `phys * 1.1` along y (and 0 on x/z) out of the shell; that is what lifts a player standing on
it. Without that allowance the player, now inside the box, would not be pushed out (acacia's `clip.rs` models
"no depenetration" for players). The store lambda itself was not decompiled; confirm with a drill.
Side note: other solid mobs (boats) default to magnitude 0.1 per axis, the same mechanism.

## Java cross-check (Java-assumed, not BDS)

Java `Shulker`: same 0.05 step toward target, same `physicalPeek` formula, box grown along `attachFace.opposite`.
Java differs: 1.0 base height, no 0.0001 inset, and pushes entities in `onPeekAmountChange` itself.

## Unknowns

- Order within a server tick of PeekSystem (sets data 64) vs `postNormalTick` (steps toward it), and when
  SetActorData goes out: whether the client's first tick after receiving 30 is server step 0 or 1. Drill it.
- Ceiling/wall shulkers are not drilled; signs above follow the switch directly.
- Linux build: same rule assumed (live 0.98 and 0.4999 agree); addresses not found.

## Port suggestion (`state/entities/solids.rs`)

Per shulker, track `peek_target: f32` (metadata 64 as Int / 100; accept Byte too if acacia decodes it so),
`peek: f32` (the server's `cur`), `face: u8` (metadata 65, default 0) and its position (or metadata 67 + 0.5).
- On spawn: `peek = target` (best guess; an AddActor mid-animation is off by at most a few ticks).
- Each client tick, before the player's physics step: `peek` steps toward `peek_target` by 0.05 as above.
- On a metadata update: set `peek_target` only; never jump `peek`.
- `solid_box`: build the box from position, `face` and `phys = 0.5 - mc_sin((peek + 0.5) * PI) * 0.5`
  (`acacia_physics::math::mc_sin`), base 0.98, x/z inset 0.0001; drop the `(0.9998, 1.0)` SOLID entry and the TODO.
- Expose a per-axis depenetration allowance `|n| * phys * 1.1` (max over solid mobs within 2 blocks) and let
  `clip.rs` move the player out of an overlap along that axis up to the allowance (now it allows none).
- Validate with `tests/traces/entity-shulker.btrc.gz` (`trace_regression.rs`), then drill a wall and a ceiling
  shulker and a 100 peek.
