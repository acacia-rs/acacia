# Riptide in rain (BDS 1.26.52, static)

Sources: bdsre on `.testserver/bds-blocks/bedrock_server.exe` (Windows 1.26.52), LeviLamina headers for offsets,
sapling's live drills and ports (cited inline). Nothing here comes from Java Edition unless it says so. bdsre labels added:
`Weather::isPrecipitatingAt` 14119d6b0, `BlockSource::getBiome` 141602f90, `Biome::getTemperature` 14119c8c0,
`LevelChunk::getTopRainBlockPos` 140a09950, `Block::tryGetPrecipitationInteractions` 144c4e400, `Weather::tick`
14119d160, `Weather::_doNaturalWeatherCycle` 14119d290. See riptide-impact-exit.md for the launch itself.

## The rule
`TridentItem::releaseUsing` (141fa6b10) takes the riptide branch if `Actor::isInWaterOrRain` (140d2a130) is true and the
player is not riding. `isInWaterOrRain`:
```
if has(WasInWaterFlag): return true
p = BlockPos(floor(mPos.x), floor(mPos.y + mBBDim.y), floor(mPos.z))  // StateVector +0x218, AABBShape.mBBDim.y +0x1c
w = dimension.mWeather (+0x1e8);  region = dimension's BlockSource
return Weather::isPrecipitatingAt(w, region, p) && Biome::getTemperature(getBiome(region, p), region, p) > 0.15
```
For a player `mPos` is the eye position, feet + 1.62001 (acacia `DEFAULT_PLAYER_HEIGHT_OFFSET`), so the one cell
tested is `floor(feet + 1.62001 + h)`, with `h` the box height: 1.8 standing (feet + 3.42), 1.5 sneaking, 0.6 while
swimming, gliding or crawling. Feet and head are not tested. Since the cell is above the head, a player in a two-high
tunnel whose roof is one block thick (roof at feet + 2, sky above it) counts as in rain. This needs a drill.

`Weather::isPrecipitatingAt(region, p)` (14119d6b0):
```
if !dimension.mHasWeather (Dimension +0x100): false     // overworld only
if !biome(p).mRain (Biome +0x40): false                  // BiomeDefinition.rain
if !(w.mRainLevel (+0x38) > 0.2): false                  // current level, not interpolated; thunder not read
if p.y < region.minY: false
if p.y < region.maxY:                                    // fast reject on the light heightmap (LevelChunk +0xb80)
    chunk && p.y - minY < chunk.mHeightmap[col]: false
top = getTopRainBlockPos(chunk(p), col).y                // no chunk: -1, so "true"
return top <= p.y
```
`getTopRainBlockPos` (140a09950): cached per column in `mRainHeights` (LevelChunk +0x1160). Starting from the top
sub-chunk that is not uniform air, it scans down. The first y at which the layer-0 block, or a non-air layer-1 block,
obstructs gives `top = y + 1`. If nothing obstructs, `top = minY - 1`.
A block obstructs when `tryGetPrecipitationInteractions(blockType.defaultState)` is null or 1/2. The values are
`none`=0, `obstruct_rain`=1, `obstruct_rain_accumulate_snow`=2 and `snowlogging`=3, the last one shown by
`BlockType::canBeBuiltOver` 141a254f0 testing `== 3`. The permutation's state is ignored and the type's default
state is what counts.
Native blocks get the component through the cereal schema (the only adder, 144c4bb90), so the per-block values are
not visible statically. Sapling's oracle-fitted worldgen rule (`sapling-worldgen/src/feature/freeze.rs::stops_rain`)
is **everything except air and plants, with crops still obstructing** (`Block::is_plant` in
sapling-worldgen/src/block/mod.rs). By that rule glass, leaves, water and slabs all obstruct rain, and so does any
non-air block. The light heightmap is not the test (sapling `cells.rs::is_rained_on` uses sky light 15, which is a
different rule).

`Biome::getTemperature(region, p)` (14119c8c0) is already ported bit-exact, fitted to the oracle, in
sapling `crates/sapling-worldgen/src/surface/temperature.rs`:
```
t = biome.mTemperature (+0x08)
if biome has the partially-frozen component (frozen_ocean 46, deep_frozen_ocean 47):
    if sum(N3456x3, p, 0.05) + sum(N2345, p, 0.2) < 0.3 && sum(N2345, p, 0.09) < 0.8: t = 0.2  // no Java ×7
cool = seaLevel(63) + 17                         // +1 on old base-game versions (BaseGameVersion gate)
if p.y > cool: t += (p.y + int8(sum(N1234, p, 0.125)·64)·0.125 − cool)·−0.05/40   // per-column byte, chunk +0x383
```
Rain needs `t > 0.15` (strict). At `t <= 0.15` it snows, and snow does not launch.

## Rain level over time
- `Weather::tick` (14119d160) runs every server tick. It sets `old = rain` and moves `rain` 0.01 towards the target
  (+0x3c, which is 0 or 1), clamped to [0, 1]. In f32 the level passes 0.2 on the **20th tick** after the target goes to 1, and it falls
  back to ≤ 0.2 on the 81st tick after it goes to 0. A sapling drill confirmed this (mob-damage.md "What the rain's
  level drives": riptide threw 2-3 s after `/weather clear` but not 9 s after; rain damage came 1009 ms after
  `/weather rain`).
- Only the natural cycle (14119d290) sends LevelEvent `StartRain` 3001 or `StopRain` 3003. Their `data` is
  `floor(float(newRainTime) · 65535)`, which saturates to INT_MIN for every real duration. The client's handler
  (14119d11d) sets target = data/65535 (`StopRain` sets 0; `StartThunder` 3002 and `StopThunder` 3004 do the same for
  lightning). So `data` cannot be read as an intensity. Treat 3001 as target 1 and 3003 as target 0.
- `/weather` calls `Level::updateWeather` (vtable +0xb40) with rain 1 or 0. I did not follow how it reaches clients.
- `StartGame.rain_level`: whether this is the current level or the target is not established. Treat > 0 as target 1
  and start the ramp from it.

## What the client has
- **Weather**: `acacia-bot/src/state/environment.rs` keeps `rain`, but it sets the level straight from the event,
  takes data/65535 as the level and has no ramp. It needs `rain_target` + `rain_level`, ticked 0.01 per client tick.
- **Biomes**: chunk biome ids per section, `acacia-world/src/chunk/mod.rs::Chunk::biome(x, y, z)` (decoded by
  `chunk/biomes.rs`; `view.rs::insert_biomes` for the blob/sub-chunk modes). The bot does not decode
  `BiomeDefinitionList` today. That packet carries `temperature` and `rain` per biome (acacia-proto
  `types::BiomeDefinition`, part_04.rs:81). Vanilla entries have `biome_id = UNSET`, and their ids come from
  `acacia-render/src/biome/ids.rs`. acacia-render parses the list for tints (`biome.rs`). Its
  `weather/climate.rs` is the **Java** 26.3 rule (×7 frozen noise, `< 0.15` snow) and must not be reused here.
- **Heightmaps / sky**: acacia-world has only the server-side encoder heightmap (`chunk/heightmap.rs`, light-based,
  dragonfly), and acacia-bot/src/world has nothing. Neither matches the rain top, so it has to be computed from the
  blocks.
- **Dimension**: rain is overworld-only (`mHasWeather`). The dimension comes from StartGame/ChangeDimension.

## Port plan
1. `acacia-bot/src/state/environment.rs`: `rain_target: f32`, `rain_level: f32`; StartRain → 1, StopRain → 0,
   StartGame → `rain_level > 0`; `tick()` ramps by 0.01 with the clamp; `rains() = rain_level > 0.2`. Call `tick` from
   the bot's per-tick step before movement.
2. `BiomeDefinitionList` → a `Biomes` table `id → (temperature, rain, partially_frozen)` (frozen = vanilla 46/47).
   Put it in acacia-registry or acacia-world so the bot and acacia-render share one parser.
3. `acacia-world/src/climate.rs` (new): port sapling's `temperature.rs` (it needs sapling-worldgen's `SimplexNoise`
   and `Mt19937`, about 100 lines; copy them, since acacia must not depend on sapling).
   `World::rain_top(x, z)`: scan down from the top loaded section for the first cell whose layer 0 or a non-air layer 1
   `obstructs_rain`. Cache it per column and invalidate the cache on block updates in that column.
4. acacia-registry: `BlockState::lets_rain_through` (air + sapling's plant set, minus crops), generated by name.
5. `acacia-bot/src/movement/riptide.rs::predict_riptide`: `touched_water() || in_rain(world, env, feet, h)` with
   `cell = floor(feet + 1.62001 + h)`, `h` from the pose (`collision_dims`), and the overworld only. Then
   `env.rains() && biome.rain && rain_top(cell.xz) <= cell.y && temperature(cell) > 0.15`. Read the state as of the
   tick the ReleaseItem lands on.

## Confirmed live (riptide drill, 2026-11-12)
Ported as planned except the y > 80 temperature and the frozen-ocean noise (base biome temperature). `/weather`
reaches the client as StartRain/StopRain: `rain_ground` predicted and drew 0 corrections, and `dry_none`, released
~45 ticks after `/weather clear`, launched on both sides (the 81-tick ramp down). Trace `riptide-rain`.

## Unconfirmed / drills
- That the tested cell is eye + box height. The drill: riptide in a 2-high tunnel under a 1-thick roof (BDS should
  launch) and under a roof at feet + 4 (no launch), standing and sneaking.
- The per-block obstruct table. Glass, leaves, slabs and a carpet roof each need a drill, and a flower/grass "roof"
  should not block.
- The `data` of StartRain on the wire, for the natural cycle and for `/weather` (capdump). Also StartGame.rain_level
  mid-ramp.
- Prediction near the 20-tick edge depends on the server/client tick offset, so expect a correction there.
