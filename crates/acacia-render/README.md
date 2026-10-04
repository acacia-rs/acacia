# acacia-render

wgpu terrain renderer for `acacia-world`. Depends only on `acacia-world`, never on the bot or the network;
`acacia-viewer` wires it to a bot.

## Assets

`tools/fetch-vanilla-pack.sh` copies `blocks.json`, `terrain_texture.json`, `biomes_client.json`, the block,
entity and environment textures and the entity files from Mojang/bedrock-samples into the git-ignored `assets/vanilla`. BDS ships no textures. Pin the tag
to the block palette version (`acacia-world/src/registry/mod.rs`).

- `blocks.json`: block name → texture name per face (`side` fills the horizontal faces).
- `terrain_texture.json`: texture name → image path. Arrays hold variants; the first is used (leaves list
  `[fancy, opaque]`). `overlay_color` marks a tint mask in alpha (grass block sides: alpha 255 = tinted).
  `quad: 1` frames hold 2×2 tiles (flowing water, lava).
- Images are normalized to 16×16 in one `Rgba8UnormSrgb` texture array with mips.
- Flipbooks (`textures/flipbook_textures.json`) keep every frame of their strip. Each game tick the
  renderer rewrites the animated layers in place, cross-fading unless `blend_frames` is false. Only a
  texture's first variant is drawn, so only its flipbook plays; flowing liquids show one tile of their 2×2.

## Block table (`blocks/`)

`BlockTable::build(registry, pack)` maps each runtime id to shape, render layer, texture layer, tint and
material per face. Build it from the **world's** registry: custom blocks shift runtime ids.

- **Shape**: vanilla shapes are hard-coded in the game, so full cubes use `Cube`, other solid blocks use
  their collision boxes (clamped to the block, rounded to 1/16), and collisionless blocks use a small table
  (carpets, rails, torches, buttons, snow layers, open fence gates) or crossed planes. Chests, beds,
  signs and heads are models (see "Block models"). Hanging signs, banners and vines draw nothing yet.
- **Orientation**: `pillar_axis` and `minecraft:cardinal_direction` (blocks.json fronts face south).
- **Tint**: which faces take grass, foliage or water colour is by name (birch and spruce leaves are
  fixed colours). Water opacity comes from the look.

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
- Other shapes emit one quad per visible face.
- Liquids (`mesh/liquid.rs`, after Java's `LiquidBlockRenderer`): each block corner's height is the
  mean of the four cells sharing it (nearly full cells weigh 10×, open cells pull it down, solid ones
  don't count, full under the same liquid), so neighbouring surfaces meet. Liquid quads store the four
  heights and a texture turn instead of size and AO. A surface with a flow (the height drop towards
  each side, `FlowingFluid::getFlow`) shows the flowing texture turned downstream; at rest, the still one.
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
`min_engine_version` of a kind wins) and bakes every geometry they name (`models/`) to a rest-pose
triangle mesh. The renderer draws the `EntityInstance`s given to `Renderer::set_entities`; the caller
gets an entity's layers from `EntityModels::appearance` (or `player`).

- **Render controllers** (`controller.rs`, `render_controllers/`): each controller a definition lists
  is one `Layer`, drawn in order at the same depth: a mesh, up to three textures laid over each
  other, the bones it hides (`part_visibility`, a bit per bone in the instance) and a tint. Their
  expressions and the definition's `initialize`/`pre_animation`/`scale` scripts run per entity
  against the queries the caller answers (`is_baby`, `variant`, `mark_variant`...; unknown ones are 0).
  A kind whose controllers yield nothing draws its default geometry and texture.
- **Molang** (`molang.rs`): numbers, strings, resource names, operators, ternaries, `array.x[i]`
  (indices wrap), assignments, `math.*`. Loops, structs and `->` do not parse; such a script is
  skipped and its variables read 0. Colour expressions (`color`, `overlay_color`) and `uv_anim` are
  not evaluated.
- **Materials**: no material file is read. Layers whose material is a blended overlay (slime shell,
  charged creeper, enchantment glint: `OVERLAY_MATERIALS`) are dropped, since the pass is opaque. The
  `sheep` material tints by the `color` query where the texture's alpha is 0.
- **Geometry** (`geometry.rs`): both file layouts. A `minecraft:geometry` file without a texture size
  takes the size of the kind's texture; the old layout defaults to 64×32. With inheritance (`geometry.a:geometry.b`) a child
  bone of the same name adds its cubes and overrides the keys it sets, or replaces the bone with
  `"reset": true`. `bind_pose_rotation` turns only the bone's own cubes; `rotation` also carries its
  children. A cube without a pivot turns around its centre. `poly_mesh` bones (persona skins) are
  fan-triangulated.
- **Model space**: 1/16 block, feet at the origin, facing -z, the entity's right at -x (left-handed
  against the world). Angles in files are degrees, x and z clockwise: `Rz(-z) · Ry(y) · Rx(-x)`.
  Placement mirrors z, then turns by the yaw.
- **Box UV** (`bake.rs`): the unfolded box of Java's `ModelBox`; `mirror` flips u and swaps the sides.
- **Animation** (`animation.rs`, `pose.rs`): `EntityModels::pose` plays what the definition's
  `scripts.animate` lists (animations, and animation controllers nested up to 4 deep) into a `Pose`:
  per bone, a rotation, position and scale on top of the rest pose, each animation scaled by its blend
  weight. `this` in a channel is the bone's value so far, rest rotation included, so `target - this`
  sets an absolute angle. The caller answers the moving state (`life_time`, `modified_distance_moved`,
  `modified_move_speed`, `target_x_rotation`, `target_y_rotation`); kinds whose animations never touch
  `head` get the look angles on it directly. `Mesh::skin` turns a pose into one matrix per bone,
  uploaded per instance; vertices carry their bone index.
  - Nothing is remembered between frames. A controller is walked from its initial state through at
    most 4 transitions, so states entered by one-off events and `query.all_animations_finished`, blend
    times between states, and variables scripts accumulate are lost. `anim_time` is the entity's age
    unless `anim_time_update` gives it (walk cycles use the distance moved).
  - Keyframes interpolate linearly (`pre`/`post` honoured, no catmull-rom). `relative_to` is ignored.
    Molang outside the supported slice reads 0.
- **Textures**: one GPU texture per distinct set of layer textures or skin, composed on first use.
  TGA alpha marks tinted or overlaid texels, so those load opaque unless the layer is tinted;
  otherwise texels under 10% alpha are cut out.
- **Scale**: the definition's `scale` script times the entity's synced scale. Servers send babies at
  0.5, and kinds with a baby geometry undo that with a script scale of 2.
- **Players** (`skin.rs`): a classic skin is a texture for one of three humanoids (wide, slim, 64×32
  layout). A persona skin brings its own geometry and a separate face texture; both bake into one mesh
  over one stacked texture.
- **Light**: block and sky level at the entity's position from `LightData`, through the terrain's
  curve (`gpu/globals.wgsl`), times a directional shade from the normal.

## Block models (`blocks/model.rs`, `entity/block_models.rs`, `block_models.rs`)

Chests, beds, signs and heads have no quads: their `RenderBlock` carries a `BlockModel` (geometry,
texture, yaw from the block state) and the shape `None`. The mesher lists a section's models, and the
renderer draws those within the fog distance through the entity pass, lit like entities.

- **Meshes**: the game hard-codes chests and signs, so `entity/block_models.json` holds them (after
  Java's `ChestModel` and `SignModel`; signs at 2/3 scale). Beds and heads come from the pack's
  `geometry.bed` and `geometry.*_head`, moved into the block: the pack's bed stands upright and the
  heads sit at neck height. All bake to one bone (`Mesh::fixed`).
- **Block entity data** (`BlockData`, `Renderer::set_block_data`): the caller reads it from the NBT.
  `color` picks the bed texture (red without it). `pairx`/`pairz`/`pairlead` join two chests: the
  lead draws the double model between both blocks, the other half nothing. `Rotation` turns a floor
  head; that 0 faces north is assumed from Java.
- A bed's head piece draws both halves. Lids never open. Signs show no text.

## Day and night (`sky.rs`, `gpu/sky.rs`)

`Renderer::time` is the time of day in ticks (noon until set). `Sky::at` turns it into the sky and fog
colour and the sky light levels lost (up to 11 at midnight), with Java's sun-angle formulas; the
shaders subtract those levels from every cell's sky light, so block light is untouched. Dimensions
without a sky ignore the time.

After `Renderer::set_sky_textures` the frame starts with the sun, the moon and 1500 stars (after
Java's `SkyRenderer`): quads 100 blocks from the camera, turned with the time around the north-south
axis and added onto the sky colour without depth writes, so terrain covers them. The sun and moon
textures have black backgrounds, which adding leaves invisible. `Renderer::moon_phase` picks the
cell of `moon_phases.png` (`sky::moon_phase` of the world time). Stars fade in as the sun sets.
There is no sunrise glow.

## Look (`look.rs`)

`Renderer::look` holds what differs between the editions at draw time; it reaches the shaders through
the `Globals` uniform and can change between frames. Plan for the rest: docs/java-look.md.

| | `Look::BEDROCK` (default) | `Look::JAVA` |
|---|---|---|
| Fog distance | From the camera | From the vertical axis through the camera |
| Fog ramp | Smoothstep over the last 30% | Linear over the last 10%, 4 to 64 blocks |
| Water opacity | 0.65 (`water_surface_transparency`) | The texture's alpha |

## Look pack (`lookpack/`)

A `LookPack` is what the renderer draws blocks with: a `Look`, the distinct `RenderBlock`s, which
block state uses which (keyed `minecraft:oak_log[pillar_axis=y]`), and the texture array with its
animations. `LookPack::block_table(registry)` gives the `BlockTable` for a world; states the pack
lacks (custom blocks) are missing-texture cubes. Block models are not stored: they follow from the state.

`LookPack::bake_bedrock` builds one from the Bedrock pack over the vanilla registry. `tools/lookbake`
saves it (`cargo run --release -p lookbake -- bedrock`), by default to `assets/looks/bedrock`
(`$ACACIA_LOOKS` moves `assets/looks`):

- `pack.json`: version, look, blocks, states, animation timing. Another version is refused.
- `textures.png`: the array layers stacked top to bottom.
- `frames.png`: every animation's frames, in `pack.json`'s order.

The viewer loads a baked pack when there is one and bakes in memory otherwise. Entities, biome
colours and the sky textures still come from the Bedrock pack directly.

## Not yet

Weather, GPU occlusion culling (Hi-Z), UI. Block models: hanging signs, banners, sign text, bells,
open lids, piglin heads. Entities: animation
state between frames (attacks, grazing, swimming, riding), blended overlay layers and controller colours (slime shell,
creeper flash, collar and armour dyes), queries that need untracked state (equipment, synced
properties such as the climate variant), babies' own proportions where the pack has no baby
geometry, dropped items, name tags, armour and held items, capes.

Approximate: water loses 2 light per block (the wiki's Bedrock opacity note; its table is ambiguous),
so seabeds deeper than ~7 blocks go dark. Each section change relights its whole column.
