# Unported small rules (README "Not ported"): what they are, whether a client needs them

2026-11-12. Sources:
- bedsim: no checkout in the repo. A clone at commit `9c12b34` (2026-10-07, newer than the ported `34d11dc`) was
  in another session's scratchpad (`/tmp/claude-1001/.../d52a1fe5-.../scratchpad/src/bedsim`); its line numbers
  are quoted here. To get a lasting copy, `git clone https://github.com/oomph-ac/bedsim` into `research/`.
  cinnabar's `tools/bedsimtrace` pins v0.1.3–v0.1.5 and sets `IgnoreClientStepTiebreaker: true` too.
- BDS 1.26.52.3. Linux ELF `~/work/bds-ulp/bedrock_server` (byte-identical to `bds-pool/N/bedrock_server`), with
  addresses from ELF RTTI (`sapling-*/crates/sapling-worldgen/scripts/bds_elf.py`) and r2 disassembly. Windows
  addresses (`1…`) come from bdsre on `bds-blocks/bedrock_server.exe`.
- Slot layout of `GetCollisionShapeInterface` (LeviLamina header, Itanium): +0x10 getFeetAttachPosY,
  +0x18 canDescendThroughBlock, +0x20 getFallDistance, +0x28 getEntityTypeId, +0x30 isWearingLeatherBoots,
  +0x38 canStandOnPowderSnow, +0x40 getAABB. The actor proxy is `GetCollisionShapeActorProxy`, vtable 0xf2478a0.

## 1. Step tie-breaker
- **Rule** (bedsim `simulation.go:1229-1259`): this is bedsim's reconciliation step. A step-up that the sweep allows
  (no boxes at the stepped box, and more horizontal distance than the plain collision) is rejected when the
  *client-reported* position is closer to the non-step result and the non-step result is within
  `correctionThreshold`: `accept = Ignore || |collisionPos-clientPos| > thr || |stepPos-clientPos| <= |collisionPos-clientPos|`.
- **Client?** No. It uses the client's own claimed position to resolve a server-side ambiguity. BDS has no such
  input: it steps from its own sweep. The crate always accepts (`collide.rs:83-84`), as BDS does, and the
  step-ups pass in the fuzz and drills.
- **Port:** none. Delete the item from the README and gaps lists. **Verdict: irrelevant-for-client.**

## 2. Slide offset
- **Rule** (`constants.go:12` `SlideOffsetMultiplier = 0.4`; `simulation.go:1243-1283`; `bbox.go:66-131`, behind
  `Options.UseSlideOffset`): this is an old-client `ySlideOffset`. On a completed step,
  `slide = slide*0.4; slide.y += stepVel.y`. The box is raised by `slide.y`, and the final Y is
  `endPos.y -= slide.y`. A tick with no step resets it to 0.
- **Client?** No. The 1.26.52 BDS has no 0.4 constant in the code that reaches `MoveCollisionSystem` or any step
  system (`bdsre f32 0.4 "MoveCollision|AutoStep|Step"` finds only `ActorPostAiStepTickSystem`). LeviLamina's
  headers have no slide-offset field. bedsim keeps the option off by default, for legacy clients.
- **Port:** none. **Verdict: irrelevant-for-client.**

## 3. Legacy sprint timing
- **Rule** (`simulator.go:23-29`; `simulation.go:48-50` and `applyLegacySprint` at `simulation.go:487-507`): in legacy
  mode the speed change from StartSprinting/StopSprinting applies *after* the tick's movement, for old clients.
  Modern mode (`simulation.go:286-313`) applies it in `applyInput`, before the move.
- **Client?** No. The crate already uses the modern order (`input.rs` `apply_input` → `refresh_movement_speed`),
  and the strict-BDS sprint drills (`sprint`, `fracsprint`, `turnsprint`, ...) run with that order. This report
  did not re-run them.
- **Port:** none. **Verdict: irrelevant-for-client.**

## 4. Server-forced sprint
- **Rule** (`simulation.go:292-305`, fields in `movement.go:87`): when an input carries neither StartSprinting
  nor StopSprinting and the server's sprint flag (`ServerSprint`, from its SetActorData) differs from the
  simulated state, the simulation adopts the server's flag once (`ServerSprintApplied`). This is a tolerance for
  the server flipping the sprint under the client.
- **Client?** Yes, in a different form. Under strict BDS the server owns the sprint. When it refuses or ends a
  sprint the client keeps predicting, the client draws a stream of corrections. This happened live: food ≤ 6
  produced 388 and 1,416 corrections (memory `strict-bds-far-coordinates`). The fix (`PlayerState::hungry`, the
  sprint gate `fed = !hungry || may_fly` in `input.rs`, and `packets.rs` setting `hungry` from
  `minecraft:player.hunger` ≤ 6) lives on branch `pathfind/ulp-drift` (`acacia-pathfind-drift`), **not on
  `physics/gaps`**. The other server-side stop conditions are the vanilla-client rules in BDS
  `IntentSprintTriggerSystem::fn0` (1483ebc00): item-use slowdown, forward input < 0.7071, the water/ground gates,
  and `SprintTimerSystem` (1483e95f0). The crate already models the movement-blocked and swim cases.
- **Port:**
  1. Bring in the hunger gate from `pathfind/ulp-drift`: `state.rs` `hungry` plus the `fed` term in
     `input.rs` (`wanted = self.sprint && enough && fed && !sprint_movement_blocked`).
  2. Add a guard in acacia-bot `movement/packets.rs`. When our own `SetActorData` clears flag SPRINTING (byte 0,
     bit 3 of MetadataFlags1) while `st.sprinting`, set `st.sprinting = false` and `st.refresh_movement_speed()`
     before the next tick, and log it (`tracing::debug!`). Every such log line names a server stop rule we have
     not modelled yet.
- **Drill:** add `hungrysprint` to `drills.rs`: `/effect @s hunger 30 255` (via `effect_of`), then sprint 200 ticks
  along the pad. Expect 0 corrections with the gate and hundreds without it. Also add `blindsprint` (blindness
  given mid-sprint) to find out whether BDS stops sprinting under blindness.
- **Verdict: port** (the hunger gate is already proven). The SetActorData guard **needs-live-evidence**: it should
  never fire in the drills above.

## 5. Crawl input flags
- **Rule** (`input.go:44-45`; `simulation.go:348-367`): StartCrawling enters the crawl pose (0.6 high) only when
  standing does not fit. StopCrawling leaves it for standing, or for sneaking with sneak held, when that fits;
  otherwise the crawl continues. The crawl pose then takes the sneak slowdown (`simulation.go:474`).
  The wire bits are PlayerAuthInput 40/41; the actor flag is `Crawling` = 114.
- **Client?** Partly. The crate enters a crawl only through `restore_upright_pose` (`pose.rs:164-183`), after a
  swim or glide ends under a low ceiling, and leaves it on any tick where standing or sneaking fits
  (`input.rs:171`, a deviation fitted on strict BDS). acacia-bot never sends Start/StopCrawling
  (`movement/auth_input.rs`), yet the README lists these crawl rules as fitted on strict BDS without the flags.
  So in those paths BDS evidently derives the pose itself, and the flags only matter for fidelity (the input fingerprint, and proxies such as
  oomph that read them). Two things are untested:
  - **(a)** A crawl entered from standing, for example a block placed over the player's head: standing (1.8) and
    sneaking (1.49) both fail to fit. bedsim and the vanilla client switch to crawl on StartCrawling. The crate
    keeps the sneak box inside the ceiling (`input.rs` keeps sneaking when standing does not fit; nothing tries
    the crawl height).
  - **(b)** Whether BDS needs StartCrawling to apply the crawl slowdown, or derives it.
- **Port:**
  - In `input.rs` `apply_input`, after the sneak block: if `!st.crawling && !st.swimming && !st.gliding` and
    neither standing nor sneaking fits (`can_fit` both false, known), then set `st.crawling = true`,
    `st.sneaking = false`, `st.size[1] = st.crawling_height`.
  - In acacia-bot `auth_input.rs`, add edges `crawl: (bool, bool)` from `st.crawling` changes, as is already done
    for swim (`movement/mod.rs:214-241`), and set `F::StartCrawling`/`F::StopCrawling`.
- **Drill:** add `crawlenter` to `drills.rs`. Stand at x+1.5, run `/setblock ~ ~1 ~ stone` above the head (through
  `pad.run`), wait 5 ticks, then walk forward 40 ticks into a 1-high tunnel (stone at y+1 from x+2 to x+8).
  Compare corrections with and without the input flags. A vanilla capture of the same scene (does the client
  send StartCrawling?) settles (b).
- **Verdict: needs-live-evidence**, then port the entry rule and the flags.

## 6. `AutoJumpingInWater`
- **Rule** (`input.go:37`; `simulation.go:387, 424`): `EffectiveJumping = Jumping || AutoJumpingInWater ||
  AscendBlock`. It drives liquid ascent like a held jump key. It is wire bit 7, and `MoveInputComponent::Flag`
  5 in BDS (`mHoldAutoJumpInWaterTicks`). The vanilla client sets it from its own auto-jump-in-water assist,
  only when the auto-jump setting is on.
- **Client?** No. It is a second way to say "jump held in water". The bot asks for ascent with Jumping, which
  every rule already reads, and it never sends this flag, so BDS sees nothing to interpret. bedsim keeps it so
  that it can validate vanilla clients that use the assist.
- **Port:** none. To mimic a touch client's fingerprint later, set the flag only on ticks where the crate already
  holds jump in water (`st.pressing_jump && in water`). **Verdict: irrelevant-for-client.**

## 7. Scaffolding's unsupported-bottom lip
- **Origin:** Java `ScaffoldingBlock.getCollisionShape` has `UNSTABLE_SHAPE_BOTTOM` (0..0.125 high) for a
  `bottom` scaffold when the entity is above `pos - 1`. bedsim does not have it: the shape comes from the
  host's `MovementCollisionProvider` (`interfaces.go:42-58`, `simulation.go:1597-1617`). The note entered the
  README with `d43fd69` as a Java comparison.
- **BDS** `ScaffoldingBlock::getCollisionShape` (ELF 0xcf40ac0, vtable slot 4; `clip` override 0xcf40d50):
  1. If the block state at 0xf5fe030 (most likely `stability`, the first state registered at 0x5a892df)
     equals 7, the shape is empty.
  2. Else, if there is an entity, `!entity.canDescendThroughBlock()` (ActorFlag `DescendThroughBlock` = 71,
     proxy slot +0x18 → 0xbe64930), `pos.y + 1 + (-1e-6) <= aabb.min.y`, and the entity's *current* AABB
     overlaps the cell strictly in x and z (`pos.x < max.x && pos.x+1 > min.x`, likewise for z), the shape is
     `pos + 0xf62b528` (0xcf40cf0). That is a widely copied static AABB, read as the unit block.
  3. Otherwise the shape is empty.
  4. `addCollisionShapes` is not overridden, so movement sees only this shape.
- **There is no bottom lip in BDS**, so the crate's omission is correct. Small differences remain in
  `pose.rs:145-147` `collides_for` (`st.pos.y > top - 1e-5 && !pressing_descend`):
  - The tolerance is 1e-5 where BDS uses 1e-6, and the comparison is `>` where BDS uses `>=`. They differ only for
    feet sitting 1e-6 to 1e-5 under the top.
  - BDS gates on the DescendThroughBlock flag, the crate on the descend key.
  - BDS requires horizontal overlap with the *pre-move* box. The crate also collides with scaffold cells that
    only the swept area reaches, such as a fast lateral landing onto a cell the box is not over yet.
  - BDS has the stability-7 exception.
  - The registry's top slab plus legs (`acacia-world/tools/rules.mjs` `SCAFFOLDING_TOP`) and BDS's full cube are
    equivalent for a player who is above the top.
- **Port:**
  - Fix the README and gaps list: "BDS has no lip".
  - Optional: in `collides_for`, add BDS's strict x/z overlap of `st.bounding_box()` with the cell. Leave the
    threshold and gate alone without evidence.
- **Drills:**
  - `scafflip`: a stone lane at y0, z+1, beside a scaffold arm at y1, z0 (air under the arm; the arm hangs off a
    2-high scaffold tower at x+2). Walk +x at z+1.25, so the box overlaps the arm's cells at feet level. Expect
    0 corrections with the current crate, which confirms there is no lip.
  - `scaffside`: sprint-jump off a 2-high stone pillar towards a 1-high scaffold floor 2 cells away. This checks
    the pre-move-overlap rule, and is a candidate for the residual scaffold fuzz mismatches.
- **Verdict: irrelevant-for-client** (the lip is Java only). The overlap tweak **needs-live-evidence**.

## 8. Powder snow's player-dependent shape
- **BDS** `PowderSnowBlock::getCollisionShape` (ELF 0xcf596f0, slot 4):
  - If there is an entity, `top = pos.y + visualShape.max.y` (that is, +1), and
    `top + (-1.1920929e-7) <= entity.getFeetAttachPosY()` (proxy +0x10 → 0xbe64910) and
    `!entity.canDescendThroughBlock()`:
    - if `entity.getFallDistance() > 2.5` (const 0x109845c; proxy +0x20 → `FallDistanceComponent`), the shape is
      `pos + (0,0,0)-(1,0.9,1)` (static 0xf459fa8, filled at 0x5a1eb6e from 0x13e41d0 and 0x13e53e0 = 0.9/1.0);
    - else, if `getEntityTypeId() == 66` (falling_block) or `canStandOnPowderSnow()` (+0x38 →
      `CanStandOnPowderSnowComponent`), the shape is `pos + unit cube`.
  - Otherwise the shape is empty.
  - Players get `CanStandOnPowderSnowComponent` through `CanStandOnPowderSnowFromEquipmentComponent` (Windows
    14252e4d0 and its siblings 142528950, 14252b3e0, 14252ebe0), meaning leather boots, as in Java.
    `isWearingLeatherBoots` (+0x30) is not read here.
  - The fall check comes first: even with boots, a long fall lands on the 0.9 slab. On the next tick the feet are
    under the top, the shape is gone, and the player sinks. With boots and a short fall the player stands on top
    until descending.
- **bedsim:** delegates the shape to the host's `GetMovementBBoxes(aabb, ctx{LeatherBoots, Descending,
  DescendThroughBlock, ...})`. Its tests only stub "leather && !descending → cube" (`dynamic_collision_test.go`).
- **Crate today:** **not handled anywhere.** `RegistryWorld::block_collisions` returns the registry boxes, and
  `rules.mjs` gives `powder_snow: []`. acacia-bot uses `RegistryWorld` directly (`world/adapter.rs`) and adds
  nothing. `TestWorld` also gives none. Only the inside effects (stuck multiplier, freeze, leather-boots ascent)
  exist (`block_effects.rs`).
- **Client?** Yes. Snowy slopes and groves have powder snow fields:
  - A fall of more than 2.5 blocks onto one lands for a tick on BDS; the crate falls straight in, which costs a
    correction at least, and also affects the landing's `on_ground` and fall damage.
  - A bot wearing leather boots walks on top on BDS but sinks in the crate, giving a stream of corrections.
- **Port** (in-crate, so every `WorldView` gets it), in `pose.rs`:
  - Add `fn player_boxes(&self, st, cell, out)`, which calls `w.block_collisions(cell)` and then, when
    `w.block(cell).traversal == Traversal::PowderSnow && st.pos[1] >= cell.y + 1 - 1.1920929e-7 &&
    !st.pressing_descend`:
    - pushes `(0,0,0)-(1,0.9,1)` if `st.fall_distance > 2.5`,
    - else pushes the unit cube if `st.equipment.leather_boots`.
  - Use it in `nearby_bboxes` (append the powder snow cells of `area`, since `w.collisions` misses them) and in
    `find_supporting_block` and `standing_on_block` (`collide.rs:186-232`).
  - The descend gate follows the scaffolding precedent (`pressing_descend`). BDS uses the DescendThroughBlock
    flag; a drill settles it.
  - `st.fall_distance` must be the value from before this tick's update, which is BDS's component value during
    the move.
- **Drills:**
  - `powderfall2`/`powderfall3`/`powderfall5`: a powder snow pit 3 deep (y-2..y0) under the pad, dropped from
    feet y = 2, 3, 5 above its top (FALL steps). Expect: 2 sinks straight in; 3 and 5 land one tick at +0.9,
    then sink. Today expect at least one correction on 3 and 5; after the port, 0.
  - `powderboots`: the same pit, with `/replaceitem entity @s slot.armor.feet 0 leather_boots` (a `drills.rs`
    prefix hook like `effect_of`). Start on top, walk 40 ticks, sneak 20 ticks (should sink), then jump with
    jump held (ascent 0.2, `block_effects.rs:53`).
  - `powderbootsfall`: boots plus a drop from 5, which checks that the fall slab wins over boots.
- **Verdict: port.**

## 9. No-clip (spectator)
- **bedsim:** does not simulate it. `simulationIsReliable` (`simulation.go:792-804`) returns false for NoClip,
  flying, dead or non-survival players, and `resetToClient` (`simulation.go:806`) accepts the client's position.
  `applyStuckSpeedMultiplier` drops stuck multipliers under NoClip (`block_effects.go:37`).
- **BDS:** `NoClipOrNoBlockMoveFilterSystem::fn0` (Windows 148265630): for an actor *without* ActorFlag
  `HasCollision` (48, byte 6 bit 0 of `ActorDataFlagComponent`), the move is a plain translation of the AABB.
  The position becomes the box's x/z centre and `min.y + offset`, with a sanity check for |x|,|z| > 2^25 or
  non-finite values. There is no sweep and no step. A spectator also has the NoClip ability (AbilitiesIndex 17,
  `MovementAbilities::NoClip`), and flies.
- **Client?** Only if bots run in spectator mode: observer or camera bots, or a `/gamemode spectator` admin
  script. Today the crate collides with walls and the floor that BDS passes through, so every wall contact
  gives a correction.
- **Port:**
  - `PlayerState.no_clip: bool`. acacia-bot sets it in `movement/flight.rs` `set_abilities` from
    `player.abilities.no_clip`, which `state/abilities.rs` already parses (more precisely: from our
    SetActorData losing `has_collision`).
  - In `collide.rs`, when `st.no_clip`: `end = pos + vel`, no step, `on_ground = false`, no horizontal or
    vertical collision, `supporting_block = None`, and the box rebuilt from the new position.
  - Skip inside-block effects (stuck multipliers, as bedsim does) and the edge avoidance.
- **Drill:** add `spectatorwall` to `drills.rs`: `/gamemode spectator`, then fly +x (the flight toggles through
  `fly_jump`) through WALL3 and down through the pad floor. Expect 0 corrections after the port. Restore
  `/gamemode creative` afterwards. Also check whether BDS still applies flight friction in spectator; compare the
  deltas.
- **Verdict: port if spectator bots are in scope; otherwise irrelevant-for-client.** Physics details (friction,
  vertical speed) **need-live-evidence**.

## Summary
| Item | Verdict |
|---|---|
| Step tie-breaker | irrelevant-for-client |
| Slide offset | irrelevant-for-client |
| Legacy sprint timing | irrelevant-for-client |
| Server-forced sprint | port (hunger gate from `pathfind/ulp-drift`); SetActorData guard needs-live-evidence |
| Crawl input flags | needs-live-evidence (crawl entered from standing, flags), then port |
| `AutoJumpingInWater` | irrelevant-for-client |
| Scaffolding lip | irrelevant-for-client (not in BDS); overlap tweak needs-live-evidence |
| Powder snow shape | port (crate-side; not handled in the adapter or the bot) |
| No-clip | port if spectator is in scope |
