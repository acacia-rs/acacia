# acacia-registry

Edition-neutral Minecraft blocks for Java 26.3 and Bedrock 26.50: a `Block` kind, a `BlockState`, and typed
properties. No dependencies, no I/O. Plan: docs/java-edition.md.

```rust
use acacia_registry::{prop, value::Direction, Block, BlockState};

let s = Block::OakStairs.default_state().with(prop::HorizontalFacing, Direction::East).unwrap();
assert_eq!(s.get(prop::Half), Some(acacia_registry::value::Half::Bottom));
let t: BlockState = "minecraft:oak_stairs[facing=east]".parse().unwrap();
assert_eq!(s, t);
assert_eq!("noteblock".parse::<Block>(), Ok(Block::NoteBlock)); // Bedrock spelling
```

## Model

- Blocks and properties follow Java 26.3. Blocks only Bedrock has (`hard_glass`, `underwater_tnt`, `frame`...)
  are appended, sorted by name, with their Bedrock properties (`prop::Bedrock*`). Bedrock's legacy
  `stonecutter` becomes `legacy_stonecutter`, since Java's `stonecutter` is a different block.
- `BlockState` ids below 35 723 are Java's global palette ids. A Java chunk needs no translation.
- One marker type per Java `BlockStateProperties` constant (`prop::HorizontalFacing`, `prop::Age7`). Same-named
  properties with different values stay apart. Value types live in `value` (`Direction` serves `FACING`,
  `HORIZONTAL_FACING`, `FACING_HOPPER`); ints are `u8`, booleans `bool`.
- Names parse with or without `minecraft:`, in Java's spelling or in a Bedrock spelling that names exactly one
  block. Merged Bedrock names (`bed`, `standing_banner`) don't parse: they don't say which block.
- Tags: every Java block and item tag, nested tags resolved (`BlockTag::Fences`, `ItemTag::Logs`,
  `BlockTag::MineablePickaxe`). Bedrock-only blocks belong to none.
- `Item` follows Java's registry, so `item as u16` is Java's network id. It carries `max_stack`, the `block` it
  places and its Bedrock `(name, data)`. Data is 0 except where Bedrock merges items under one name: beds and
  banners by colour. `Item::from_bedrock(name, data)` resolves a server's item names. Items only Bedrock has
  give `None`.
- Bedrock runtime ids ↔ neutral states live in acacia-world (`BlockState::neutral`, `neutral_flags`,
  `BlockRegistry::runtime_id_of`). The mapping rules are in `tools/gen.mjs`.

## Data sources

Regenerate with `node crates/acacia-registry/tools/gen.mjs crates/acacia-registry/tools/.cache`. It writes
`src/generated/*.rs` and `crates/acacia-world/data/neutral.bin`. The cache holds:

- `java-26.3-blocks.json` and `java-26.3-registries.json`: data-generator reports,
  `java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --reports`.
- `java-26.3-item-stacks.json`: `{path: max_stack_size}` from that run's `reports/minecraft/components/item/*.json`
  (the components aren't bound in a bare bootstrap, so BlockDump can't read them).
- `tags/`: `unzip <server jar> 'data/minecraft/tags/block/*' 'data/minecraft/tags/item/*'`.
- `geyser-items.json`: GeyserMC/mappings `items.json` at the same commit as `geyser-blocks.nbt`.
- `java-26.3-block-dump.json`: `tools/javadump/BlockDump.java` run against the unobfuscated server jar (classpath:
  the bundler's `versions/26.3/*.jar` plus `libraries/**.jar`, which the report run extracts). It gives each
  property's declaring constant and value class, each item's block, and collision boxes per state.
- `geyser-blocks.nbt`: GeyserMC/mappings `blocks.nbt` at `81700ab` (MIT), the same commit as acacia-anvil.
- `bedrock-states.tsv`: `cargo run -p acacia-world --example dump_states`. Regenerate it whenever acacia-world's
  `blocks.bin` changes, then rerun the generator.
