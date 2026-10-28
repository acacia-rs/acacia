# Builder: interact tier (2026-10-28)

States that take more than a placement click. Rules: `build/rules/interact.rs` (hooked from `rules/mod.rs`); the
adjust step: `build/adjust.rs`; tests: `build/tests/rules_interact.rs`, sim `pathfind/tests/build_tests_interact.rs`
(server model `pathfind/tests/interact_model.rs`). Measured on strict BDS 1.26.52.3 with `examples/place_probe.rs
SET=interact` and `examples/interact_probe.rs` (`PART=use,count,sign,hanging`).

## Adjust step (`Need::Adjust`)

A cell holding the target's block whose only differences are use-toggled states (`rules::uses`), or a sign whose
text differs, is used or written instead of broken. Fill-only jobs adjust too (the builder's own placements come out
this way). Planned after breaks, before places: in reach and some point of the block visible (`reach::sees_block`),
else a walk to a stand that has both. One click per act, confirmed by the cell changing (`Bot::use_confirmed`, one
try: a second would toggle back); the survey then counts again.

`Act::Use { pos, item, face }` is general: `item` is held first (`None`: whatever is held), `face` is preferred
when it shows. Tilling: `Act::Use { pos: dirt, item: Some("minecraft:wooden_hoe".into()), face: Face::Up }`; it is
done once the server shows the cell changed (any other block id). `Act::Sign { pos, front, text }` writes one side.

## Missed click at a stand (`build/arrival.rs`, user's choice 2026-10-28)

Stands are judged from their centre, but a walk that starts inside its stand cell ends at once (an empty path,
`astar` start-in-goal), anywhere in the cell: live 0.43 off, so a plank's one Up-face point was hidden by a daylight
detector, a north comparator would have faced east, and the task rested until the timeout. Now, a stand reached and
asked for again drives to that stand's centre (`GotoOpts::settle`, tolerance 0.1), then to the task's other stands,
then rests. This replaces oneclick's drift rule (other stands when the body is in the cell), which it covers.

## Rules

| State | Decided by |
|---|---|
| `open_bit` (doors, trapdoors, fence gates, levers; not iron) | use-click toggles; placed closed |
| door upper half `open_bit` | not compared: BDS keeps it 0 while the door is open (copper door) |
| `repeater_delay` | use-click: +1 mod 4 (empty hand or holding a block alike) |
| `output_subtract_bit` | use-click toggles |
| `daylight_detector_inverted` | placed as `daylight_detector`, one use-click |
| repeater/comparator `cardinal_direction` | towards the player (as furnaces), any face with support below |
| powered/unpowered repeater, comparator; `output_lit_bit`, `redstone_signal`, `dead_bit`, `cracked_state` | derived: never compared |
| `lever_direction` | side face: the clicked face; Up/Down: `up_`/`down_` + the look's axis |
| standing sign `ground_sign_direction` | Up face only: `round((yaw + 180) / 22.5) mod 16`; clicks within 4° of a boundary avoided; Up faces tried on a 3×3 grid (`rules::fine_aim`) |
| wall sign `facing_direction` | the clicked side face (Down: refused) |
| hanging sign | Down face: `hanging=1`; side or Up: `hanging=0`. `facing_direction` towards the player: on Up/Down from the look, on a side face along the wall (within 0.2 of straight on BDS breaks the tie its own way: avoided) |
| `candles` | one more candle placed into the cell (against the floor) adds one |
| `cluster_count`, `turtle_egg_count` | the floor click is refused: a use-click on the block itself holding the item adds one |
| sign text | written after placing (the server opens the front editor); the back from behind (`rules::front`) |

Items: signs are placed by `<wood>_sign` (`darkoak` blocks by `dark_oak_sign`), repeaters and comparators by their
item, the inverted detector by `daylight_detector`. Note blocks have no pitch state on Bedrock (block entity): placed,
pitch not compared.

## Evidence (live, fresh world, test pack off, `examples/build.rs SCENE=`)

- `interact-use` (repeater delay 2 over a delay-3 junk repeater, subtract comparator, a floor lever over a closed
  junk one, a wall lever, inverted detector): with the re-centre below 4/4 PASS, 31/31/30/31 s, 0 broken (the junk
  was used). Before it 1 of 6 (five ended at the 184 s timeout with one ordinary placement open).
- `interact-count` (4 and 2 candles, 3 pickles, 3 eggs): PASS 30 s.
- `interact-sign` (two standing signs, one with back text, a wall sign, a hanging sign, text on all): PASS 56 s,
  text checked by the observer (`build::check_text`). Each side written takes ~3 s (the human typing pause).
- house16 one bot (`swarm_build BOTS=1`, port 19314): build/block-set 284/321 s, with the re-centre 256/270/309 s,
  all PASS, 0 mismatches; 2 re-centre walks in a run.
- Sim: `uses_counts_and_signs_build_block_exact` (toggles used not broken, 3 clicks for delay 3 -> 2, pickles and
  eggs by use, both sign sides).

## Still denied

- Lit candles (`lit=1`: flint and steel), `candle_cake`, attached hanging signs (`attached_bit=1`, from sneaking or a
  thin block above: not measured).
- Iron doors and trapdoors with `open_bit=1` (redstone only).

## Open

- A use-click is confirmed by any change of the cell: a daylight detector's signal update in flight could confirm a
  click before its toggle shows, and the next survey click again (not seen live).
- Pickles and eggs added by use are not in the restock need (`restock::shortfall` counts `Place` cells); the first
  placement restocks the whole count.
