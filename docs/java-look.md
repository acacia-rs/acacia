# Looks and the Java look

A setting that makes the client render and present itself like Minecraft Java Edition: Java block
models and textures, Java entity models, Java-style HUD and menus. Plan only; nothing here is built.

Surveyed 2026-10-04 against master `256e8fe` and PommeMC/Client `0fcbc20`.

## Design

The renderer knows nothing about Java. It loads a **look pack**: baked data in one format, whatever
edition it came from. A separate **bake tool** turns an edition's assets into a look pack on the
user's machine. Switching looks is loading another pack and remeshing.

```
Java client jar ──┐
                  ├─ tools/lookbake ──► assets/looks/<name>/ ──► acacia-render ──► screen
Bedrock pack ─────┘   (separate binary)   (look pack, data)        (MIT, one loader)
```

## Decisions

| Area | Choice | Why |
|---|---|---|
| Assets | Real Java Edition assets, not Java styling over the Bedrock pack | The look people prefer includes Java's models, GUI sprites and font |
| Obtaining them | `lookbake fetch-java` downloads the pinned client jar (Java 26.3, the release of Bedrock 26.50's drop) from Mojang's version manifest into `assets/java`, checks its SHA-1 and unpacks `assets/minecraft` | Nothing redistributed; the version matches the world. A local `.minecraft` is often older than the Bedrock world |
| Packaging | Generic renderer plus a baked look pack, not Java loaders linked into the client | One load path, the client stays MIT, custom Java resource packs work by re-baking |
| Distribution | Packs are baked locally, never uploaded or shipped | A baked pack contains Mojang's assets |
| Entities | Java models and animations, expressed in the geometry and animation format the renderer already draws | No second entity pipeline. Mobs not yet converted keep the Bedrock geometry |
| UI | Code, not data: a `Theme` interface, Java theme first, Bedrock theme later | Data-driven layout means rebuilding Bedrock's JSON UI for two themes |
| Lighting | Parameters in the pack, no pluggable lighting engine | The editions differ in a few numbers, not in the algorithm |
| Switching | Live: load the pack, rebuild the block table, remesh | `Renderer::set_world` and `set_biomes` already do this |
| Default | Bedrock until Java blocks, HUD and the options menu are done; then decide | A half-Java look is worse than either |

## Licensing

Pomme (PommeMC/Client) is GPL-3.0-or-later; acacia is MIT.

- Code ported or translated from Pomme lives only in `tools/lookbake`, marked
  `license = "GPL-3.0-or-later"` with Pomme's notice.
- `lookbake` is a separate program. No crate depends on it and it is not linked into any binary; the
  client reads only the data it writes.
- Every crate under `crates/` stays MIT and may be written with Pomme open as a reference for
  behaviour, never by translating its code. This includes shaders.
- DESIGN.md's License row ("never port from GPL/LGPL sources") gains this one exception when
  `lookbake` lands.

Open: entity cube tables transcribed from Pomme's source would carry its text into the pack. Take
them from the Java models directly, or confirm that is acceptable, before J4.

## Crates

```
acacia-viewer            (app) settings, persistence, look switch, runs the bake on first use
 ├─ acacia-ui            MIT  2D pass, element list, GUI scale, text drawing, Theme trait, Java theme
 └─ acacia-render        MIT  look pack loader, Look parameters, baked-quad mesh path
tools/lookbake           GPL  (bin) jar download, blockstates, model baking, .mcmeta, colormaps,
                              font providers, state mapping; writes a look pack. Not a dependency of anything
```

## Look pack contents

| Part | Holds | Replaces today's |
|---|---|---|
| Blocks | Per Bedrock block state: baked quads (or "full cube" plus six layers), cull faces, tint kind, render layer | `blocks.json`, `terrain_texture.json`, built-in shapes |
| Textures | 16×16 layers for the texture array, with animation frames and timing | `flipbook_textures.json` |
| Tints | Colormaps, per-biome water colours, overrides | `biomes_client.json`, constants in `biome.rs` and `blocks/tint.rs` |
| Entities | Geometry, animations, render controllers, textures per Bedrock entity identifier | Same files, read from the Bedrock pack |
| Block models | Chests, beds, signs, heads | `entity/block_models.json` |
| Font | Glyph atlas and metrics | Nothing yet |
| GUI | Sprites with nine-slice metadata | Nothing yet |
| Parameters | The table under "Parameters" | Shader literals |

The format is ours and versioned; the renderer refuses a pack baked for another version and the
viewer re-bakes. The Bedrock look goes through the same format (see Milestones for where its baker lives).

## Parameters

The renderer already follows Java for face shading, the brightness curve and gamma, smooth lighting,
night darkening, liquid geometry and the 70° FOV. The pack sets the rest through the `Globals`
uniform (`gpu/globals.rs`, `gpu/globals.wgsl`):

| Item | Bedrock look | Java look | Applies |
|---|---|---|---|
| Fog | Sphere, smoothstep from 0.7×end | Cylinder, linear, two bands | Draw time |
| Water opacity | Fixed 0.65 | Texture alpha | Draw time |
| Water light filter | Bedrock opacity | One level per block | Relight |
| Cutout threshold | 0.5 | 0.1, or 0.5 on mipped cutout | Draw time |
| Mip levels | Full chain | Capped at 4 | Atlas rebuild |
| Biome blend | 3×3 | 5×5 | Remesh |

Sun, moon and stars are already in `acacia-render` and serve both looks.

## What Pomme gives us

| Piece | Pomme source | Use |
|---|---|---|
| Block and item model baking | `world/block/model.rs` (2413 lines) | Port into `lookbake`. Add what it lacks: `uvlock`, weighted variants, multipart `OR`/`AND`, real `tintindex` |
| Font providers | `ui/font.rs` | Port into `lookbake`; the client gets a glyph atlas |
| `.mcmeta` animation, mip strategy | CPU half of `renderer/chunk/atlas.rs` | Port the parsing; keep our texture array |
| Colormap and tint maths, grass modifiers | inside `renderer/chunk/mesher.rs` | Port into `lookbake` |
| Asset key and asset-index lookup | `assets.rs` | Port into `lookbake` |
| Entity cubes and animations, 36 kinds | `renderer/entity_model.rs` (5421 lines) | Reference: its animations are Rust functions, ours are Molang files written by hand |
| Styled text layout, HUD and screen layouts | `ui/text.rs`, `ui/hud.rs`, `ui/chat.rs`, `ui/inventory.rs`, `ui/menu/*` | Reference for behaviour and pixel positions; `acacia-ui` is MIT and written by us |
| Clouds, fog, weather, hand, break overlay | `renderer/pipelines/*`, GLSL shaders | Reference; Vulkan, and the client is MIT |

Not in Pomme: the Bedrock-to-Java state mapping, most Java mobs, a lightmap, a selection outline,
GUI sprite `scaling` metadata, reading assets straight from the jar.

Pomme's files run to several thousand lines. Ports are split by responsibility to fit the 300-line rule.

## Bedrock state to Java model

The world is Bedrock block states; Java models are keyed by Java block name and properties. `lookbake`
resolves this once, at bake time, so the pack is keyed by Bedrock state.

The table is ViaBedrock's `blockstate_mappings.json` (GPL-3.0, so it is used only inside `lookbake`):
a native Bedrock-to-Java map covering all 22,091 states of the 26.50 palette with Java 26.3 targets.
GeyserMC's MIT `blocks.nbt` was the first candidate and is kept as a cross-check only: it maps Java
to Bedrock, and its inverse reaches 72% of the palette, misses real states (water depths, partial
redstone signals, most hanging signs) and gets 156 hanging-sign rotations wrong.

`lookbake java-report` measures the coverage: 22,058 of the 22,091 states reach a Java model
(2,437 distinct models) through the table and the jar's blockstate files. The other 33 are walls with
no post and no sides, which Java draws as nothing.

A Bedrock state whose Java target has no model is baked from the Bedrock pack into the same look
pack. Bed and banner colour, pot contents, skull rotation and chest pairing are in block entities
under either mapping; the renderer already reads those for beds, chests and heads.

The same applies to entity identifiers and items. Sources, counts and licences: the workspace's
`research/java-look-sources.md` (2026-10-04).

## Not covered by a pack

- **Server-sent Bedrock resource packs** have no Java form. In the Java look they are either ignored
  or baked over the Java assets; undecided.
- **Bedrock-only UI** (server forms, boss bar variants) has no Java original and gets Java styling
  designed by us.

## Milestones

| | Milestone | Contents | Checked by |
|---|---|---|---|
| J0 | Look pack, Bedrock only | Pack format and loader in `acacia-render`; `lookbake` baking the Bedrock pack; parameters in `Globals`; `Settings` with JSON persistence, `--look` flag and a key in the viewer | Screenshots identical to today's at fixed positions (`ACACIA_SCREENSHOT`, `ACACIA_AT`, `ACACIA_LOOK`); existing render tests |
| J1 | Java blocks | Jar download, Pomme ports, state mapping, baked-quad mesh path, Java parameters; DESIGN.md License row amended | Screenshots beside the Java client on the same seed; a count of Bedrock states baked from the fallback |
| J2 | UI foundation | `acacia-ui`: 2D pass after the world, GUI scale, sprites, nine-slice, text; `Theme` trait; HUD (hotbar, hearts, hunger, xp, crosshair), chat, debug overlay. Viewer forwards bot state through `NetEvent` | Screenshot per element at GUI scales 1 to 4 |
| J3 | Menus | Pause and options screens, including the look option itself and the first-use bake; inventory and containers; server forms and boss bar | Driven against the test BDS |
| J4 | Java entities | Geometry and Molang animations per mob, most-seen first; Bedrock geometry as the fallback | Side-by-side screenshots per mob; a list of mobs still on the fallback |
| J5 | Bedrock theme | Bedrock-style HUD and menus as the second `Theme` | As J2 and J3 |

J0 is done (crates/acacia-render/README.md "Look" and "Look pack"): `Look` (fog shape, water
opacity); the viewer's settings, `--look` and `L`; the pack format; `tools/lookbake bedrock`; the
viewer drawing from a saved pack with no resource pack present. Two things differ from the design above:

- Entities, tints and sky textures are carried as files in the Bedrock pack's layout, not baked
  into tables. A Java bake writes the same files.
- The Bedrock baker stays in `acacia-render` (`LookPack::bake_bedrock`), which `lookbake` calls. It
  is MIT, the renderer's tests are built on it, and it lets the viewer run with no bake step. Only
  code ported from Pomme has to live in the tool.

J0 is a refactor of `acacia-render`'s loaders with no visible change, and is the step that proves the
format. J1 so far: `lookbake java` writes `assets/looks/java`. It starts from the Bedrock look and replaces
every state whose Java model has geometry: 21,269 of 22,091 states (934 as greedy-meshed cubes, the
rest as model faces), with 1,171 texture layers after compaction. Kept on Bedrock rendering: 382
states drawn as block entities, 64 liquids, 193 with no Java geometry, and 183 whose model lays one
face over another (grass block sides under their overlay). Left in J1:

- Model faces take no ambient occlusion, weighted variants take the first, and `uvlock` re-projects
  the texture (exact only for faces textured by position).
- Tints are still the Bedrock look's, by Bedrock block name: Java-only tinted blocks (lily pads,
  stems, redstone wire) come out untinted, and water keeps Bedrock's colours and textures.
- Biome blend radius, mip cap, cutout threshold and the second fog band from "Parameters".
- `pack.json` is 36 MB for the Java look; model faces want a binary encoding.

J0 and J1 touch `acacia-render`; under the one-agent-per-area rule they are done by, or
handed over from, whoever holds the render area. J2 onward are new areas (`ui`, `entities`).

## Open items

- The jar lacks sounds, other languages and the Unifont glyphs; those come from Mojang's asset index.
- Pack encoding: JSON plus PNG strips for now (about 5 MB for Bedrock, 4 MB of it `pack.json`).
  Revisit if loading is slow or once baked quads make the table much larger.
- README "Non-goals" still lists graphics and Java Edition; DESIGN.md milestone 10 is out of date.
- Whether the Java look becomes the default once J3 is done.
