# acacia-world

Network-free world state for Bedrock 1.26.5x (protocol 2193): block registry, chunk decoding, and
chunk storage shared between bots on one server.

## Data sources (regenerate with `node crates/acacia-world/tools/gen-blocks.mjs [cacheDir]`)

- **State list and order**: Geyser `core/src/main/resources/bedrock/block_palette.26_50.nbt` (MIT), 22 091
  states already sorted by FNV-1 64 of the name (the generator asserts this). Includes each state's
  `network_id` (FNV-1a 32 of LE NBT `{name, states}`) for servers with hashed ids (BDS).
- **Collision**: minecraft-data `bedrock/1.26.30/{blocks,blockStates,blockCollisionShapes,blocksJ2B}.json`
  (MIT). Already carries most of Boar's Bedrock diffs (chest, trapdoor 0.1825, bed, honey, cactus...).
- **Bedrock overrides, friction, flags**: Boar (MIT) `BedrockCollision.java`, `AbstractBoarBlockState.java`,
  `GeyserBlockMappings.java`; ported into `tools/rules.mjs`.
- **Mining** (`tools/mining.mjs`): minecraft-data `blocks.json` `hardness`, `material` (→ `Material`) and
  `harvestTools` (item ids 939 + 5·tier + kind, 1134 = shears → allowed tool kinds + minimum harvest level).
  Blocks missing from minecraft-data borrow via `templateFor`; the rest (`shelf_mushroom`) get NaN hardness.
- **Light** (`tools/light.mjs`): minecraft-data `emitLight`/`filterLight`, corrected from minecraft.wiki/w/Light:
  its Bedrock filter table (leaves and ice 2, slabs 1, beacon 14...) and state-dependent emitters (`lit_*`
  blocks, candles, sea pickles, respawn anchors, copper bulbs, campfires, lava cauldrons, `light_block_N`).
- Output: `data/blocks.bin` (~610 KB, `include_bytes!`, parsed once on first use).

## Runtime ids

Runtime id = palette index. With `StartGame.block_properties`, build
`BlockRegistry::vanilla().with_custom_blocks(&[CustomBlock { name, state_count }])`: custom states are
appended and stable-sorted by FNV-1 64 of the name (Geyser `BlockRegistryPopulator`). `state_count` = product
of `properties[].enum` lengths. Names already in vanilla (26.50 data-driven blocks such as wool stairs) are
skipped. A Geyser server with custom blocks sends `geyser_custom:*` entries, so this step is required there.
If `block_network_ids_are_hashes`, create the world with `BlockIds::Hashed`.

## API

- `BlockRegistry`: `vanilla()`, `vanilla_arc()`, `with_custom_blocks`, `get(id)`, `air_id()`, `find(name, props)`,
  `states_of(name)`, `runtime_id_from_hash`. `BlockState`: `name`, `properties` (`k=v,...` sorted), `boxes`,
  `friction`, `flags` (`BlockFlags`), `liquid_depth`, `network_hash`, `light_emission`, `light_filter`, `mining` (`Mining`: hardness, `Material`,
  `can_harvest(Option<Tool>)`; `Tool::from_identifier("minecraft:iron_pickaxe")`), plus `is_solid`, `is_liquid`,
  `fluid_height`, `jump_factor`, `stuck_multiplier`, `property(key)`.
- `Chunk::decode(x, z, Dimension, sub_chunk_count, payload)`, `block`/`liquid(x, y, z)` (world coords),
  `set(x, y, z, layer, id)`, `set_sub_chunk(section_y, payload)`. Keeps layers 0 and 1 packed with their palettes.
- `World::new(registry, dimension_id, BlockIds)` holds chunks weakly; `ChunkView::new(world)` per bot holds
  strong refs: `insert_level_chunk` (skips decoding when the payload hash matches the live chunk),
  `insert_sub_chunk`, `set_block` (UpdateBlock / UpdateSubChunkBlocks, wire ids), `retain_within`, `remove`.
  `World::wire_id(runtime_id)` gives the id to send back (hash under `BlockIds::Hashed`).
- `World::subscribe()` yields every applied `ChunkChange` (`Column`, `Section`, `Block`); loaded chunks
  aren't replayed, so read `chunk_positions()` after subscribing. `Chunk::copy_section` unpacks a section's
  two layers in XZY order and `section_uniform` spots all-air sections (for renderers).
- Biome ids per section: decoded after the sections of a full `LevelChunk`; `World`/`ChunkView::insert_biomes`
  takes a biomes-only payload (request-mode `LevelChunk`, or its cache-mode biome blob, which the bot's
  `Blobs` resolves). `Chunk::biome(x, y, z)`, `copy_biomes(index, out)`; ids are raw, never remapped.
  Malformed biome data keeps what decoded instead of failing the chunk.
- `BlockAccess` (`block`, `liquid`; unloaded reads air) is implemented by `ChunkView`.
- Since 26.50, stairs (`minecraft:corner`), fences and panes (`minecraft:connection_*`) carry their
  connections in the state, and walls already did. So `boxes` is complete and needs no neighbour lookups.

## Approximate

- minecraft-data shapes are partly Java-derived; blocks new in 26.50 borrow shapes (poplar set → oak,
  wool/concrete slabs → oak slab); `shelf_mushroom` is assumed to have no collision.
- Stair corners come from the 26.50 `minecraft:corner` state and are assumed to follow Java `StairsShape`.
- Doors are rebuilt from Boar's Java-keyed rules through J2B, because minecraft-data gives every door state
  the same shape.
- `DYNAMIC_SHAPE` blocks (scaffolding, powder snow, bamboo, pointed dripstone) need physics-side handling.
- Fence arms use Java's post width (0.375 to 0.625, 1.5 high). Panes and bars use Boar's thin-bar boxes.
- Custom blocks collide as full cubes, block all light, and have no properties or hash.
- Trial spawner light by state (4 idle, 9 active) and vault (6 inactive, 12 otherwise) follow the wiki's
  wording; conduits always emit 15 (activity lives in the block entity).
