# Builder tier `twocell`: blocks one placement fills two cells

Doors, beds and tall plants (`build/rules/twocell.rs`, planner side `build/pairs.rs`). Measured 2026-10-28 on
strict BDS 1.26.52.3 with `place_probe` `SET=twocell` (`examples/place_probe/twocell.rs`; `HINGE=1` for the door
neighbour cases only): every item from four sides, five points on the top face below, side faces low and high.

## Rules

| Block | Placed by | States |
|---|---|---|
| doors (`wooden_door`, `spruce_door`..., `iron_door`, copper and waxed copper) | Up face of the block below only; side faces refused | `minecraft:cardinal_direction`: the player's look turned clockwise (looking north: `east`); click point never matters |
| door hinge | neighbours at placement, left/right as seen from the placing side (`right` = the facing) | upper half: 1 with a door left (any type or facing, lower half is enough) and none right, or more opaque full blocks in the two cells right than left (stone counts; glass, slabs, fences, doors don't); else 0. Lower half: always 0, not compared |
| `bed` | Up face only; side faces refused | `direction` (legacy: south 0, west 1, north 2, east 3) = the look; head in the cell ahead, which must be empty |
| bed colour | item aux (`bed 1 14` red) | block entity `color` (`Byte`) on both halves, in the chunk/BlockActorData the bot already tracks, there the tick the placement confirms; a bed set by `/setblock` or `/fill` is red (14) |
| `sunflower`, `lilac`, `rose_bush`, `peony`, `tall_grass`, `large_fern`, `pitcher_plant` | any face but Down (side clicks into the cell work) | `upper_block_bit` only |
| `small_dripleaf_block` | as plants; on clay or moss only (grass refused) | `minecraft:cardinal_direction` towards the player |
| any pair | — | refused with the second cell occupied; `open_bit=1` (interact tier) and `occupied_bit=1` stay unsupported |

Mirroring a door turns its facing half round (it is the look turned) and swaps its hinge (`twocell::mirror_door`).

## Planner

- `pairs::settle` rewrites the survey: a pair is placed from its first cell (lower half, bed foot) and its second
  cell is `Need::Paired`, never placed alone (so restock counts one item per pair). Done only when both halves
  match. Either half wrong with the first cell filled: break the first (BDS takes the other half); junk in an empty
  pair's second cell is broken first. A pair cut by the footprint or with a mismatched partner is unsupported; a
  half unknown makes the pair unknown.
- `pairs::waits`: the first cell isn't placed while a body is in the second, or while `hinge_now` (the hinge BDS
  would give from the neighbours as they stand) differs from the target's upper half; it rests 40 ticks instead
  (a hinge 1 door waits for the wall on its right or the door on its left).
- Hinge ordering, `pairs::held`: for each unplaced door, the fewest unbuilt side neighbours (two cells left, two
  right) that, left out, make the hinge with the final neighbours come out right; those rest until the door is in
  (hinge 0 with a wall right: the wall waits). The door's own wait for neighbours it needs stays `pairs::waits`.
  Deadlock guard: a hold lasts only while the held cell and the door each have under 20 rests (800 ticks) and the
  door isn't given up; swarm bands use the same count, so a door another bot never places frees its neighbours.
- Bed colour: `Structure::load` reads `block_position_data` and carries `color` as a pseudo state `bed_color` (written
  back by `to_bytes`). The item is a key `minecraft:bed@14` (`pathfind/process/item_key.rs`): counts, the station
  take and the slot picked for a place match the aux. `Look::color` reads the block entity; `pairs::settle` breaks
  a bed of the wrong colour (no block entity yet is no mismatch). `build::colors` is the observer's colour check.
- Second cells are filled-later cells: no stands or pillars in them (`plan::fills_later`).
- Sim: `pathfind/tests/place_twocell.rs` (own model of the table), breaks take both halves;
  `build_twocell_tests.rs` builds a double door (one with a wrong hinge to clear, junk in an upper cell), a bed and
  a sunflower block-exact with one placement per pair.

## Evidence

- Unit: `build/tests/rules_twocell.rs`. Sim: `builds_doors_a_bed_and_a_sunflower_block_exact`,
  `a_door_goes_in_before_the_wall_that_would_flip_its_hinge`, `a_bed_takes_the_item_of_its_colour` (the sim keeps no
  colours; colour mismatch is unit-tested).
- Live, second round 2026-10-28, all observer diff 0 including bed colours:
  - `bedcolors` (blue, white, lime; the lime one over a red bed set by command; a red bed in the barrel as a
    decoy): PASS 32 s, 23 placed, 1 broken.
  - `hinge` (hinge 0 door with cobblestone right; hinge 1 door needing its wall): PASS 31 s, 32 placed, 0 broken.
  - Regressions: `doors` 29 s, `beds` 29 s, `plants` 29 s, all PASS.
- Live (`tools/build-scenes.sh`, fresh world each, strict BDS :19312, 20 TPS; scenes in
  `examples/support/scenes_twocell.rs`, a third barrel holds their items), 2026-10-28, all observer diff 0:
  - `doors` (double wooden door, hinges 0 and 1, an iron door between walls, stone junk in its upper cell):
    PASS 28 s, 27 placed (one per door), 1 broken, retries 0.
  - `beds` (facing east and north): PASS 30 s, 22 placed. Beds don't stack: a station needs a slot per bed (a
    first run with `bed 4` in one slot held only one and stalled on `missing bed`).
  - `plants` (sunflower, tall grass, rose bush, pitcher plant on grass, small dripleaf on clay): PASS 29 s, 23
    placed. A first run stalled with the bot's body in the dripleaf's upper cell: that cell's body check now sits
    with the placement's own (the pair stays ready, so the walk moves the bot), not in `pairs::waits`.

## Still denied / not done

- A hinge no subset of the neighbours gives (none known in practice) is not held for: the door waits, then gives
  up "no stand" as before.
- Other block-entity data is still not read; only bed `color` is.
- Open doors (`open_bit=1`): interact tier.
