# acacia-bot block-name audit (2026-11-08)

Sites in `crates/acacia-bot/src` that identify blocks by name or Bedrock property string, for the move to
acacia-registry (docs/java-edition.md step 1). Line numbers are as of `15661b7`.

Counts: A shared logic ≈165 lines in ~30 files (~70 % in `build/rules/*`); B Bedrock protocol ≈26; C items/entities ≈99;
D tests 466 lines in 66 files plus 62 in inline `#[cfg(test)]` modules.

## Migration order (central helpers first)

1. `pathfind/cell.rs` `state_flags` (fire, campfire, cactus, magma, FALLS, OPENABLE) and the door `open_bit` /
   `upper_block_bit` reads in `pathfind/alter.rs`, `terrain.rs` `toggled_boxes`, `execute.rs`.
2. `interact/direct.rs` `is_interactive` (`INTERACTIVE` substrings).
3. Slab checks (`interact/item_use.rs`, `build/reach.rs`, `build/plan.rs`) → `BlockTag::Slabs` + `prop::SlabType`.
4. Sleep (`sleep.rs`, `sleep/tick.rs` `lying_in`) → `BlockTag::Beds` + `prop::HorizontalFacing`.
5. `build/rules` kinds: `twocell::kind`, `rules::thin`/`gravity`, `redstone::kind`.
6. Rest of `build/rules` last: its property logic models BDS placement, and the build target `structure::Block` is a
   Bedrock `.mcstructure` palette entry.

## Likely bugs found

- `is_interactive`: `"bed"` matches `bedrock`, `"door"` matches `iron_door`/`iron_trapdoor`, `"note_block"` misses Bedrock `noteblock`.
- `ends_with("_slab")` accepts double slabs (`*_double_slab`): `item_use.rs` occupied check, `reach.rs` bottom_slab/support, `plan.rs`.
- `rules::thin` `contains("_wall")` matches `*_wall_sign` (Java names would add wall torches, banners, fans, hanging signs).
- `cell.rs` FALLS lacks anvils (`rules::gravity` has them); both lack `dragon_egg`, `pointed_dripstone`.
- `twocell::kind`, `structure.rs`, `sleep/tick.rs` only know Bedrock `bed`/`straw_bed`.
- `cell.rs` uses Bedrock-only `magma` (Java `magma_block`).
- physics `is_fence_or_wall` missed Bedrock `fence_gate` (fixed in acacia-physics, tests/fence_or_wall.rs).

## Stays Bedrock (B)

`prediction_sync.rs` movement components, `movement/packets.rs` + `state/player.rs` attributes,
`state/environment.rs` clock, `signs.rs` sign NBT, `build/structure.rs` `.mcstructure` typing, diagnostics in
`build/sight_stats.rs` and `trace/replay.rs`.

## Items (C) wait for an `Item` enum

books, fishing, elytra, flight, geometry, riding, boat launch, entities, survival food/equip, workstations,
`state/recipes/tags.rs` (hand-rolled item tags Java's item tags could replace), `pathfind/kit.rs` scaffold list.

## Already neutral

Break times (`Mining`/`Material`), climbable/liquids/cobweb/berry/powder snow (`BlockFlags`).
