# Parity harnesses: the Java look against Java Edition

The Java look must draw what Minecraft: Java Edition 26.3 draws. A parity harness proves one piece
of it: a **Java oracle** runs the game's own code headlessly and writes what it computes (geometry,
light, colours, draw calls, layouts), a **Rust probe** computes the same from acacia, and a
**comparison** names every divergence as case, field, expected (the game) and got (acacia).
Screenshots are not evidence. Adapted from [Horizon](https://github.com/ludovic111/horizon)'s
oracle workflow (THIRD-PARTY.md).

## Running

```sh
tools/parity/remote.sh             # from Windows: syncs the worktree to devbox, runs every harness there
tools/parity/remote.sh bake        # one harness (several: bake light)
tools/parity/remote.sh --list      # harnesses, owners
python3 tools/parity/parity.py bake   # on devbox itself, in a checkout
```

Options: `--fresh` reruns cached oracle runs, `--detail N` shows N divergences per harness (all
of them are in `~/java-truth/runs/<name>/divergences.tsv`, the log in `log.txt`). The exit code is
1 if any harness diverges or fails. `remote.sh` runs in `work/<worktree folder>` on devbox; long
runs: `DETACH=1` (see `tools/remote-run.sh`).

## The oracle home

`python3 tools/parity/setup.py` builds it on a Linux x86-64 host (devbox has it at `~/java-truth`;
`JAVA_TRUTH` overrides): the pinned Temurin JDK 25, the 26.3 client and server jars and the
client's libraries from Mojang's version manifest (every file SHA-checked against
`tools/parity/lock.json` and the manifest), the server bundle unpacked, and the client
decompiled by Vineflower into `sources/client/` for reading (26.3 ships unobfuscated, so there is
no remapping). Linux x86-64 matters: `Math.sin` and the like round as HotSpot does there.
Nothing from it is committed: no game code, decompiled source, assets or dumps.

Oracle classes and runs are cached under `~/java-truth/cache/`: classes by the content of every
Java source compiled, a run by its classes, arguments and input files. A Rust-only change reruns
only the probe.

## The shared bootstrap (`tools/parity/java/acacia/parity/`)

- `Game.init()`: version, `Bootstrap` and `ClientBootstrap`, the client jar's vanilla pack as a
  `ResourceManager` (`Game.resources()`, no window, no GPU, no asset index), the grass, foliage and
  dry foliage colour maps; `Game.world()` vanilla's world registries; `Game.blockColors()`.
- `Data.get()`: the vanilla data pack loaded as a server loads it (world registries with tags,
  loot, default components): the registries a client receives.
- `Atlases.manager()`: every atlas the client knows over `UnitAtlas` (sprites from the
  `atlases/*.json` sources, real images and metadata, each alone in a unit atlas: UVs are
  sprite-local in [0,1]). Gui sprites name themselves: a blit's texture reads
  `minecraft:textures/atlas/gui.png#minecraft:hud/heart/full`.
- `Models.get()`: the client's full `ModelManager` reload (block states, items, entity models,
  fluids) over those atlases; `blockStateModel(state)`, `.manager`, `.entityModels`.
- `Fonts.font()`: the default font on a real `FontSet` (space and default providers, the game's
  metrics, bounds, missing and white glyphs); unifont is not in the jar, its codepoints come out
  missing.
- `Client`: a `Minecraft` shell as `Minecraft.getInstance()` (real `Options`, window size and GUI
  scale via `Client.window`, font, atlases, models, textures, `Gui`/`Hud`). `overworld()` /
  `level(...)` a real `ClientLevel`; `setBlock`, `setBiome`, `chunk`; `spawn(level, type, x, y, z)`;
  `entityRenderers()` the real `EntityRenderDispatcher`, so
  `Client.entityRenderers().extractEntity(entity, partial)` is the game's render state.
- `View`: `player(level, x, y, z, yRot, xRot)` a real `LocalPlayer`; `camera(partial)` the main
  camera aligned to it (fov, fluid, attribute probe); `lightmap(partial)` the
  `LightmapRenderState` the lightmap shader takes; `fog(camera, partial)` `FogRenderer.setupFog`.
- `Gui`: a real `GuiGraphicsExtractor` into a `GuiRenderState`. `Gui.record(mx, my, g -> ...)`
  lists elements, text (styled runs, laid-out glyphs), items and picture-in-picture states;
  `Gui.frame(mx, my, g -> ...)` adds `draws`: what `GuiRenderer` draws, in its order (items as
  blits, text as glyphs, sorted by its comparator, `afterBlur`).
- `Dump.of(x)`: any render state, record or game value as a JSON tree (registry names, joml as
  float arrays, formatted text as runs). `Shell`: Unsafe allocation and reflective access for
  shells. `Out`: JSON and JSON-lines writers, `stateKey`
  (`minecraft:oak_slab[type=double,waterlogged=false]`, properties sorted), colour hex.

What is not here stays null on the shells: set it with `Shell.set` in your oracle, then fold it
into the bootstrap (tell the oracle owner). Harness `bootstrap` runs each piece once; its oracle
is the shortest example of each.

## Adding a harness

1. `tools/parity/harnesses/<name>/`: one directory per harness.
2. The oracle, `<Name>Oracle.java` in package `acacia.parity.<name>` (MIT, ours or adapted from
   Horizon's `mods/horizon/src/test/java/dev/horizonlab/parity`). `main(String[] args)`:
   `args[0]` is the directory to write into, the rest are the harness's arguments. Start from
   `Game.init()`. Prefer one JSON line per case with a `case` key (`Out.Lines`).
3. The probe: a CPU-side example or binary in the crate it tests (no GPU; a rule that lives only
   in WGSL moves CPU-side first). It reads the same inputs and writes the same records.
4. `harness.py`, with a one-line docstring, `OWNER` (the area) and `run(h)`:

   ```python
   def run(h):
       expected = h.oracle("acacia.parity.light.LightOracle", "--cases", 500)  # out dir
       h.probe("acacia-render", "light_probe", expected / "cases.jsonl", h.out / "rust.jsonl")
       h.compare_jsonl(expected / "records.jsonl", h.out / "rust.jsonl", key="case", tolerance=1e-4)
   ```

   `h.compare_jsonl` matches records by key and compares them field by field (numbers within a
   relative tolerance). A probe that compares by itself prints `CASES\t<n>` and one
   `DIVERGENCE\t<case>\t<field>\t<expected>\t<got>` line per divergence; pass its output to
   `h.read_probe` (harness `bake` does). `h.diverge(...)` and `h.add(cases, list)` take
   divergences computed in Python.
5. Run it, then add or update its row below when you merge. A general piece of headless game
   (a subsystem the bootstrap lacks) goes in your harness first; tell the oracle owner to fold it in.

## Harnesses

Cases and divergences as of the last merge that touched the harness.

| Harness | What | Cases | Divergences | Owner |
|---|---|---|---|---|
| `bake` | every block state's baked quads (corners, texture, UVs, cull, tint, shade direction, AO) and alternatives (35,723 states, 644 with alternatives picked at 48 positions), off-grid shifts (39 blocks), biome colours (67 biomes, grass at 64 columns), AO shade (22,091 mapped Bedrock states); probe `lookbake java-check` | 57,920 | 0 | lookbake |
| `weather` | rain/snow columns and quads from `extractRenderState` over random scenes (400: heights, biomes, unloaded chunks, light), height-adjusted temperature and precipitation per biome (4,000 positions), `tickWeatherEffects` splashes and rain sound over 8 ticks (400 scenes, 21 ground blocks, all particle options), lightning bolt vertices (400 seeds); probe `weather_probe` | 5,200 | 0 | atmos (weather) |
| `sky` | every rendered environment attribute (sky, fog, cloud, sunrise colours; sun, moon, star angles and brightness; moon phase; fog, sky, cloud and water fog distances; light attributes; sky light level) from a headless `ClientLevel`'s own `EnvironmentAttributeSystem` over Horizon's grid (3 dimensions × 195 times × weathers, every biome per dimension at 4 times), weighted biome blends, `GaussianSampler` blends and lightning flashes; `SkyRenderer`'s vertex buffers recorded off its GPU device (sky and dark discs, stars, sunrise fan, sun quad, 8 moon phases, End box) and its sun, moon, star, sunrise and dark-disc frames over 10 angles; probe `sky_probe` | 1,949 | 0 | atmos (sky) |
| `fog` | `FogRenderer.setupFog` (colour and every `FogData` distance) for a headless camera and player: atmospheric over 3 dimensions, 9 biomes, 7 times, 4 view directions, 4 render distances and storm; rain fog easing (sky light, precipitation, delta ticks); void darkness (flat or not); cloud range; boss darkening and world fog; water (5 biomes, water vision), lava and powder snow (spectator, fire resistance); blindness, darkness blend and night vision durations; probe `fog_probe` | 1,370 | 0 | atmos (fog) |
| `clouds` | cloud-map cells and empty neighbours (vanilla and 4 made-up maps), per frame `CloudInfo` (colour, offset, cell size) and fancy/fast faces in build order (120 frames, ranges 2–128); probe `clouds_probe` | 125 | 0 | atmos (clouds) |
| `bootstrap` | the shared bootstrap runs headless: level and blocks, entity render states (pig, zombie, armor stand, item frame, creeper), GUI records and frame order, lightmap and fog; oracle only | 10 | 0 | parity oracle |
| `entity_models` | every entity model layer the client bakes (361), at rest under `LivingEntityRenderer`'s root transform: each part's pose, visibility and `skipDraw`, each drawn corner's position, normal and UV, against the look's `java/entity_models.json` drawn as the entity pass draws it; probe `acacia-render --example java_entities` | 361 | 0 | acacia-render entity |
| `entity_poses` | `setupAnim` of the 23 layers the look draws (cows, pigs, sheep, chickens, creeper, spiders, zombies, husks, drowned, skeletons, babies) on walks, looks, ages, swings with either hand, aggression, held items, every arm pose but `SPEAR`, crouching, riding, gliding, swimming, grazing, flapping: parts and drawn corners; same probe | 6,048 | 0 | acacia-render entity |
| `lightmap` | the 16x16 lightmap: the real client's `LightmapRenderState` (a `ClientLevel` per dimension with its clocks, rain, thunder, lightning and End flash set; a `LocalPlayer` with night vision and darkness; gamma, darkness scale, boss darkening, flicker) through `LightmapRenderStateExtractor`, and `lightmap.fsh` transliterated to Java floats over it (the game runs it on the GPU); 3 dimensions × 14 times × 6 weathers × lightning, and 15 player settings × 4 skies; state fields and texture bytes; probe `acacia-render --example lightmap_probe` | 564 | 0 | acacia-render light |
| `smooth_light` | the client's `BlockModelLighter` over 1,500 random 5³ scenes (55 states: cubes, slabs, stairs, partial models, glass, leaves, emitters, darkening blocks; random light; Nether or not), in `ModelBlockRenderer`'s loop: each drawn quad's (14,639, 6,590 not uniform) per-vertex light coords and grey, against `mesh::smooth::Lighter` from the oracle's cell facts; probe `smooth_light_probe` | 1,500 | 0 | acacia-render light |
| `light_engine` | the game's `LevelLightEngine` over 160 generated 2-chunk, 32-high layouts (stone floor, roofs, 50 states: liquids, slabs, stairs, snow, glass, leaves, emitters) with sky or not, then 0–11 block edits each: block and sky level of every cell (5,242,880), against `light::column::Lighter` under Java rules from the oracle's light facts; probe `light_engine_probe` | 160 | 6 | acacia-render light |
| `light_facts` | every mapped Bedrock state's light facts in the Java look (emission, dampening, solid render, full collision, emissive, AO darkening, the 16x16 face occlusion shapes the light engine reads) against the game's state; probe `lookbake java-light-check` | 22,091 | 0 | acacia-render light |
| `entity_layers` | held items: the matrix `ItemInHandLayer` hands each arm's item (the model's `translateToHand`, skeletons' 1 px shift included; the layer's turn and adult or baby fist offset, transcribed in the oracle) on the armed models of the `entity_poses` corpus, against `Mesh::hand` and `item::hand::fist`; same probe | 4,800 | 0 | acacia-render entity |
