# Java look: what is left

State on 2026-10-05. The plan and its decisions are in [java-look.md](java-look.md); this is the
work list. J0 (look packs) and J1 (Java blocks) are on master. J2 to J5 are not started.

## Where J1 stands

`lookbake java` bakes a pack the viewer draws with `--look java`. `lookbake java-check` compares
it with the game's own output (java-look.md "Checking against the game"), and all of it agrees:

| What | Compared |
|---|---|
| Baked faces of every Java block state, every weighted alternative | 35,723 states |
| The alternative picked at 48 positions | 644 states |
| Shift of the blocks that stand off the grid | 39 blocks |
| Grass, foliage, dry foliage and water per biome | 67 biomes |
| Which states darken ambient occlusion | 22,091 mapped Bedrock states |

What the dump cannot cover is how the faces are drawn. That is the first item below.

## 1. Drawing parity (renderer, `look` area)

Each is checked against the decompiled 26.3 client, not by eye. None is started.

- **Light curve.** `globals.wgsl`'s `curve` is the renderer's own. Java's is its lightmap
  (`LightmapRenderStateExtractor`, `lightmap.fsh`): block and sky level to brightness, with the
  dimension's ambient light and the brightness option. Make the curve the look's.
- **Light smoothing.** Java blends the light of four cells per face corner
  (`BlockModelLighter`, `LightCoordsUtil.smoothBlend`). We sample the light volume per fragment.
  Decide whether that is close enough or whether corners carry light.
- **Ambient occlusion detail.** The strengths and the darkening blocks are Java's. Not compared:
  the centre cell's share, the rule that takes a corner for dark when both its sides block light,
  and the weights Java gives a face that does not reach the block's edge (`doNonCubicWeight`); we
  interpolate the corner values instead. A dump of the game's per-vertex shade for a set of
  neighbourhoods would settle all three.
- **Liquids.** Geometry is the renderer's, after an older `LiquidBlockRenderer`. Compare corner
  heights, the flow angle and the side faces with 26.3's `FluidRenderer`.
- **Fog.** Both bands are drawn, in the renderer's sky colour. Java's is `fog_color` mixed with
  the sky colour by render distance (`AtmosphericFogEnvironment.getBaseColor`), and rain pulls the
  haze in. The Nether's haze is chosen by the dimension's height, since `Dimension` has no name.
  The haze has only been through shader validation; look at it once.
- **Grass block and the other overlaid models** (182 states) keep Bedrock's overlay cube and
  textures. Drawing Java's two layers needs a second face on the same quad without depth
  fighting.
- **States with no Java geometry** (193) and **block entities** (382: chests, signs, beds,
  banners, heads) keep their Bedrock rendering. The second group belongs with J4.
- **Biomes.** Where a biome is comes from the Bedrock server, so only colours are Java's.
  `dappled_forest` and the End's outer biomes have no Bedrock name; a custom Bedrock biome falls
  back to the colormaps.
- **Cost.** A block with alternatives rarely merges with its neighbours, so stone, dirt and sand
  cost one quad per visible face. Measure a far render distance before deciding whether it needs
  a setting.

## 2. J2: UI foundation — done (2026-10-08)

Decided: a GPU-free `acacia-ui` crate lays widgets out into draw lists that `acacia-render`'s UI
pass draws (crates/acacia-ui/README.md). Each theme takes its own game's art; Bedrock's pack has
no font, so it borrows the baked Java look's. HUD, chat (with translations), debug screen, GUI
scale, block-item icons. Before it, the viewer became playable (P0): first-person control,
targeting, mining, placing, F5. Checked by `tools/testbox-shot.sh` (Xvfb + lavapipe on testbox)
and the live actions `mine_held`, `mine_creative` (`MOUSE=1`), `place_aimed`.

Left from the milestone: nine-slice sprites, Unicode glyph pages, the boss bar, titles.

## 3. After that

- **J3 menus:** started: the player's inventory (E) and 9-wide containers, with Java's click
  rules (`Bot::click_slot`); the crafting grid and the portrait are not wired. Left: pause and
  options (with the look option and the first-use bake), workstations and other
  containers, boss bar. Server forms are done in both looks (acacia-ui `form/`, `widget/`), shown
  live from `tools/formtest-pack`; `ACACIA_FORM=tools/forms/<kind>.json` shows one for screenshots.
- **J4 Java entities:** models and animations per mob, most seen first, Bedrock geometry as the
  fallback. Block entities from item 1 go here.
- **J5 Bedrock theme:** the second `Theme`.

## Working on it

- Ground truth: `bash research/java-truth/run.sh` (workspace folder, outside the repo) writes the
  dumps in about a minute. Its `decomp/src` holds only the classes decompiled so far; unpack
  more from the client jar and run `tools/vineflower.jar` over them as needed.
- `lookbake java-check research/java-truth/quads.json.gz [java dir]` reads the dumps beside it.
- Look packs are version 8. One baked before that is refused: bake again.
- The Bedrock pack is fetched by `tools/fetch-vanilla-pack.sh` into the main checkout's
  `assets/vanilla`; worktrees have none.
- The test world's scene near the bot (-129, 64, -19) has leftover test blocks.
