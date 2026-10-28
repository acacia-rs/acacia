# Builder: blocks one click decides (oneclick tier)

Rules in `build/rules/oneclick.rs`, hooked from `rules/mod.rs`; tests `build/tests/rules_oneclick.rs`; sim model
`pathfind/tests/place_oneclick.rs`, sim test `pathfind/tests/build_oneclick_tests.rs`; live scenes
`examples/support/oneclick.rs`. Measured on strict BDS 1.26.52.3 (2026-10-28) with `examples/place_probe.rs`:
`SET=oneclick` (every family from four sides, every visible face, low and high), `PAIR=1` (chests side by side),
`SUPPORT=1` (side clicks over a fence or air, under stone or air; clicks on a chain's ends), `VERT=1` (barrel
from close above and below).

## Rules

| Block | State | Decided by |
|---|---|---|
| trapdoors (all) | `direction` | towards the player, named as stairs name it (east 0, west 1, south 2, north 3), whatever face |
| | `upside_down_bit` | as stairs: Down face or a side click above 0.5 |
| fence gates | `cardinal_direction` | towards the player (furnace rule) |
| chests, trapped chests, ender chests | `cardinal_direction` | towards the player |
| anvils | `cardinal_direction` | towards the player turned a quarter clockwise (from the north: east) |
| campfires | `cardinal_direction` | the look (away from the player) |
| stonecutter, smoker, blast furnace, lectern | `cardinal_direction` | towards the player (already in B1's rule) |
| lanterns (all) | `hanging` | Down face: 1; Up face: 0. A side click gives 0 with a block under the cell (a fence counts), 1 with only one above, and is refused with neither: planned as 0 only, which the bottom-up order makes safe |
| chains (all) | `pillar_axis` | clicked face's axis |
| hopper | `facing_direction` | Up or Down face: down (0); side face: into the clicked block |
| end rod | `facing_direction` | the clicked face; BDS names the horizontal ones backwards (north-pointing reads 3) |
| lightning rods | `facing_direction` | the clicked face |
| barrel | `facing_direction` | towards the player, six-way like a dispenser (up from close above, down from below) |
| glazed terracotta | `facing_direction` | towards the player, horizontal |
| amethyst cluster, buds | `block_face` | the clicked face |
| bell, grindstone | `attachment` | Up: standing, Down: hanging, side: side (bell: `multiple` when the cell across is filled; it changes when that cell fills later, so `side` and `multiple` are compared alike) |
| | `direction` | floor and ceiling: towards the player; wall: the clicked face (south 0, west 1, north 2, east 3) |
| loom, beehive, bee nest, decorated pot, chiseled bookshelf | `direction` | towards the player |
| pressure plates, carpets | - | need a block under the cell (side clicks over air refused) |
| redstone block, lamp, torch, ores; sea lantern; dead coral blocks; leaves; hay; cauldron, composter, copper bulb, tnt | - | as plain blocks / torches; were denied by `DENIED` substrings or by unknown keys |

Fixed (every placement gives it; another target value is reported `unsupported`): `open_bit` 0, `deprecated` 0,
`extinguished` 0, `honey_level` 0, `composter_fill_level` 0, `books_stored` 0, `explode_bit` 0, `fill_level` 0,
`cauldron_liquid` water, `lit` 0 (copper bulb). Never compared (server-set): `in_wall_bit`, `redstone_signal`,
`toggle_bit`, `update_bit`, `persistent_bit` (a placed leaf is persistent, a structure's natural one not).
Lit twins are placed and compared as their base block: `unlit_redstone_torch`, `lit_redstone_lamp`,
`lit_redstone_ore`, `lit_deepslate_redstone_ore`, `lit_furnace`, `lit_smoker`, `lit_blast_furnace`. Transform: trapdoor `direction` turns by its own names,
`block_face` turns.

## Planning changes

- Hanging targets, everything attached to the block above (`oneclick::hangs`, the one place: hanging lanterns,
  bells and grindstones, down-facing rods, ceiling buttons, down-facing amethyst; y chains may hang):
  the block above may be placed while they are open (the never-cover rule would deadlock them), and with the
  cell above open they wait (a y chain only when the cell under it is open too).
- A y chain's Up and Down faces take a click at their centre (`rules::post`, `reach::points`): a lantern or a
  chain under a chain (measured: lantern hanging=1, chain axis y), a lantern on top of one. A chain also counts
  as a neighbour to click for the "nothing clickable, scaffold up" check.
- Anvils wait for support like sand.
- `process.rs`: a bot that drifted into the target cell after arriving at a stand beside it (live: 0.16 off at
  a fence gate's stand) goes to another of the task's stands; before, the arbiter kept the arrived route and the
  build held for good. Also hit by `build/block-set` without this tier in house16 (one of two runs: feet at
  z -7.006 in an oak log's cell, 3005 zero-tick "arrived" walks, FAIL 340 s, 9 mismatches).

## Chests side by side (`PAIR=1`)

Two chests (or trapped chests) placed next to each other with the same facing, the row across the facing, pair
by themselves (`pairx`/`pairz`/`pairlead` in the block entity); each keeps the facing its own placement gave. A
third with another facing next to them does not pair. Ender chests and barrels never pair. Pairing is block
entity data: not compared (two unpaired same-facing chests side by side in a structure come out paired).

## Evidence (2026-10-28, strict BDS 1.26.52.3, fresh world each, `tools/build-scenes.sh`)

- Sim: `builds_the_oneclick_blocks_block_exact` (5×5×3: trapdoors, gate, chest pair, hopper, end rod, a hanging
  lantern, a chandelier, a ceiling button) block-exact.
- Live scenes (`examples/build.rs SCENE=`), observer diff 0, two runs each on the final code where noted:
  - `gates` (trapdoors low and high, iron trapdoor, two fence gates): PASS 30 s, 30 s. The first run failed: the
    high trapdoor's target faced away from the only wall it could be clicked on (scene fixed), and the bot held
    for good inside the spruce gate's cell (the `process.rs` fix above; later runs did not drift there, so the
    fix is untested live).
  - `lights` (hanging lantern, chandelier chain + lantern under a ceiling, standing lantern, rods): PASS 33 s, 31 s;
    with a ceiling button added: PASS 33 s.
  - `stations` (chest pair, barrel, hopper, anvil, bell, grindstone, loom, plate, carpet): PASS 30 s, 30 s; with a
    lit furnace added: PASS 31 s.
  - `redstone` (redstone block, lit lamp beside it, torch on it, lamp, torch, natural leaves, hay): PASS 26 s, 27 s.
  - Regression: `ladder` PASS 52 s; `gates` 31 s and `redstone` 27 s again after merging `build/block-set`.
  - house16, one bot (`examples/swarm_build.rs BOTS=1`, fresh world, this branch with `build/block-set` merged):
    PASS 261 s (3732 ticks), observer diff 0. Same box and BDS, `build/block-set` alone: PASS 260 s, then the
    FAIL above. Both are slower than the 157.5 s of 2026-10-21 (docs/builder.md), so this tier is not the cause.

## Still denied or unsupported

- Open trapdoors and gates (`open_bit=1`), honey, books, filled cauldrons and composters, lit copper bulbs:
  more than a click (interact tier or later).
- Live coral blocks (die out of water), skulls and banners (16-way rotation, block entities), crafter
  (orientation), vault and trial spawner, command blocks: unchanged.
- Glazed terracotta and barrel far-off vertical cases follow the dispenser's measurement (1.9 off: horizontal),
  not measured for these blocks.
