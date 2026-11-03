# Real-schematic trial (2026-10-28)

Nine vanilla structures saved from a generated BDS 1.26.52.3 world (seed 20261028), built by one bot each on a fresh
flat pool world (strict BDS, stock 20 TPS) with `examples/build.rs SCHEMATIC=`. This is measurement only: no builder
behaviour was changed.

The live runs used the harness (1cd4379) on master fd09a5d. The branch was then merged up to 750f657, which changed
the planner (upright-only rule, sight-row work) and the pilot. The door, restock, loader and twocell code named
below is unchanged there; rerun the trial before relying on the speed numbers.

**Result: 1 PASS out of 9.** Only the ruined portal, the one structure without a door, passed. Five of the eight
failures are one chain: a door's upper half can never match, and the bot then gets trapped in the door's cell.

## Structures

Boxes are cropped to the built blocks. Cells inside a box that are terrain are kept, and the "ground" column counts
them; the builder has to place them like any other cell.

| Structure | Size | Non-air | Supported | Ground in box |
|---|---|---|---|---|
| plains_house (big house) | 9×10×9 | 314 | 100% | 13 short_grass, 2 bush |
| plains_smith (weaponsmith, lava) | 11×8×8 | 343 | 100% | 6 short_grass |
| savanna_house | 11×7×9 | 373 | 98.1% | 51 grass, 19 dirt, 41 short_grass, 6 path |
| desert_house | 12×8×9 | 330 | 100% | 61 sand |
| taiga_house | 9×6×9 | 212 | 99.1% | 4 grass, 5 ferns |
| snowy_taiga_house (taiga village on snow) | 7×12×7 | 371 | 88.7% | 81 dirt, 59 grass, 42 snow layers, ferns |
| pillager_outpost | 15×23×15 | 1562 | 99.5% | 188 dirt, 144 grass, 32 plants |
| ruined_portal | 7×7×9 | 121 | 100% | netherrack 50 (part of the portal) |
| mansion_slice (16×16 column, floors 1-2 and roof) | 16×21×16 | 1932 | 99.9% | none |

No igloo exists within locate range of this seed (49 `/locate structure igloo` from a 12 000-block grid), and no
snowy-plains village was found, so the snow-covered taiga house stands in for the snowy one.

## Offline survey (`build/tests/vanilla_survey.rs`)

On an empty site every non-air cell is `Place` except these:

| Block | Cells | Structures | Rejected by |
|---|---|---|---|
| snow_layer | 42 | snowy_taiga_house | `keys_known`: `height=0`, `covered_bit=0` |
| wall_banner | 15 | savanna_house 7, pillager_outpost 8 | `DENIED "banner"` |
| cauldron (`fill_level=6`, water) | 3 | taiga_house 2, mansion_slice 1 | `keys_known`: `fill_level=6` |

## Live results

| Structure | Result | Time | Ticks | Placed / broken | Walks | Corrections | Mismatches | Failed (why) | End |
|---|---|---|---|---|---|---|---|---|---|
| ruined_portal | **PASS** | 47 s | 570 | 121 / 2 | 17 | 2 | 0 | none | done |
| taiga_house | FAIL | 195 s | 3258 | 68 / 3 | 7 | 1534 | 147 | door (placed wrong) | stall cut |
| plains_house | FAIL | 203 s | 3354 | 50 / 3 | 9 | 1522 | 269 | door (placed wrong), 15 floor cells (acts failed) | stall cut |
| plains_smith | FAIL | 213 s | 3490 | 139 / 3 | 17 | 1525 | 208 | door (placed wrong), 4 floor cells (acts failed) | stall cut |
| desert_house | FAIL | 203 s | 3355 | 115 / 3 | 10 | 1535 | 218 | door (placed wrong) | stall cut |
| savanna_house | FAIL | 250 s | 4058 | 278 / 3 | 27 | 1519 | 102 (7 banners) | door (placed wrong), 1 no stand | stall cut |
| snowy_taiga_house | FAIL | 229 s | 3947 | 209 / 2 | 32 | 7 | 162 (42 snow) | none | stall cut (held Blocked 2978 ticks) |
| pillager_outpost (2 runs) | ERROR | 2523 s | ~44 000 | 1500 / 1503 placed | 410 | n/a | n/a | ~12 no stand | watchdog, both runs |
| mansion_slice | FAIL | 929 s | 11106 | 716 / 118 | 699 | 62 | 1285 | none | stall cut (missing 3 items), scaffold 96 left |

- "Stall cut" is the harness ending a run after 3000 ticks without progress. None of these runs would have ended on
  its own.
- Almost every mismatch is "want X, found air": cells never placed because the run stalled. The only wrong states are
  the doors' upper halves (every door house) and two lower halves found open.
- Restocks: one each, except snowy 2 and mansion 9. Refused acts: plains_house and plains_smith ended with 15 and 4
  "acts failed" on floor cells while the bot was trapped (below).

## Ranked gaps

Ranked by structures blocked, then cells.

1. **Door upper half compares a state vanilla never sets.** *Wrong state (rule bug).* Fix size **S**.
   - Blocks: 7 structures, 10 of 11 doors. It starts the stall in 5 runs.
   - In every vanilla door (11 of 11) the upper half stores `cardinal_direction=south` and `open_bit=0`, whatever the
     lower half says. BDS gives the upper half the lower half's facing, so the cell is placed wrong forever: 4
     place/break cycles, then given up "placed wrong".
   - Fix: `rules/twocell.rs` `ignored()` skips both keys on door upper halves, next to the hinge rule. Doors are the
     only two-cell block in the files.
2. **The pilot walks into a door cell and the bot is trapped for good.** *Unreachable / stuck.* Fix size **M**.
   - Blocks: 5 structures, stalled for good.
   - After the give-up the pilot walks into the closed door's cell; in plains_house feet reach z -6.9, inside door
     cell z -7. From then on every tick is a movement correction (~1520 a run). Every walk fails "no closer for 601
     ticks", acts time out ("acts failed" on unrelated floor cells), and the door is found opened (`open_bit=1`).
   - Two holes: doors are not in `rules::thin` / `SearchOpts::no_body`, and the pathfinder treats a door cell as
     passable. Nothing in the builder notices a correction storm or escapes it.
   - Fix: pathfind's door model plus a builder escape (break the thin block in the body, as panes already get).
3. **Stranded on top of a tall build.** *Unreachable / no stand.* Fix size **M-L**.
   - Blocks: pillager_outpost, both runs (1500 of ~1554 placed).
   - The bot finishes the tower roof at y 21 and stays there. Walks fail "every reachable cell searched" and ~90
     cells below have no stand, until the watchdog at 2523 s. The pilot has no way down: no controlled descent, no
     digging down through its own pillar.
   - Fix: the builder keeps a way down (a scaffold column or ladder it removes last), or pathfind gains a dig-down
     move.
4. **Restock asks for more than the inventory holds.** *Restock / inventory.* Fix size **S-M**.
   - Blocks: mansion_slice (60 stacks of need against 36 slots).
   - `restock::fetch` asks for the whole shortfall. What does not fit comes back unfetched, and the station is marked
     lacking it. The bot then waits "Missing" on items the barrels still hold: 204 s of `hold/Missing`, and in the
     end a stall with 1215 cells open and `missing [cobblestone_wall, glass_pane, dark_oak_fence]`.
   - Likely cause, not traced through the inventory: the journal has no inventory snapshot.
   - Fix: cap the request at the free slots, and never mark a station lacking when the inventory was full.
5. **A pit left with no stand.** *Order problem.* Fix size **M**.
   - Blocks: snowy_taiga_house.
   - The house stands on a dirt mound. The builder filled the mound around a 2×1×2 column of dirt (cells
     [6..7,-20..-19,-3]) and then had no stand for it: rested 87 times, never given up, `hold/Blocked` 153 s.
   - The give-up rule ("45 rests with nowhere to do it from") did not fire for these rests.
   - Fix: in `plan.rs`, fill ground pits before what walls them in, and count these rests towards the give-up.
6. **World drift during long builds.** *Wrong state (environment).* Fix size **S** (harness) or **M** (rules).
   - Blocks: pillager_outpost (cells to break rose from 168 to 206 while idle). `[4,-20,*]` dirt cells ended "placed
     wrong".
   - Grass spreads onto dirt targets and vines grow into air targets. Random ticks run on the pool worlds.
   - Fix: decide whether grass on dirt and vine growth count. Either compare them as done, or set the pool's
     `randomtickspeed` to 0.
7. **Unsupported blocks.** *Unsupported block.* Fix size **S-M** each.
   - snow_layer (42 cells, 1 structure): a count like candles, plus `covered_bit`; interact tier, M.
   - wall_banner (15 cells, 2 structures): denied by user decision (patterns are not built). Placing a plain banner
     would be S.
   - cauldron with water (3 cells, 2 structures): fill by bucket, interact tier, S.
8. **The loader rejects BDS LevelDB structures.** *Harness / loader.* Fix size **S**.
   - Blocks: all 9 files as saved.
   - `/structure save ... disk` writes `structuretemplate_*` keys into the world's LevelDB, with each
     `block_indices` layer stored as an IntArray. `Structure::load` (`structure.rs` `ints`) reads only lists of
     ints and fails with `Field("block_indices")`.
   - `tools/ldb-structures.py` rewrites the layers as lists for this trial; the loader should accept both.
9. **Speed.** *Speed only.* Fix size **M**.
   - Wooden doors break by hand in ~4.7 s (the kit has no axe): 4 cycles cost ~19 s a door.
   - Mansion: 699 walks for 716 placements (`go` 541 s against `act` 174 s) and 5206 rests. Leaving the stalls
     aside, that is ~1.3 s a placement against house16's ~0.4 s.
   - Scaffold left standing (savanna 3, snowy 5, mansion 96) is a symptom of the stalls, not a gap of its own.

Harness notes:
- `KILL_AT` stays 0 in file mode.
- The stall cut (3000 ticks) and the watchdog (180 s + 1.5 s a block) bound every run.
- Pool BDS worlds are flat and ~40 blocks of air sit above the pad, so tall structures fit.
- The observer goes to 2 blocks above the structure's top.

## Rerun

```
tools/save-vanilla-structures.sh                      # ~2 min: own BDS ~/work/bds-trial-gen :19520, stops it after
REMOTE_DIR=work/acacia-build-trial \
  bash tools/remote-test.sh -j 6 -p acacia-bot --lib vanilla_survey -- --ignored --nocapture   # offline survey
REMOTE_DIR=work/acacia-build-trial-live TIMEOUT=3600 STALL=1500 DETACH=1 \
  bash tools/remote-run.sh bash tools/schematic-trial.sh             # all nine, ~10 min; or name some: taiga_house
bash tools/testbox-wait.sh work/acacia-build-trial-live run          # in the background; exits with the run's code
python3 tools/trial-times.py ~/work/acacia-build-trial-live          # build, per-run game seconds, stalled tails
```

- Live runs keep their own dir (`-live`), so unit tests (`REMOTE_DIR=work/acacia-build-trial`) run meanwhile.
- Checking one fix: name the structure, not all nine. `STALL_TICKS` (default 1200 here) ends a run that long
  without a cell done; the stall explain still prints at 1000 (research/iteration-speed.md).
- Each run writes `trial-<name>.log` (verdict line, mismatches, `build::findings`, explain on stalls) and
  `journal-<name>/` in the remote dir.
- The `.mcstructure` files are Mojang content: they stay in `~/work/schematics/vanilla`, outside the repo.
