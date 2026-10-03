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
  (carpets, rails, torches, buttons, snow layers, open fence gates) or crossed planes. Signs, banners,
  vines and heads draw nothing yet.
- **Orientation**: `pillar_axis` and `minecraft:cardinal_direction` (blocks.json fronts face south).
- **Tint**: which faces take grass, foliage or water colour is by name (birch and spruce leaves are
  fixed colours). Water opacity is `water_surface_transparency` (0.65), not texture alpha.

## Biomes (`biome.rs`)

`BiomeColors::build` takes the server's `BiomeDefinitionList` (id, name, temperature, downfall): grass and
foliage sample `textures/colormap/{grass,foliage}.png` at (1 − temperature, 1 − downfall·temperature),
then vanilla's overrides (swamp, mangrove swamp, badlands, cherry grove, pale garden, dark forest);
water comes from `biomes_client.json`. Until it arrives, and for chunks without biomes, everything
tints like plains. The mesher averages the tint over the 3×3 columns around each block, so greedy
quads merge only within a uniform colour.

## Meshing (`mesh/`)

- A job copies the section plus a 1-block border (18³ ids, `Volume::gather`), so meshing takes no locks.
  Unloaded neighbour columns and sections not yet received (`Chunk::section_known`) read as
  occluders; the scene remeshes the edge when they arrive. Read as air, they drew water walls at chunk
  borders.
- Full cubes are greedy-meshed per face and slice; faces merge only with identical texture, tint,
  material and AO. AO looks at the 3 neighbours of each corner; quads split along the brighter diagonal.
- Other shapes emit one quad per visible face. Liquids draw their surface at `fluid_height`, full height
  under the same liquid.
- Quads are 12 bytes (`mesh/quad.rs`, tint colour as 7-bit sRGB per channel), pulled by vertex index in
  `gpu/terrain.wgsl`: no vertex or index buffers. UVs are world-aligned and tile per block through the repeat sampler, so merged quads need no
  atlas math.

## Lighting (`light/`)

Bedrock sends no light, so the client computes it. Emission and filter per state come from
`acacia-world` (`BlockState::light_emission`, `light_filter`; waterlogged blocks take the stronger of
both layers).

- **Thread**: `Lighting::new(world)` starts a light thread that subscribes to world changes. Column and
  section changes relight the whole column, once per batch. Block changes update incrementally. Then
  it forwards the change and every section whose bordered light changed (`LightEvent`), so the scene
  meshes only lit sections.
- **Data** (`LightData`, `parking_lot::RwLock`): a byte per block for light (block << 4 | sky) and for
  props (emission << 4 | filter), uniform sections stored as one value. Unloaded columns are dropped
  by polling `World::get`.
- **Propagation** (`propagate.rs`): BFS per channel. A step loses 1 + the entered block's filter; sky
  light at 15 going down loses only the filter. Removal clears what may have come from the old light,
  then refills from the brighter cells it met. Sky enters the top layer. Sky columns fill straight down
  first and spread only from their ends and their sides that face shorter columns or the chunk edge.
  There's no sky light in the nether and the end; they get a higher ambient instead (`gpu/mod.rs`
  `AMBIENT`).
- **GPU**: each section slot has an 18³ light volume (5832 bytes, `OPAQUE_CELL` marks filter-15 blocks).
  `gpu/light.wgsl` smooths per fragment: each face corner averages the 4 cells in front of it (without
  opaque ones, and without the diagonal when both edges are opaque), interpolated across the face.
  Cross planes and faces whose front cell is opaque use their own cell. Brightness is Java's curve at
  50% gamma, `max(block, sky)`, raised by the ambient. Quads keep their AO and directional shade.
- **Light-only updates**: when a section's light changes but its blocks don't, a light job re-gathers
  the volume into its slot without remeshing. Volumes carry the light generation, and the store keeps
  the newest.

## Scene and GPU

- `scene.rs` reads the light thread's events, marks affected sections (a block change dirties the sections
  of its 3×3×3 neighbourhood) and feeds the worker pool nearest first, sections with unloaded neighbours
  last. Light changes queue light-only jobs. One job per section is in flight at a time, and stale mesh
  results are dropped by version. Unloaded columns are found by polling `World::get`.
- `gpu/store.rs`: all quads live in one storage buffer (first-fit free list, grows by doubling); section
  origins and light volumes live in buffers indexed by slot, passed as the instance index.
- Cave culling (`cull.rs`, Java's `SectionOcclusionGraph`): meshing records which of a section's
  face pairs see each other through non-occluding blocks (`mesh/visibility.rs`, 15 bits). Each frame a
  BFS from the camera's section enters a neighbour only through a face its entry face sees, never moves
  back towards the camera, and stays in the frustum. Unmeshed sections count as open.
  `Renderer::cave_culling` turns it off (C in the viewer).
- Frames: frustum culling per section, solid pass front to back, then entities, then the translucent
  pass back to front with blending and no depth writes. Reverse-Z with an infinite far plane,
  camera-relative coordinates.
- Translucent order inside a section: back faces are culled and `Quad::blend_order` sorts the quads per
  face direction, far plane first, so every direction that draws blends back to front (water under ice,
  then the ice). Liquid surfaces emit their own underside. Quads of different directions still blend
  in key order. Unsorted, water drew over the ice above it everywhere but at section borders.

## Entities (`entity/`, `gpu/entities.rs`)

`EntityModels::load(pack)` reads the client entity definitions (`entity/*.entity.json`; the newest
`min_engine_version` of a kind wins) and bakes the geometry each names (`models/`) to a rest-pose
triangle mesh. The renderer draws the `EntityInstance`s given to `Renderer::set_entities`; the caller
maps its entities to models with `EntityModels::lookup` and `player`.

- **Geometry** (`geometry.rs`): both file layouts. With inheritance (`geometry.a:geometry.b`) a child
  bone of the same name adds its cubes and overrides the keys it sets, or replaces the bone with
  `"reset": true`. `bind_pose_rotation` turns only the bone's own cubes; `rotation` also carries its
  children. A cube without a pivot turns around its centre. `poly_mesh` bones (persona skins) are
  fan-triangulated.
- **Model space**: 1/16 block, feet at the origin, facing -z, the entity's right at -x (left-handed
  against the world). Angles in files are degrees, x and z clockwise: `Rz(-z) · Ry(y) · Rx(-x)`.
  Placement mirrors z, then turns by the yaw.
- **Box UV** (`bake.rs`): the unfolded box of Java's `ModelBox`; `mirror` flips u and swaps the sides.
- **Pose**: none but the head, which turns around the head bone's pivot by head yaw and pitch
  (vertices under a bone named `head` carry a part flag; two matrices per instance).
- **Textures**: `default`, or for villagers `base` + `plains` + `unskilled` laid over each other. One
  GPU texture per model or skin. TGA alpha is a tint mask, so those load opaque; otherwise texels
  under 10% alpha are cut out.
- **Players** (`skin.rs`): a classic skin is a texture for one of three humanoids (wide, slim, 64×32
  layout). A persona skin brings its own geometry and a separate face texture; both bake into one mesh
  over one stacked texture.
- **Light**: block and sky level at the entity's position from `LightData`, through the terrain's
  curve (`gpu/globals.wgsl`), times a directional shade from the normal.

## Not yet

Day/night (sky light is always full), GPU occlusion culling (Hi-Z), texture animation,
flow-direction water and sloped liquid surfaces, block entities, UI. Entities: animation (limbs,
setup poses some old models rely on), render controllers (variants, part visibility: villagers show
their hat brim, sheep wool has no colour), babies' own proportions where the pack has no baby
geometry, dropped items, name tags, armour and held items, capes.

Approximate: water loses 2 light per block (the wiki's Bedrock opacity note; its table is ambiguous),
so seabeds deeper than ~7 blocks go dark. Each section change relights its whole column.
