# Backend split: actions slice (interact, items, build, survival, workstation, swarm, misc)

Recon for step 4 of docs/java-edition.md, 2026-11-10, branch java/edition. Paths are relative to
`crates/acacia-bot/src/` unless noted. Java references are to `~/java-truth/sources/client/net/minecraft/`.

## Shared seams this slice depends on (outside the slice)

- `Bot.client: acacia_client::Client` (bot.rs:42). Every `self.client.send(&BedrockPacket)` in this slice is
  the direct coupling.
- `Bot::next_tick(check: FnMut(&Bot, &RawPacket) -> bool)` (wait.rs:92) and `wait_until` closures take
  Bedrock `RawPacket`. Most closures in the slice ignore the packet (`|bot, _|`); the ones that decode it are
  the real work (confirm.rs:132, pipeline.rs:156, items/request.rs:135, sleep.rs:108, forms, lag.rs).
- `Movement.pending_actions: Vec<PlayerAuthInputBlockActionItem>` (movement/mod.rs:39), drained into
  `PlayerAuthInput` at bot/tick.rs:135. Breaking writes its block actions here.
- `state::ItemStack` (state/inventory/stack.rs:8) is Bedrock-shaped: `network_id`, `metadata`,
  `block_runtime_id`, `stack_network_id`, Bedrock `nbt`. `state.items.name(network_id)` is the StartGame
  item table. `state.player.game_mode` and `Container.window_type` are Bedrock proto enums.
- Java codecs: `acacia-java-proto/src/ids.rs` has ids for PLAYER_ACTION 41, USE_ITEM_ON 66, USE_ITEM 67,
  SET_CARRIED_ITEM 54, ATTACK 1, INTERACT 26, CONTAINER_CLICK 18, CONTAINER_CLOSE 19, SIGN_UPDATE 62,
  EDIT_BOOK 24, clientbound BLOCK_CHANGED_ACK 4, BLOCK_UPDATE 8, SECTION_BLOCKS_UPDATE 86, but
  `packets.rs` has no structs for any of them yet. There is no serverbound swing id: 26.3 has no
  `ServerboundSwingPacket` (see below).

## 1. Digging

### Bedrock today
- Entry: `Bot::break_block` interact/breaking.rs:36 (async, confirmed) and the hold-to-mine API
  `start_mining`/`tick_mining`/`stop_mining` interact/direct.rs:62/122/96.
- Wire: `PlayerAuthInput` block actions only, pushed into `movement.pending_actions`:
  `wire::break_step` interact/wire.rs:244 = `StartBreak` on tick 0, `ContinueBreak` after, and on the last
  tick `PredictBreak` (survival only) + `AbortBreak` (BDS keeps mining the cell otherwise).
  `abort_break`/`end_break` breaking.rs:98/106 push `AbortBreak`. Swing = `Animate` SwingArm with source
  "mine" every 5 ticks (breaking.rs:123, item_use.rs:27, wire.rs:97). No InventoryTransaction is involved.
- Break time: `break_ticks` interact/break_time.rs:32 + `Progress` (break_time.rs:95). The formula is
  vanilla-ish and edition-neutral in shape (inputs: `acacia_world::Mining`, `BreakConditions`), already
  documented as "Java 1.21 sword/shears speeds, never faster than Bedrock". The **inputs** are not neutral:
  `Bot::break_conditions` breaking.rs:142 reads tool via `state.items.name(held.network_id)`, Efficiency /
  Aqua Affinity via Bedrock NBT `ench` list with Bedrock ids 15/8 (`enchantment_level` breaking.rs:163),
  game mode via proto `GameMode` (direct.rs:74).
- Confirmation: `Confirm`/`Expect::Gone` interact/confirm.rs:54; `await_confirm` confirm.rs:167 ticks until
  `is_update_at` (confirm.rs:132: decodes `UpdateBlock`/`UpdateBlockSynced`/`UpdateSubchunkBlocks` at the
  cell) and the tracked cell matches; retry up to 3 (`MAX_TRIES` breaking.rs:24), governor timeout and RTT
  sample. `Confirm::check` itself is network-free.

### Java equivalent (MultiPlayerGameMode.java, Minecraft.java, ServerPlayerGameMode.java)
- `ServerboundPlayerActionPacket(START_DESTROY_BLOCK, pos, dir, seq)` on the first tick (GameMode:153-208).
  Creative: START alone destroys (with `destroyDelay = 5`). Instant blocks (progress >= 1 at start): START
  alone breaks.
- Each later tick `continueDestroyBlock` (GameMode:232) adds `getDestroyProgress`; at >= 1.0 sends
  `STOP_DESTROY_BLOCK(pos, dir, seq)` and sets `destroyDelay = 5` (5 idle ticks before the next block).
  Changing target sends `ABORT_DESTROY_BLOCK` (no sequence on that path, GameMode:180/224).
- Swing: 26.3 replaced the serverbound swing with `ServerboundPunchPacket` (unit, no hand), sent each tick
  `continueDestroyBlock` returns true (Minecraft.java:1605-1606) and on attack (1684-1685).
  Right-click swings are client-local only (no packet; Minecraft.java:1717-1747).
- `ensureHasSentCarriedItem` (GameMode:310) sends `ServerboundSetCarriedItemPacket` lazily before any action.
- Server check: STOP accepted if `progress * (ticksSinceStart + 1) >= 0.7` (ServerPlayerGameMode:241), so
  the bot's tick count can be a little short without a reject; Grim checks closer.
- Confirmation: every predicted action carries a client sequence; the server answers
  `ClientboundBlockChangedAckPacket(seq)` from `tick()` (ServerGamePacketListenerImpl:302) plus
  `BlockUpdate`/`SectionBlocksUpdate`. Vanilla's `BlockStatePredictionHandler` keeps server states for
  predicted cells until the ack, so the order of update vs ack does not matter.
- Break progress per tick: Java `getDestroyProgress` = destroySpeed / hardness / (30|100) with 26.3's
  attribute-based modifiers (mining efficiency, submerged/airborne mining speed attributes, haste/fatigue);
  counting starts at START (tick 0 contributes nothing on the client, progress accumulates from the first
  continue tick). Needs its own `break_ticks` variant or a `BreakConditions` filled from Java attributes.

### Where the seam goes
- Keep `break_block`/`break_confirmed`/`mine` (breaking.rs) and `start_mining`/`tick_mining` (direct.rs)
  as shared drivers; replace the three edition-specific calls with backend methods:
  `backend.break_step(first, last, pos, face, creative)` (Bedrock: push to `pending_actions`; Java: send
  PlayerAction START/STOP with a fresh sequence, nothing on middle ticks), `backend.abort_break(pos, face)`,
  `backend.swing(source)` (Java: Punch only for Mine/Attack, nothing for Build/Interact/UseItem).
- `Progress`/`break_ticks` stay neutral; move `break_conditions` inputs onto neutral accessors
  (`ItemStack::tool()`, `ItemStack::enchantment(Enchantment::Efficiency)`, neutral `GameMode`). Java needs a
  per-edition tick-count rule (start tick does not count, `destroyDelay` 5 after a finished break) — put that
  in the backend's `break_step` schedule, not in the driver.
- Confirmation: replace the `&RawPacket` probe in `await_confirm` (confirm.rs:171) and `note_echo`
  (pipeline.rs:156) with a neutral per-tick "cells echoed" set the edition's world tracker fills
  (Bedrock: the three update packets; Java: BlockUpdate/SectionBlocksUpdate, and an action is "echoed" when
  `ack >= its seq`). `Confirm`, `Expect`, `Verdict`, `Pipeline` need no change.

## 2. Placing / using items on blocks and entities

### Bedrock today
- `click_block` interact/item_use.rs:221 is the one block-click path (used by `use_item_on_block`:47,
  `door_confirmed`:59, `use_confirmed`:83, `place_block_at`:114, `place_confirmed`:173, `place_sent`
  pipeline.rs:118, `right_click_block` direct.rs:147). Sends, in order: `Interact` MouseOverEntity(none) if
  hovering, `PlayerAction StartItemUseOn`, `Animate` swing, `InventoryTransaction` UseItem ClickBlock
  (wire.rs:134) — for builds wrapped with the legacy slot-change action list (`legacy::with_slot_change`
  legacy.rs:52) and a legacy request id — then `MobEquipment` (`equip_now`), ClickAir after a sign, and a
  delayed `StopItemUseOn` via `Reflex::StopUseOn` (item_use.rs:247).
- It needs the clicked block's server wire id (`block_wire_id` interact/mod.rs:121) and the held item as
  `ItemV4` (`to_wire` wire.rs:26, `Bot::hand` mod.rs:102).
- Prediction: `predict_block` confirm.rs:145 writes `held.block_runtime_id` into the tracked world;
  `unpredict`/`give_back` confirm.rs:115/123 restore the stack (compare `network_id`/`stack_network_id`).
  "Does the held item place a block" = `block_runtime_id != 0` (item_use.rs:192, :255; direct.rs:150).
- Air use: `use_item` = ClickAir (item_use.rs:35, wire.rs:141), `release_item` = ItemRelease (wire.rs:223),
  eating `finish_use` (wire.rs:147) + `EntityEvent EatingItem` (survival/consume.rs:62) + `InputData::
  StartUsingItem` flag (consume.rs:48, fishing.rs:81).
- Entities: interact/entity.rs:243-293 sends `Interact` MouseOverEntity (wire.rs:170) then
  `InventoryTransaction` UseItemOnEntity Attack/Interact (wire.rs:203) and swings.
- Hotbar: `select_hotbar`/`equip_now` interact/equipment.rs:13/23 send `MobEquipment` (+ mouse-over-nothing).

### Java equivalent
- Block click: `ServerboundUseItemOnPacket(hand, BlockHitResult{pos, face, location, inside, worldBorder},
  seq)` (GameMode:318). `location` is the world-space hit point (server rejects > 1.0000001 from block
  centre per axis, ServerGamePacketListenerImpl:1415-1417); sneak state comes from PlayerInput, not the
  packet. No StartItemUseOn/StopItemUseOn, no mouse-over, no held-item copy, no slot-change list.
  A refused use makes the server resend `BlockUpdate` for the clicked cell and its neighbour, then the ack.
- Air use: `ServerboundUseItemPacket(hand, seq, yRot, xRot)` (GameMode:383); release:
  `PlayerAction RELEASE_USE_ITEM` (GameMode:524); eating finishes server-side (no finish packet).
- Entities: attack = `ServerboundAttackPacket(entityId)` (GameMode:433) + Punch; interact =
  `ServerboundInteractPacket(entityId, hand, location - entityPos, sneaking)` (GameMode:451).
- Hotbar: `ServerboundSetCarriedItemPacket(slot)`; offhand swap `PlayerAction SWAP_ITEM_WITH_OFFHAND`.

### Where the seam goes
- Backend method `click_block(pos, face, hit_point, source, placing)` replacing item_use.rs:227-247 body.
  The shared part (`aim_placement`, reach/visibility checks, `Confirm`, prediction and give-back) stays.
  `block_wire_id` is Bedrock-only input; the Java backend ignores it.
- `use_item`, `release_item`, `click_entity_aimed`'s two sends, `equip_now`, `swing_from` → backend.
  The mouse-over bookkeeping (`reflexes.hovered`) and `Reflex::StopUseOn` are Bedrock backend state.
- Prediction needs neutral item facts: "places block X" (Java: BlockItem → default state of that block,
  better: the state Java's placement logic would pick; until then, predict nothing on Java and wait for the
  ack), and stack identity for `give_back` (Java has no stack ids; compare item + components).

## 3. Inventory / items

Bedrock ItemStackRequest-bound (defer to step 5):
- items/request.rs (encodes `Op` → `ItemStackRequestActionsItem`, request ids, `response_for` decode),
  items/plan.rs (predict/commit by `stack_network_id`, `THIS_REQUEST` marker plan.rs:93/207),
  items/slot.rs:111-135 (`ContainerSlotType`/`FullContainerName`/`StackRequestSlotInfo`), items/ui.rs (UI
  window 124 offsets), items/craft.rs (CraftRecipe/Auto/Creative/Loom/Grindstone + ResultsDeprecated),
  items/actions.rs (`run_plan`, `send_request`, open/close own inventory via `Interact OpenInventory` /
  `ContainerClose`), items/quick.rs and items/click.rs (pure planners over `Op`/`SlotRef`; neutral logic,
  but only meaningful once a Java executor exists).
- Already neutral API surface to keep: `SlotRef`, `Op`, `Click`, `move_item`/`swap_items`/`drop_item`/
  `quick_move`/`take_all` signatures (items/actions.rs:34-84). Step 5 swaps the executor
  (Java: `ServerboundContainerClickPacket` with state id + carried item + changed slots).

Step 4 must touch:
- Selecting a hotbar slot and telling the server (equipment.rs: `select_hotbar`, `equip_now`).
- Knowing the held item: inventory tracking from Java `ContainerSetContent`/`ContainerSetSlot`
  (window 0, hotbar = slots 36-44), `SetHeldSlot`; mapped into the same `Inventory`/`ItemStack`.
  `ItemStack` needs neutral item identity (`Item` enum) beside or instead of `network_id`/`metadata`, a
  neutral "block it places", and neutral enchantment lookup. Everything in survival/tools.rs, equip.rs,
  breaking.rs, item_use.rs that calls `state.items.name(network_id)` reads through that.
- Tool choice for digging (survival/tools.rs:24 `best tool slot`) then `equip_hand_paced`
  (survival/equip.rs:152): hotbar-only selection works in step 4; bringing a tool up from the main
  inventory goes through `move_item`/`swap_items` (equip.rs:143/145) = step 5. Cheap stop-gap for Java:
  a single `ContainerClick` mode SWAP (button = hotbar index) inside the player window, or error with
  `NotPossible` until step 5.
- Block placing consumes from the held stack locally (`one_used` item_use.rs:268); Java has no legacy slot
  list, so the backend just sends the click and the shared code keeps predicting the count.

## 4. build/, survival/, workstation/, swarm/

- **build/**: neutral. Plans against `pathfind::process::{Act, Ctx, Look, BlockView}` with string item
  names (`Act::Place/Break/Use/UseOn/Fill/Restock/Sign`, pathfind/process/mod.rs:49-66); no `self.client`,
  no ids. Only direct import: build/structure.rs:5 (`proto::nbt` to read `.mcstructure` little-endian
  NBT; a file format, not a packet — switch to acacia-nbt; palette names are Bedrock, which step 1's neutral
  `Block` handles). Java parity of build depends on the `Act` executor in pathfind (other slice), plus
  Restock (containers, step 5) and Sign (signs.rs backend method).
- **survival/**: mixed.
  - consume.rs:1-2 — sends ClickAir, `EntityEvent EatingItem`, finish ClickAir; `InputData` flag; proto
    `GameMode`. Needs backend `start_use/finish_use`.
  - auto_eat.rs:7 — only proto `GameMode` (neutral enum).
  - fetch.rs:7 — raw `ItemStackRequest` flow via `send_ops` + `response_for` on `RawPacket` (step 5).
  - item_use.rs (no import) — `eating_data` (network_id<<16|meta) for Bedrock EntityEvent; Bedrock-only.
  - equip.rs, tools.rs (no import) — compare `network_id`; go through Bot methods otherwise.
- **workstation/**: all Bedrock ItemStackRequest (craft/beacon/loom/grindstone/anvil/enchant/trade/
  smithing/cartography/brewing/furnace), Bedrock `WindowType`, Bedrock item NBT (`Value` for enchants,
  repair cost, banner patterns) and recipe network ids from CraftingData. Entirely step 5.
- **swarm/**: connection layer. login.rs:4-5, builder.rs:7-8, supervisor.rs:6, policy.rs:4-6 use
  `ClientBuilder`, `realm_builder`, auth types, `ConnectError`, `DisconnectReason`, `PlayStatusStatus`.
  Needs the edition-chosen connect (`Connect::java/bedrock`) and neutral connect/disconnect error types;
  retry policy logic (policy.rs) is neutral once the error enum is.

## 5. Classification of slice files that import acacia_client / acacia_proto

(a) neutral logic, import move only · (b) needs a backend method · (c) Bedrock-only, hang off `bot.bedrock()`
· (5) Bedrock inventory machinery, replaced in step 5 · (t) test, stays a Bedrock backend test

| File | Class | Reason |
|---|---|---|
| interact/breaking.rs | b | pushes `AbortBreak` / `break_step` into PlayerAuthInput; NBT `ench` enchant read → neutral accessor |
| interact/direct.rs | b | same block actions for hold-to-mine; proto `GameMode` → neutral |
| interact/item_use.rs | b | `click_block` packet sequence, ClickAir, ItemRelease, Animate swing |
| interact/wire.rs | b | all Bedrock packet builders; becomes the Bedrock backend's module |
| interact/legacy.rs | b | InventoryTransaction legacy slot list (books, placement); Bedrock backend internals |
| interact/confirm.rs | a+b | `Confirm` neutral; `is_update_at` decodes Bedrock update packets → neutral echo set |
| interact/pipeline.rs | a+b | `Pipeline` neutral; `note_echo` decodes Bedrock packets → neutral echo set |
| interact/container.rs | b (5) | `Interact OpenInventory`, `ContainerClose`, `WindowType::Trading`; open via click is neutral |
| interact/closing.rs | c | Geyser close/ack sequencing; Java closes are one `ContainerClose(id)` |
| items/slot.rs | 5 | ContainerSlotType / FullContainerName / StackRequestSlotInfo; `SlotRef` itself is neutral |
| items/request.rs | 5 | ItemStackRequest encode/decode |
| items/plan.rs | 5 | request packet + stack-id prediction |
| items/actions.rs | 5 | request send/wait, OpenInventory/ContainerClose |
| items/craft.rs | 5 | Bedrock craft actions |
| items/ui.rs | 5 | Bedrock UI window offsets (ContainerSlotType) |
| build/structure.rs | a | `.mcstructure` NBT reader; use acacia-nbt directly |
| survival/consume.rs | b | eat = ClickAir + EntityEvent + finish ClickAir + input flag |
| survival/auto_eat.rs | a | proto `GameMode` only |
| survival/fetch.rs | 5 | raw ItemStackRequest + `RawPacket` response match |
| workstation/mod.rs, craft.rs, hand.rs, held_grid.rs, smithing.rs, furnace.rs, brewing.rs, beacon.rs, anvil.rs, loom.rs, grindstone.rs, cartography.rs, enchant.rs, trade.rs | 5 | `WindowType`, ItemStackRequest actions, recipe network ids, `RawPacket` waits |
| workstation/enchants.rs, anvil_cost.rs, shield.rs | a (5) | pure functions over Bedrock item NBT `Value` / `GameMode`; neutral once items carry neutral enchant/repair data |
| sleep.rs | b | `PlayerAction StartSleeping/StopSleeping`, `Text`/`Animate` refusal/wake parsing (Java: use bed + PlayerCommand LEAVE_BED, system chat) |
| sleep/tick.rs | b | sends PlayerAuthInput with BDS lying position; movement-backend concern |
| fishing.rs | b | ClickAir + `InputData::StartUsingItem`; bite detection via state is neutral |
| books.rs | b | `BookEdit` + legacy slot rewrite (Java: `ServerboundEditBookPacket(slot, pages, title)`) |
| signs.rs | b | `BlockEntityData` sign NBT (Java: `ServerboundSignUpdatePacket(pos, front, lines[4])`); build needs it |
| forms/mod.rs, forms/actions.rs, forms/reply.rs | c | ModalForm* are Bedrock-only |
| subchunks.rs | c | idle-mode SubchunkRequest stream |
| lag.rs | a | queues `RawPacket`; make generic over the edition's packet type |
| trace/mod.rs, trace/replay.rs | c | Bedrock movement traces (PlayerAuthInput, CorrectPlayerMovePrediction); Java has its own in acacia-java-physics |
| swarm/login.rs, builder.rs, supervisor.rs | b | Bedrock `ClientBuilder` / realm / auth connect → edition connect |
| swarm/policy.rs | a | needs neutral `ConnectError` / `DisconnectReason`; `PlayStatusStatus` is Bedrock |
| interact/tests.rs, vanilla_tests.rs, confirm_tests (via confirm), items/tests/*, survival/tests/*, workstation/tests/*, signs/tests.rs, sleep/tests.rs, books/tests.rs | t | assert Bedrock wire; keep under the Bedrock backend |

Files in the slice that do **not** import proto but still send Bedrock packets through `wire`/`client`:
interact/entity.rs (MouseOver + UseItemOnEntity, b), interact/equipment.rs (MobEquipment, b),
survival/equip.rs (`use_item` to wear armour + local hand/armour swap that only BDS does, b),
cancel/residue.rs:96-97 (calls `abort_break`, `close_own_inventory_screen`; fine once those are backend
methods). governor.rs, act_timing.rs, cancel/mod.rs, text.rs, interact/geometry.rs, interact/sight.rs,
interact/break_time.rs are neutral.

## Suggested step-4 backend surface (minimal for path, dig, place)

`select_slot(u8)`, `swing(SwingSource)`, `break_step(first, last, pos, face, creative)`,
`abort_break(pos, face)`, `click_block(pos, face, hit, source)`, `use_item()`, `release_item()`,
`attack(entity)`, `interact(entity, hit)`, plus a per-tick neutral `echoes: &[BlockPos]` (and Java ack
bookkeeping) feeding `Confirm`/`Pipeline`. Everything else in this slice either already sits on Bot methods
(build, most of survival) or is step 5 (items, workstation, fetch) or `bot.bedrock()` (forms, subchunks,
closing, traces).
