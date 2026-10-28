# Builder: redstone tier (2026-10-28)

Blocks whose final state depends on neighbours, order, support or growth. Rules: `build/rules/redstone.rs` (hooked
from `rules/mod.rs`), order and survey `build/rules/redstone_order.rs`, the rail model `build/rules/rails.rs`; tests: `build/tests/rules_redstone.rs`, sim
`pathfind/tests/redstone_tests.rs` (server model `pathfind/tests/redstone_sim.rs`); scenes
`build/tests/scenes_redstone.rs` (`SCENE=rs-rails`, `rs-garden`). Measured on strict BDS 1.26.52.3 with
`examples/place_probe.rs SET=redstone` (`examples/place_probe/redstone.rs`, `ONLY=<case>,...`).

## Rules

| Family | Placed | Compared |
|---|---|---|
| redstone dust | `redstone` on an Up face | the wire is there (`redstone_signal` derived) |
| rail, golden/detector/activator rail | Up face, in an order that gives the shape (below) | `rail_direction`; `rail_data_bit` derived |
| water, lava source | `water_bucket`/`lava_bucket` on the Up face of the block under the cell (`Act::UseOn`) | a source of that liquid; flows never compared |
| sapling, flower, mushroom, bamboo sapling | Up face of the block under it, once that is done | name; `age_bit` derived |
| sugar cane (`reeds`), cactus | Up face; cane once its sand and the water beside it are done | name; `age` derived |
| vine (one side) | the clicked side face: North 1, East 2, South 4, West 8 (bit = the face clicked) | `vine_direction_bits` |
| kelp, seagrass (`default`) | water bucket into the cell first, then the plant on the Up face | name, `sea_grass_type`; `kelp_age` derived |
| farmland | dirt, then `wooden_hoe` on its Up face (`Act::UseOn`, done when the cell shows `farmland`) | name; `moisturized_amount` derived |
| wheat, carrots, potatoes, beetroot | seed item on farmland, clicked at y 0.9375 | name; `growth` derived |

Order: a plant waits for its support (and a vine for its side block); liquids, kelp and seagrass go after every
dry cell that is ready (`dry_first`), so pours do not wash plants or dust away. Flows from the job's own sources
into cells where air belongs are left alone by the survey and the check (draining them would fight the sources).
Hoes and buckets come from the stations like blocks (`restock` counts the extra item); an emptied bucket stays in
the inventory.

### Rails (port of Java `RailState`, matches sapling `rails.rs`)

A placed rail joins free neighbouring rails (same level, or one up: it rises). Measured: a lone rail is 0 (N-S)
whatever the look; a straight-only rail joined along both axes runs E-W; a plain rail with 3-4 neighbours takes the
last fitting curve of NW 8, NE 9, SW 7, SE 6; a rail with two joined ends never turns again. The planner places a
rail only when every rail the placement may join leads where its target does (`rail_ready`, with one step of
lookahead over open rail cells), and the survey counts a rail as on its way (Done) while its joined ends are a
subset of its target's and a rail at a target end is not at its own shape yet. Turned and mirrored structures map
`rail_direction` through `turned_rail`.

Unreachable, left to `Need::Break` and a retry or reported: a lone E-W rail (nothing to join), a curve or rise
without its neighbours, and junction shapes whose neighbours were placed in an order BDS resolves the other way.

## Evidence

- Probes, BDS 19316, 2026-10-28: rails as above (golden curve-last 1, rising 2 either order, rail-steal E 8); dust
  on top and side clicks; saplings on grass/dirt, refused on stone; poppy on grass, dandelion refused on stone;
  mushrooms refused at noon on dirt and stone; cane needs water beside its sand; cactus refused with a solid
  beside; vine only on the clicked side (not ceilings, not floors); kelp in water and dry, kelp on kelp in air
  refused; seagrass default only; buckets on top and side faces, the held item becomes `bucket`; hoe tills dirt
  and grass from top and side, refused when covered; crops on farmland at y 0.9375, seeds refused on dirt.
- Live, pool BDS, fresh worlds, observer diff 0, 2026-10-28: `rs-rails` PASS 115 and 119 ticks (18 s, 17 s), 20
  placed, 1 broken; `rs-garden` PASS 144 and 146 ticks (22 s, 23 s), 25 placed, 7 broken (the floor cells the scene
  replaces). rs-rails passed on all four pool runs after the to-come fix.
- Live fixes they needed: a pour against a side face grazing the neighbour's top edge landed on that top, so
  liquids pour on Up faces only; a falling flow shows as `water` with `liquid_depth=9`, so `UseOn` waits for a
  source; pours and tilling are never en-route acts; grass under a placed block turns to dirt at once (the garden
  puts dirt under its vine's stone).
- Sim: `cargo test -p acacia-bot --lib -- build:: pathfind::tests` (both scenes block-exact, no scaffold left).

## Still denied, and why

- Grown `bamboo`: the stalk on top of a placed one was not clickable.
- Vines with more than one bit, `_vines` (twisting, weeping, cave): one click attaches one side; cave vines hang.
- Double seagrass: seagrass on seagrass was not clickable.
- Waterlogged blocks are built dry: `.mcstructure` keeps the water in `block_indices` layer 2
  (`minecraft:water liquid_depth=0`), which `structure.rs` does not read.
- A liquid source with nothing under it (Up-face pours only).
- Mushrooms are supported but BDS refuses them in daylight; a lit build fails them as `acts failed`.
- Kelp on kelp: refused in air, unmeasured in water (the scenes use single kelp).
- `waterlily`, `lava_cauldron`, `cactus_flower`, `bubble_column`: not probed.

## Seams for merging

- `Act::UseOn { item, against, face, cell, expect }` (tilling and pouring) sits beside the interact tier's
  `Act::Use`. Tilling could become `Act::Use`; a pour changes the cell beyond the clicked block, which `Use` does
  not watch, so it stays its own act unless `Use` learns a watched cell.
