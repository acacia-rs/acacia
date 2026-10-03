# acacia-render

wgpu terrain renderer for `acacia-world`. Depends only on `acacia-world`, never on the bot or the network;
`acacia-viewer` wires it to a bot.

## Assets

`tools/fetch-vanilla-pack.sh` copies `blocks.json`, `terrain_texture.json`, `biomes_client.json` and the block
textures from Mojang/bedrock-samples into the git-ignored `assets/vanilla`. BDS ships no textures. Pin the tag
to the block palette version (`acacia-world/src/registry/mod.rs`).

- `blocks.json`: block name → texture name per face (`side` fills the horizontal faces).
- `terrain_texture.json`: texture name → image path. Arrays hold variants; the first is used (leaves list
  `[fancy, opaque]`). `overlay_color` marks a tint mask in alpha (grass block sides: alpha 255 = tinted).
  `quad: 1` frames hold 2×2 tiles (flowing water, lava).
- Images are normalized to 16×16 (first flipbook frame) in one `Rgba8UnormSrgb` texture array with mips.

## Block table (`blocks/`)

`BlockTable::build(registry, pack)` maps each runtime id to shape, render layer, texture layer, tint and
material per face. Build it from the **world's** registry: custom blocks shift runtime ids.

- **Shape**: vanilla shapes are hard-coded in the game, so full cubes use `Cube`, other solid blocks use
  their collision boxes (clamped to the block, rounded to 1/16), and collisionless blocks use a small table
  (carpets, rails, torches, buttons) or crossed planes. Signs, banners, vines and heads draw nothing yet.
- **Orientation**: `pillar_axis` and `minecraft:cardinal_direction` (blocks.json fronts face south).
- **Tint**: grass, foliage and water by name, plains colours until biomes are decoded. Water opacity is
  `water_surface_transparency` (0.65), not texture alpha.

## Meshing (`mesh/`)

- A job copies the section plus a 1-block border (18³ ids, `Volume::gather`), so meshing takes no locks.
  Unloaded neighbour columns read as occluders; the scene remeshes the edge when they arrive.
- Full cubes are greedy-meshed per face and slice; faces merge only with identical texture, tint,
  material and AO. AO looks at the 3 neighbours of each corner; quads split along the brighter diagonal.
- Other shapes emit one quad per visible face. Liquids draw their surface at `fluid_height`, full height
  under the same liquid.
- Quads are 12 bytes (`mesh/quad.rs`), pulled by vertex index in `gpu/terrain.wgsl`: no vertex or index
  buffers. UVs are world-aligned and tile per block through the repeat sampler, so merged quads need no
  atlas math.

## Scene and GPU

- `scene.rs` subscribes to world changes, marks affected sections (a block change dirties the sections
  of its 3×3×3 neighbourhood) and feeds the worker pool nearest first, sections with unloaded neighbours
  last. Stale results are dropped by version. Unloaded columns are found by polling `World::get`.
- `gpu/store.rs`: all quads live in one storage buffer (first-fit free list, grows by doubling); section
  origins live in a second buffer indexed by slot, passed as the instance index.
- Frames: frustum culling per section, solid pass front to back, translucent pass back to front with
  blending and no depth writes. Reverse-Z with an infinite far plane, camera-relative coordinates.

## Not yet

Biome colours, lighting (Bedrock sends none; caves are lit), cave/occlusion culling, texture animation,
flow-direction water and sloped liquid surfaces, entities, block entities, UI.
