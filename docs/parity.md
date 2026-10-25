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
- `BlockModels.get()`: every block state's baked model by the client's own loaders and
  `ModelBakery`, over `UnitAtlas` (each atlas's sprites from its `atlases/*.json` sources, real
  images and metadata, each alone in a unit atlas so UVs are sprite-local).
- `Out`: JSON and JSON-lines writers, `stateKey` (`minecraft:oak_slab[type=double,waterlogged=false]`,
  properties sorted), colour hex.

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
| `chat_screen` | the chat input (EditBox: typing, caret, selection, scrolling, word moves, history) and name completion (CommandSuggestions' list: Tab, cycling, clicks, hover); messages only, commands not yet; probe `gui-probe chat_screen` | 3,000 | 33 (non-ASCII advances: the probe's stand-in font) | gui |
| `bake` | every block state's baked quads (corners, texture, UVs, cull, tint, shade direction, AO) and alternatives (35,723 states, 644 with alternatives picked at 48 positions), off-grid shifts (39 blocks), biome colours (67 biomes, grass at 64 columns), AO shade (22,091 mapped Bedrock states); probe `lookbake java-check` | 57,920 | 0 | lookbake |
| `font` | the default, alt, uniform and illageralt fonts as `FontManager` loads them from the jar plus asset index 34 (unifont, filters uniform/jp): per-codepoint glyphs (advance, bold/shadow offset, texture, texels hash) over 4,096-codepoint ranges, `glyphsByWidth` order for obfuscated text, and 30,000 styled strings' widths, bounds and `prepareText` glyph quads (shadow, bold, italic, underline, strikethrough, obfuscated, legacy codes); left-to-right text only; probe `gui-probe font` | 34,932 | 0 | gui |
