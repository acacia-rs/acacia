# acacia-anvil

Turns the Bedrock chunks an acacia bot holds (`acacia-world`) into a Java Edition world on disk: Anvil
`.mca` region files, zlib-compressed chunk NBT in Java 26.3's format (DataVersion 5023), with sky and block
light computed here. Built so bluemap-rs (or Java BlueMap) renders a Geyser-crawled server like Java BlueMap
renders the server's own world, and so `bluemap -u` picks up changes as the bots see them.

## Pipeline

1. `AnvilStore::update(chunk, block_entities)`: each Bedrock runtime id becomes a Java state (`mapping`),
   the liquid layer becomes `waterlogged=true` (or the water itself), biomes are sampled per 4×4×4 cell
   (`column.rs`). Then the block-entity pass (`blockentity.rs`) applies what Bedrock keeps in block entities.
   The column and its eight neighbours are marked dirty.
2. `AnvilStore::flush(force)`: a dirty column is written once all eight neighbours are in memory, or after
   `max_wait` (edge of the explored area), and a region file at most every `region_interval`. Per region:
   neighbour fixups (`fixup.rs`, sequential), then light, encode and zlib on `threads` workers, then
   temp-file + rename of the whole region (`region.rs`).
3. A chunk whose NBT is byte-identical to what was last written keeps its slot and timestamp. A changed one
   gets `max(now, old + 1)` seconds: bluemap-rs re-renders a tile when any chunk timestamp under it differs
   from the last render's (file mtime is not compared; crates/bm-engine `actions.rs` there).
4. Memory: columns stay as neighbours until `memory_limit`, then the oldest written ones are dropped.

`examples/crawl.rs` is the ingest loop: subscribe to the bot's `World`, read `chunk_positions()` once,
coalesce `ChunkChange`s every 500 ms, feed the store, flush. Block entities come from the bot's
`BlockEntities` (physics bots track all of them).

## Data sources (regenerate with `node crates/acacia-anvil/tools/gen-mapping.mjs [cacheDir]`)

Output `data/mapping.bin` (layout in the generator, parsed by `src/mapping/blob.rs`) and `data/ambiguous.txt`.
Inputs in `tools/.cache` (git-ignored):

- `blocks.nbt`, `biomes.json`: GeyserMC/mappings at `81700ab` (Java 26.3 + Bedrock 26.50, MIT), fetched by
  the generator. `blocks.nbt` is indexed by Java state id; a missing `bedrock_identifier` means the Java name,
  as in Geyser's `BlockRegistryPopulator`.
- `java-26.3-blocks.json`: the vanilla data report,
  `java -DbundlerMainClass=net.minecraft.data.Main -jar server-26.3.jar --reports` (`reports/blocks.json`).
- `java-26.3-light.json`: `tools/javadump/LightDump.java` run against the unobfuscated server jar (classpath:
  the bundler's `versions/26.3/server-26.3.jar` plus `libraries/**.jar`, which the report run extracts):
  per state `getLightDampening`, `getLightEmission`, the full faces of `LightEngine.getOcclusionShape` for
  shape-occluding states, air and fluid bits.
- `tags/`: `unzip server-26.3.jar 'data/minecraft/tags/block/*'`. Heightmaps test `#blocks_motion` and
  `#blocks_motion_no_leaves`, which need a datapack, so the generator resolves them.

## Mapping

- Many Java states share one Bedrock state. The generator keeps the one closest to its block's default
  (fewest non-default properties, `waterlogged` counted heavily so dry wins; ties to the lowest id).
  `data/ambiguous.txt` lists, per Java block, the properties Bedrock loses.
- Bedrock states Geyser never sends map to the mapped state of the same block sharing the most properties.
- Geyser sends some blocks as its own custom blocks (`geyser_custom:obsidian`, `crying_obsidian`, the three
  huge mushroom blocks on Geyser-Spigot 2.12): mapped to the Java block of the same path, default state.
  Anything else unknown becomes air (`StateMap::unmapped`).
- Biomes: Geyser's Java name → Bedrock id, inverted; shared ids pick `river` (7, also `the_void`) and
  `the_end` (9, all End biomes). Unknown ids are plains.

## Rebuilt properties

From neighbours (`fixup.rs`; each rule reads only properties no rule writes): snowy grass/podzol/mycelium,
huge mushroom faces (hidden against the same block), kelp/twisting/weeping/cave vine tips vs bodies,
redstone wire (Java's `getConnectionState`, a lone wire is a cross), piston `extended`.

From block entities (`blockentity.rs`): bed colour (`color`), banner colour (`Base`, counted 15 − dye), potted
plant (`PlantBlock`), floor skull rotation (`Rotation` / 22.5), shulker box facing, double chests
(`pairx`/`pairz`), lectern books. BlueMap draws block entities from their states only, so chunks carry an
empty `block_entities` list.

Since Bedrock 26.50, fences, panes, bars, walls, stairs and tripwire carry their shapes in the state.

### Known differences

- A redstone dot (a wire toggled to connect nowhere) comes out as a cross: Bedrock has no dot.
- Lost and not rebuilt (not visible on a map, or rare): leaves `distance`, note block instrument/note,
  button/lever floor rotation, `powered` on doors/gates/trapdoors, repeater `locked`, piston head `short`,
  fire and vine `up` faces, chorus plant faces, jukebox record, decorated pot `cracked`, copper golem pose,
  campfire `signal_fire`, sculk sensor power, scaffolding `bottom`.
- Huge mushroom blocks through `geyser_custom` lose their faces; the neighbour rule rebuilds Java's
  `updateShape` result (inside faces hidden), not worldgen's (cap undersides are pores in Java worldgen).
- Light treats a missing neighbour column as opaque. A column relit after a neighbour was evicted from
  memory loses that neighbour's light; reading evicted neighbours back from the region files is still open.
- Light occlusion uses full faces only; Java also blocks light where two partial faces together cover a face.

## Oracle

`py -3 crates/acacia-anvil/tools/oracle.py` (steps `server|crawl|trim|golden|candidate|diff`, default all):

1. Lab in `.testserver/anvil-lab` (git-ignored): Paper 26.3 build 159 + Geyser-Spigot (offline auth,
   `validate-bedrock-login: false`, Bedrock :19150), seed `acacia-anvil`, peaceful, no whitelist.
2. `tools/scene.py` builds a test scene at y 150 over (0, 0) by console: every Java block's default state on a
   2-block grid (`strict`, so nothing reshapes), and hand-built cases for every rebuilt property (both chest
   pairs per facing, 16 bed/banner/wall-banner/shulker colours, 12 pots, 16 skull rotations, redstone lines,
   corners, steps, snowy grass, a mushroom cap, a kelp tank, weeping vines, an extended piston, stair corners,
   fence/wall/pane/bar lines, a closed torch-lit room, a slab-roofed room, a leaf canopy, lit blocks).
   Random ticks, fire, daylight and mob spawning are off so the world holds still.
3. `examples/crawl.rs` joins as `Anvil` at the scene and writes `candidate/world`.
4. The server's own world is cut to the chunks the bot saw (`golden/world`), rendered by Java BlueMap 5.29;
   the candidate is rendered by bluemap-rs (built from that checkout into the lab's own target dir).
5. `bm-golden diff-render <java web> <rs web> --inset 16`; `tools/lowres_diff.py` compares lowres pixels.
   `tools/scene.py x,y,z …` names the grid block at a diff cell; `tools/mca_block.py <world>… -- x,y,z …`
   prints state and light at a position in both worlds; `ANVIL_PROBE="x,y,z;…"` on the crawl prints the
   Bedrock states the bot held there.

Results (2026-10-24, about 400 columns around the scene):

| Run | Faces compared | Identical | Mismatching faces | Lowres pixels differing |
|---|---|---|---|---|
| natural terrain, no scene | 756 034 | 100.00% | 0 (tiles byte-identical once ungzipped) | 0 |
| scene, Bedrock-derived light, no `geyser_custom` | 809 538 | 99.95% | 434: mushroom blocks/obsidian as air (`geyser_custom`), cauldron/shelf/shelf_mushroom light, artificial strict-placed plant bodies and dust dots | — |
| scene, Java light dump, `geyser_custom` mapped | 689 492 | 100.00% | 96: only the artificial cases above | 0 |
| scene without the artificial cases, area cleared first | 689 468 (911 890 without `--inset`) | 100.00% | 0 | 0 |

## Throughput

`cargo run --release -p acacia-anvil --example bench -- 32` (synthetic terrain, 1024 columns, 2026-10-24, this
machine under load): convert ~3 600 columns/s on one thread; light + encode + compress + write ~200 columns/s
per thread, ~1 800/s with all cores (`threads: 0`). Light is most of it: each column fills a 48×48 block
neighbourhood.
