# Selection shapes

`acacia-world` `BlockState::selection`: the boxes a crosshair ray hits, which are not the collision boxes. The bot
blocks its line of sight on collision ∪ selection and aims at selection (`acacia-bot` `interact/sight.rs`).

## Source

A static read of BDS 1.26.52.3 (`bedrock_server.exe`, Windows) with bdsre. Nothing was run or debugged. The full
method, per-family evidence addresses and the generator live outside the repo in the workspace's
`research/bds-selection-shapes/` (README, `tools/gen_shapes.py` → `shapes.json`).
`crates/acacia-world/tools/gen-selection.py <shapes.json>` packs that into `data/selection.bin` in `blocks.bin`
order. Regenerate both after `blocks.bin` changes; the registry asserts that the state counts match.

- The pick ray clips `ShapeType::Outline`: `BlockType::clip` (vt+0x18) → `getOutline` (vt+0x40) → `getVisualShape`
  (vt+0x50), which by default is the ctor's `mVisualShape` (unit cube unless set). JSON `minecraft:selection_box`
  is used only by `red_shrub` and `shelf_mushroom`.
- Coverage: 20543 states from ported overrides or ctor constants, 1548 default unit cubes, 0 fallbacks.
- 17148 states differ from the collision boxes, for example:
  - walls are 0.8125 or 0.9999 tall, not 1.5;
  - fences and gates are 1 tall;
  - panes are one bounding box;
  - hanging signs are a full-height 1/4 slab;
  - corner stairs select as straight stairs;
  - flowers select `[.15,0,.15,.7,.6,.7]` (off-centre in the exe) and have no collision;
  - torches, rails, buttons, signs and crops have small boxes and no collision.

## Random offset

Flowers, roots, corals, double plants, bamboo, propagule and eyeblossoms shift by ±0.25 on x and z. Short grass,
fern and dry grass also shift by -0.2..0 on y. `BlockState::selection` holds the unshifted box and
`selection_offset.at(x, z)` gives the shift: BDS's position hash (144c50eb0), ported statically. Its test vectors
come from the research port, not from a live client, so it is unconfirmed.

## Neighbours

`acacia_world::selection::boxes_at(world, registry, pos)` applies the offset and these rules. Use it rather
than `BlockState::selection` wherever the world is at hand.
- **Doors:** direction and open come from the lower half and the hinge from the upper. A half with no pair reads
  all three as 0.
- **Vines with no face bits:** a top slab if the block above qualifies, a bottom slab if only the block below
  does, otherwise the cube. BDS also excludes one block it identifies by a global (14ccf9950) that is still
  unidentified, so here "qualifies" means a full cube that isn't `invisible_bedrock`.
- **Twisting and weeping vines:** the full column when the same vine is above (twisting) or below (weeping).
- **Chorus plant:** each side reaches the edge where a chorus plant or flower joins; below, end stone counts too.
- **Scaffolding:** the bottom beams are dropped when the block below supports it. That's approximated as
  scaffolding below or a full top face; BDS asks the block's edge support.
- **Portal with `portal_axis=unknown`:** the x box if a portal is at x±1, otherwise the z box.

## Not modelled yet

- **Shapes that need a block actor** (left out on purpose):
  - A piston arm's boxes shift by `(progress - 1) · facing`. At rest (progress 1) that equals the table shape, so it
    differs only during the 2-tick push.
  - A moving block exists only during that push.
  - A shulker box is the closed unit cube. The open lid comes from the client's animation, not from NBT.
  - A chalkboard's shape is unknown without its actor.
- **Unconfirmed:** whether the client's pick skips `mayPick() == false` blocks. The table assumes it does, so air,
  fire, liquids and bubble columns are empty.
