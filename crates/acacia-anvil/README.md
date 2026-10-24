# acacia-anvil

Turns the Bedrock chunks an acacia bot holds (`acacia-world`) into a Java Edition world on disk: Anvil
`.mca` region files, zlib-compressed chunk NBT in Java 26.3's format (DataVersion 5023), with sky and block
light computed here. Built so bluemap-rs (or Java BlueMap) renders a Geyser-crawled server like Java BlueMap
renders the server's own world, and so `bluemap -u` picks up changes as the bots see them.

## Pipeline

1. `AnvilStore::update(chunk, block_entities)`: each Bedrock runtime id becomes a Java state (`mapping`),
   the liquid layer becomes `waterlogged=true` (or the water itself), biomes are sampled per 4×4×4 cell
   (`column.rs`), and the column is padded to Java's height (the nether is 128 high in Bedrock, 256 in Java;
   `dimension.rs`). Then the block-entity pass (`blockentity.rs`) applies what Bedrock keeps in block
   entities. The column and its eight neighbours are marked dirty, unless it is in memory exactly as it
   came last time (a relog or overlapping views send it again).
2. `AnvilStore::flush(force)`: a dirty column is ready once all eight neighbours are in memory, or after
   `max_wait` (edge of the explored area). A region file is rewritten at most every `region_interval`, and
   only once no column in it was dirtied for `region_interval` (the bot has moved on), or once a ready column
   has waited `region_hold`. Per region:
   neighbour fixups (`fixup.rs`, sequential), then light (groups of nearby columns over one volume),
   encode and zlib on `threads` workers, then temp-file + rename of the whole region (`region.rs`).
3. A chunk whose NBT is byte-identical to what was last written (or, for one this store has not written,
   to what the region file holds) keeps its slot and timestamp. A changed one gets `max(now, old + 1)`
   seconds: bluemap-rs re-renders a tile when any chunk timestamp under it differs from the last render's
   (file mtime is not compared; crates/bm-engine `actions.rs` there).
4. Memory: columns stay as neighbours until `memory_limit`, then the oldest written ones are dropped. A
   dropped column is read back from its region file (`store/disk.rs`: a presence bitmap per region for up
   to 16 384 regions, the last region's offset table, one slot read per column) when a dirty column needs it
   as a neighbour, or when a neighbour of it changes and it must be relit. Nothing else the store keeps grows
   with the explored area (see Scale).

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
- Light treats a neighbour column never seen as opaque (the edge of the explored area). Evicted columns are
  read back from the region files (`store/disk.rs`), as neighbours and to be relit when a neighbour changes.
- Light occlusion uses full faces only; Java also blocks light where two partial faces together cover a face.
- The nether above its roof (y 128–255) is air: Geyser gives Bedrock only y 0–127, so roof mushrooms and
  anything built up there are lost. With Geyser's `above-bedrock-nether-building` the nether arrives shaped
  like the End and would be filed (and lit) as the End; the converter has no switch for that yet.
- All End biomes arrive as `the_end` (Geyser gives them one Bedrock id); they share their colours, so the
  map does not change.
- Not written: a column with sections the server never sent (request mode) until it is complete, Geyser's
  filler chunks (all air, every biome Bedrock id 0: what Geyser sends for a Java unload or a chunk it has
  no data for), and any all-air column where one was already seen. A real all-air chunk (End void) comes
  with its biomes and is written. A world edit that empties a whole column to air would not be picked up.

## Oracle

`py -3 crates/acacia-anvil/tools/oracle.py [--dims all] [--memory 0]` (steps
`server|crawl|trim|golden|candidate|diff`, default all; `--dims` also crawls the nether at (0, 64, 0) and the
End at (0, 70, 0), each its own map with BlueMap's own nether/end map config; `--memory 0` evicts every
written column, so neighbours come back from the region files):

1. Lab in `.testserver/anvil-lab` (git-ignored): Paper 26.3 build 159 + Geyser-Spigot (offline auth,
   `validate-bedrock-login: false`, Bedrock :19150), seed `acacia-anvil`, peaceful, no whitelist.
2. `tools/scene.py` builds a test scene at y 150 over (0, 0) by console: every Java block's default state on a
   2-block grid (`strict`, so nothing reshapes), and hand-built cases for every rebuilt property (both chest
   pairs per facing, 16 bed/banner/wall-banner/shulker colours, 12 pots, 16 skull rotations, redstone lines,
   corners, steps, snowy grass, a mushroom cap, a kelp tank, weeping vines, an extended piston, stair corners,
   fence/wall/pane/bar lines, a closed torch-lit room, a slab-roofed room, a leaf canopy, lit blocks).
   Random ticks, fire, daylight and mob spawning are off, and `tick freeze` before the crawl keeps fresh
   chunks (nether lava falls) from flowing on after the bot has left, so the world holds still.
3. `examples/crawl.rs` joins as `Anvil` at the scene and writes `candidate/world`, then visits the other
   dimensions (`ANVIL_TP="the_nether 0 64 0;…"`).
4. The server's own world is cut to the chunks the bot saw (`golden/world`), rendered by Java BlueMap 5.29;
   the candidate is rendered by bluemap-rs (built from that checkout into the lab's own target dir).
5. Per map, `bm-golden diff-render <java web> <rs web> --map <id> --inset 16` and `tools/lowres_diff.py`
   (lowres pixels). `tools/scene.py x,y,z …` names the grid block at a diff cell;
   `[ANVIL_DIM=the_nether] tools/mca_block.py <world>… -- x,y,z …` prints state and light at a position in
   both worlds; `ANVIL_PROBE="x,y,z;…"` on the crawl prints the
   Bedrock states the bot held there.

Results (2026-10-24, about 400 columns around the scene):

| Run | Faces compared | Identical | Mismatching faces | Lowres pixels differing |
|---|---|---|---|---|
| natural terrain, no scene | 756 034 | 100.00% | 0 (tiles byte-identical once ungzipped) | 0 |
| scene, Bedrock-derived light, no `geyser_custom` | 809 538 | 99.95% | 434: mushroom blocks/obsidian as air (`geyser_custom`), cauldron/shelf/shelf_mushroom light, artificial strict-placed plant bodies and dust dots | — |
| scene, Java light dump, `geyser_custom` mapped | 689 492 | 100.00% | 96: only the artificial cases above | 0 |
| scene without the artificial cases, area cleared first | 689 468 (911 890 without `--inset`) | 100.00% | 0 | 0 |
| same, `--memory 0` (every neighbour read back from disk) | 809 490 | 100.00% | 0 | 0 |
| same, grouped light | 697 686 | 100.00% | 0 | 0 |
| `--dims all`: overworld | 809 490 | 100.00% | 0 | 0 |
| `--dims all`: nether, 401 columns | 899 453 | 99.99% | 88: red mushrooms on the roof (y 128, above what Geyser sends) | 12 of 8 032 032 (lod 1), the same mushrooms |
| `--dims all`: End, 401 columns | 93 846 | 100.00% | 0 | 0 |
| after the scale changes (debounce, parallel read-back) | 702 850 | 100.00% | 0 | 0 |

Face counts vary between runs with how far the bot's view reached. Fixed on the way to the nether and End
rows: Java's nether is 256 high (columns padded, heightmaps at 9 bits); Java 26.3's End has sky light
(`has_skylight: true` in its dimension type, unlike Bedrock's); Geyser's filler chunks (all air, biome id 0,
sent for a Java unload or the edge of the view) had overwritten seen terrain with air (53 nether columns).

## Live updates

`py -3 crates/acacia-anvil/tools/live.py [--side 40]` runs bluemap-rs `bluemap -u` (built as the oracle builds
it, `update-cooldown: 1`) on a synthetic world that `examples/live.rs` writes while it watches (40×40
chunks, four regions), and checks:

- a. a block placed in a chunk's interior re-renders exactly the hires tiles over that chunk (4), and only the
  one holding the block changes;
- b. re-sending unchanged chunks writes nothing (region file byte-identical, no update scheduled); rewriting
  a region for one changed chunk next to unchanged ones re-renders only the changed chunk's tiles;
- c. four bursts of edits (glowstone, gold, stone, air), each started as bluemap begins a region update, so
  regions are replaced while bluemap reads them: a reader thread parses every region version it can open
  (no torn file, no temp file left), bluemap logs no warning, and the incremental map equals a fresh `-r -f`
  render of the final world (every face, lowres pixels).

Result (2026-10-24, after the scale changes): all pass; 392 region reads, 0 torn, 6 writes landed during a region update; incremental
render 4 374 744 of 4 374 744 faces identical to the fresh one, lowres 0 pixels differ.

## Throughput

`cargo run --release -p acacia-anvil --example bench -- 32` (synthetic terrain, 1024 columns in one region,
2026-10-24, 22 threads, `ANVIL_THREADS=1` for one):

| | convert | light + encode + compress + write, 1 thread | same, all cores |
|---|---|---|---|
| a 48×48 volume per column | ~9 500/s | 570/s | 4 600/s |
| groups over one shared volume (`store/write.rs`) | ~9 500/s | 1 340/s | 8 600/s |
| groups, before the light work (below), rerun interleaved with the next row on a loaded machine | ~6 900/s | 877/s | 4 640/s |
| groups, after the light work | ~6 400/s | 985/s | 5 490/s |

Dirty columns are lit in aligned 8×8 groups over one volume padded by a column (10×10 columns filled for 64
lit, against 9 per column), 4×4 or 2×2 when that leaves workers idle; a sparse group is lit column by
column. The region file is byte-identical either way (`light::tests::a_lit_area_matches_lighting_each_column_alone`
proves the light part on rough terrain).

### Light

`cargo run --release -p acacia-anvil --example light -- [side=32] [iters=5] [mode=area|column|par]
[threads=N] [group=8]` lights the scale harness's terrain (below), converted once up front, and prints a
hash of the extracted light (`7f9a0770720b6e6f` for area, `792de70165426f4d` per column on Linux; equal
hashes = identical light). `tools/profile_light.sh <stat|record|annotate|heaptrack|time> <light|bench>`
profiles it on Linux, `tools/ab_light.sh <checkout A> <checkout B>` runs interleaved `perf stat` A/B.

Testbox (Xeon E-2136, 6 cores / 12 threads), 2026-10-24, before the light work → now:

| | instructions | cycles | L1d misses | LLC misses | columns/s |
|---|---|---|---|---|---|
| light, 8×8 groups, 1 thread (with extract) | 37.1G → 16.7G | 15.5G → 7.2G | 529M → 188M | 7.9M → 3M | 1 840 → 4 600 |
| light, column by column, 1 thread | 31.9G → 13.3G | 11.5G → 5.2G | 224M → 124M | — | 409 → 979 |
| light, 8×8 groups, 12 threads (`mode=par side=64`) | 55.8G → 26.9G | 48.5G → 29.2G | 810M → 306M | 47.6M → 29.9M | 7 660 → 14 400 |
| full bench 32, 1 thread (light + encode + compress + write) | 19.9G → 17.5G | 8.6G → 7.6G | 112M → 80M | — | 932 → 1 128 |

The scale harness at 4096² on the testbox (12 threads, run back to back): light 123 → 61 thread-seconds,
render 23.4 → 17.7 s wall, store time 1 179 → 1 402 columns/s, peak working set 1 550 → 1 443 MB.

Extracting a column's light allocated 49 boxes; it now refills the worker's (light bench 49 → 0.1
allocations per column, full bench 233k → 183k allocations, peak RSS 113 → 105 MB). What the light pass
does, in profile order (`light/engine.rs`):

- One 16-bit cell per block: dampening class (Java 26.3 dampens by 0, 1 or 15 only), six full-face bits,
  sky and block level, so a BFS step loads one word and the volume is 2 bytes a block. At 12 threads the
  pass was memory-bound (one thread 4.1k columns/s, twelve 9.2k with 32-bit cells; 14.7k with 16-bit).
- The volume has a one-cell opaque border, so the bucketed BFS needs no coordinates (three divisions a
  pop before) and no bounds tests; the six steps are unrolled; level-1 cells are not queued.
- fill writes every cell exactly once (no clearing), with the `Blocks` match outside the per-cell loop.
- Sky light's straight-down pass walks layer by layer from the top (column by column it touched a cache
  line a step) and stops once every column is shut.

Tried and dropped: skipping uniform opaque sections (−0.3% instructions: deep sections carry ores, so
they are never uniform); cropping the volume to the 14 blocks of the outer columns that light can cross
(−3% per column, +5% for 8×8 groups); filling and walking sky light in one top-down pass (+20%
instructions: the separate walk stops at the surface).

## Scale

`cargo run --release -p acacia-anvil --example scale -- side=<chunks> [key=value…]` (keys and defaults in
`examples/scale/main.rs`): 8 bots each mow an equal band of a `side`² world (passes along x, 15 chunks apart,
a view radius of 8, 1.4 chunks/s ≈ 22 blocks/s, so 17 new columns a step), relog every 15 min (whole view
sent again), and players edit 0.5 chunks/s; time runs 4× real time, the store's durations scaled to match.
Synthetic terrain from 16 templates (cliffs between chunks, ores, torches, edge redstone, snow, a lit cave
at a chunk edge, ponds), ~4.4 KB a chunk compressed. `check=<n>` converts n sample regions again with
everything in memory and compares every chunk's NBT.

Results (2026-10-24, 22 threads, memory limit 1 GB, `region_interval` 10 s, `region_hold` 60 s, as
`examples/crawl.rs` sets them; "store time" is `update` + `flush`):

| World | Chunks | Peak working set | Store time | Written per chunk | Bytes written / final world |
|---|---|---|---|---|---|
| 4096² | 65 536 | 1 424 MB | 2 155 columns/s | 9.2 KB | 2.1× |
| 8192² | 262 144 | 1 436 MB | 2 270 columns/s | 12.8 KB | 2.9× |
| 16384² | 1 048 576 | 1 427 MB | 2 143 columns/s | 11.4 KB | 2.6× |
| 8192², before (`region_interval` 2 s, no hold) | 262 144 | 1 371 MB | 615 columns/s | 155 KB | 35× |

- Memory is flat past the limit: the column heap stays at the bound (the working set is ~1.4× it:
  allocator and map overhead), and the bookkeeping is bounded: presence bitmaps for at most 16 384 regions
  (2 MB, rebuilt from the offset table when forgotten), debounce times only for regions touched within
  `region_interval`. At 300k² (~341k regions) nothing else grows.
- Where store time goes at 16384² (1.26M conversions, 317k columns read back): render 191 s wall (light
  1 437, encode 567, compress 774 thread-seconds), update (conversion) 142 s, read-back 82 s (22 s of it
  decompress + decode on all workers, the rest serial file reads), fixups 40 s, region reads before a
  rewrite 6 s, saves 14 s. The crawl needs ~200 columns/s for 8 bots, so the store runs at ~10% load.
- Write amplification came from rewriting whole regions every 2 s while a bot crossed them (each pass takes
  ~35 s per region): 35× the final world. A region now waits until nothing in it changed for
  `region_interval` (the bot has moved on), or `region_hold` at most, so a pass writes it about once.
  Whole-file temp + rename stays: bluemap-rs reads the header once and then sectors from its open handle,
  so in-place slot updates could hand it a torn chunk.
- A column sent again unchanged while held (relog, overlapping views) dirties nothing; one evicted is read
  back and relit with its neighbours, and its slot is kept when the NBT is the same (`relit unchanged`).
- Correctness across eviction: 8192² with a 128 MB limit (116k columns read back) and `check=8`: 8 192 of
  8 192 chunks in 8 sample regions byte-identical to the all-in-memory conversion. bluemap-rs renders 5 of
  its regions (1 186 hires tiles) with no warning. A limit far below the crawl's working set is not
  reached: dirty columns and their neighbours are never evicted (196 MB held at 128 MB, 669 MB peak).
