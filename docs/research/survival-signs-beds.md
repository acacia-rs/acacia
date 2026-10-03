# Eating, equipment, signs, books, beds and clicks: vanilla wire behaviour

Signs, books, beds, block/entity clicks, hotbar selection and commands are fitted to a vanilla capture
(Windows 1.26.52 on BDS, `vanilla-actions-2026-10-02.md`, one sample of most actions). Eating and
equipping armour by use were not captured and still come from server implementations. Code:
`crates/acacia-bot/src/{survival,signs.rs,books.rs,sleep.rs,reflex.rs,interact/}`; delays: `human.rs`.

Server sources (all checked 2026-10-02):
- PocketMine-MP (PM) `src/network/mcpe/handler/InGamePacketHandler.php`, `src/block/tile/Sign.php`,
  `src/block/Bed.php`, `src/player/Player.php`. https://github.com/pmmp/PocketMine-MP (stable)
- PowerNukkitX (PNX) `network/process/handler/{InventoryTransaction,ActorEvent,PlayerAction,BookEdit,BlockActorData}Handler.java`,
  `blockentity/BlockEntitySign.java`, `block/BlockBed.java`. https://github.com/PowerNukkitX/PowerNukkitX (beta)
- Geyser `translator/protocol/bedrock/{BedrockInventoryTransactionTranslator,BedrockBookEditTranslator,BedrockBlockEntityDataTranslator}.java`,
  `entity/BedrockEntityEventTranslator.java`, `entity/player/BedrockPlayerActionTranslator.java`,
  `entity/player/input/BedrockPlayerAuthInputTranslator.java`. https://github.com/GeyserMC/Geyser (master)
- Dragonfly (DF) `server/session/handler_{inventory_transaction,book_edit,block_actor_data}.go`, `server/player/player.go`
  (`UseItem`, `ReleaseItem`, `EditSign`, `Sleep`), `server/item/*.go` (food values, `DefaultConsumeDuration`).
- gophertunnel `minecraft/protocol/inventory.go` (`TriggerType`), `packet/player_auth_input.go` (`InputFlagStartUsingItem`).
- Nukkit-MOT PR #976 (consumable use state): https://github.com/MemoriesOfTime/Nukkit-MOT/pull/976

## 1. Eating and drinking (not captured)

Sequence we send (`Bot::consume`, `survival/item_use.rs`):
1. Press use: `InventoryTransaction` UseItem **ClickAir** (block 0,0,0, face 255, held item, hotbar slot;
   trigger 0 and prediction Failure like every captured ClickAir). Same tick: `PlayerAuthInput` flag
   **StartUsingItem**. Physics bots also simulate the item-use slowdown (`Input::using_item`).
2. While holding: `EntityEvent` **EatingItem** (57) with data `(network_id << 16) | aux`, on Java's cadence
   (from tick 7, whenever the remaining ticks are a multiple of 4: six times for 32-tick food).
3. Use time over (32 ticks; dried kelp 16, honey bottle 40, potion/milk 32) → on the next tick a second
   **ClickAir** with trigger **SimulationTick**.
4. Wait for the server: held count drops / item changes (bowl, bottle, bucket) / hunger or saturation rises.
5. Key-up after a human delay: `InventoryTransaction` **ReleaseItem** (Release).

Evidence:
- PM's ClickAir handler: while `isUsingItem()`, a ClickAir calls `consumeHeldItem()` with **no timing
  check**, otherwise it starts using. So vanilla sends exactly one finishing ClickAir.
- DF: the second ClickAir consumes only if ≥ `DefaultConsumeDuration` = 1.61 s passed, so we finish on tick 33.
- PNX: the second ClickAir (or a Release) completes once `ticksUsed` ≥ the item's use ticks.
- Geyser: ClickAir → Java `useItem`; Release → Java `RELEASE_USE_ITEM` cancels the meal, so the release
  must come **after** the server confirmed (step 4).
- EATING_ITEM: Geyser echoes it back; PNX validates the data against the held item; PM discards it.
- Food values (hunger, saturation) are Java's and DF's: `survival/food.rs`.

## 2. Equipment and tools

- Armour and offhand go through `ItemStackRequest` in the own inventory screen (items/ module): `Place` into
  an empty `Armor`/`Offhand` slot, `Swap` with a worn item. Captured for the elytra: `Take` to the cursor,
  then `Place` into the armour slot; no MobArmorEquipment from the client.
- `EquipMethod::Use` (not captured): armour in hand is put on with a plain ClickAir. The server swaps hand
  and armour slot (DF `UseContext.SwapHeldWithArmour`; Java equips on use). The bot waits up to 3 s.
- Best tool: the fewest `break_ticks` over the 36 main slots, counting each item's own Efficiency, only
  if faster than the bare hand; tools whose durability left is below the spare threshold are skipped.
- Auto-eat without hotbar food first moves the best food up from the inventory (`survival/fetch.rs`):
  `Interact` OpenInventory, the open, one `Place`/`Swap` request, `ContainerClose`.

## 3. Signs

- The server sends `OpenSign{position, is_front}` after a sign is placed (front, 172 ms after the click)
  or clicked (92 ms). It opens one side per editor session.
- On closing the editor the client sends **one** `BlockActorData` with the whole sign block entity: the
  server's last copy with only the edited side's `Text` changed. `TextOwner` stays as the server sent it
  (empty on BDS), `LockedForEditingBy` is echoed, `FilteredText`/`BlockEntityVersion` are kept, keys are in
  byte order (the client's `CompoundTag` is a sorted map). Text is sent as typed (`"t\na\n"` keeps the
  trailing newline). Sent 2.9 s and 2.2 s after OpenSign for 4 and 3 characters.
- Without a server copy (a sign seen only in chunk data while block entities are untracked) we send BDS's
  shape: sides `{FilteredText, HideGlowOutline 0, IgnoreLighting 0, PersistFormatting 1, SignTextColor
  black, Text, TextOwner ""}`, root `{BackText, BlockEntityVersion 0, FrontText, IsWaxed 0,
  LockedForEditingBy <our unique id>, id, x, y, z}`. Its other side is then sent empty, and DF would apply that.
- What servers read: Geyser `id` ends with `Sign`, x/y/z, the opened side; PNX `id`, both sides, ≤ 4 lines,
  `LockedForEditingBy` = the editor; PM `FrontText` must be a compound (else kick); DF both sides, ≤ 256
  bytes, applies only the changed side.
- Placing a sign has an extra ClickAir (§6).

## 4. Books

- `BookEdit{inventory_slot, type, ...}`, slot = the book's hotbar slot (PNX accepts 0-8). ReplacePage (0)
  `{page, text, photo_name ""}`, AddPage (1) inserts before an existing page, DeletePage (2), SwapPages (3),
  Sign (4) `{title, author, xuid}`. Trailing pages need no AddPage (PM, DF).
- Limits: 50 pages; title 16 characters (PM, Geyser); page 256 characters (PNX; PM 512, Geyser 1024).
- **Opening a new book and quill** (no `pages` tag), all in the same ms: `ReplacePage page 0 ""`,
  `ReplacePage page 1 ""`, `ItemUse ClickAir` with a legacy request id, `legacy_transactions [{29, [slot]}]`
  and the slot action book → book with `pages []` (list type 0), then `MobEquipment` with that book.
- **Text goes out once, when the book closes**, never per keystroke: `InventoryTransaction Normal`
  (legacy id 0; actions Container slot old → new, Creative 0 air → old, Creative 1 new → air), then
  `BookEdit ReplacePage {page, text}` for the written page, then `MobEquipment`. 3.0 s after the open for 3 characters.
- **Signing**: `InventoryTransaction Normal` with the next legacy id and the slot listed (writable_book →
  written_book with `author`, `generation 0`, `pages`, `title`, `xuid`), then `BookEdit Sign {title,
  author "Author Unknown", xuid}`. BDS answers with InventoryContent + PlayerHotbar select; the client's
  mouse-over-nothing + `MobEquipment` came 43 ms after that. 4.1 s after the text for a 2-character title.
- BDS answers each Normal transaction with an InventoryContent still holding the **old** book, then its
  own `InventoryTransaction` old → new; the inventory tracker applies both in order, as the client does.
  Without the Normal transactions BDS sent no update at all, so the bot also applies each change to the
  held book itself (live-action-tests.md fix 8).

## 5. Beds

- Lie down: the block click on the bed (empty hand: `result_position` is the neighbour cell). BDS answers
  with SetSpawnPosition, `tile.bed.respawnSet`, LevelEventGeneric `sleepingPlayerCount`; the client sends
  **`PlayerAction StartSleeping`** (positions 0, face 0) 513 ms after the click. PM, PNX and Geyser ignore it.
- The server shows sleep in `SetEntityData`: BDS/PM/PNX `PlayerFlags` bit 1 + `PlayerBedPosition`;
  Geyser entity flag `SLEEPING` + `BedPosition`. We track both; either means sleeping.
- **The server ends the sleep** (night skipped): `Animate WakeUp` → client `StopSleeping` (positions 0,
  face 0) 105 ms later, then `ClientCameraAimAssist Clear` 57 ms after that. No sneak, Interact or Respawn.
  The bot also answers when the sleeping flag clears without a WakeUp.
- Leaving on our own (not captured): the same `StopSleeping` and aim-assist clear, then the server's flags.
- Refusals are chat: PM/PNX a gray `Text` Translation `%tile.bed.{noSleep,notSafe,tooFar,occupied,notValid}`;
  Geyser Java's action-bar text as `JukeboxPopup`. `tile.bed.respawnSet` is the success message.

## 6. Clicks, hotbar and commands

- **Block click** (use, open, place), within 0-8 ms: `PlayerAction StartItemUseOn {pos, result_position,
  face}`, `Animate SwingArm` (`"interact"`, `"build"` when placing), `ItemUse ClickBlock {PlayerInput,
  Success}`. `StopItemUseOn {result_position, 0, face 0}` follows 15-308 ms later (25 clicks, most 30-140),
  sometimes after the ContainerOpen. No ClickBlock repeats came between Start and Stop.
- **result_position** is the neighbour cell (pos + face) when the hand is empty or holds a block item
  (`block_runtime_id` ≠ 0) or a sign; the clicked block for beds, boats, tools and other items.
- **Placing** adds the held slot's predicted change to the ClickBlock: action Container window 0 slot,
  old → old − 1. A stack that stays (sign 2 → 1) also gets a legacy request id and `legacy_transactions
  [{29, [slot]}]`; one that empties (bed, boat) has id 0. Then `MobEquipment` with the new stack; a
  placed sign also gets a `ClickAir {UnknownValue, Failure}` before it.
- **Legacy request ids** are negative and even on one counter (-70 sign, -76 book open, -78 book sign);
  the two sign edits in between seem to use one each. The starting value is unknown; we start at -2.
- **ClickAir** from a press (rod, firework, book, empty map, after a sign, a missed entity): trigger 0
  (UnknownValue), prediction Failure.
- **Entity click**: `Interact MouseOverEntity {rid, has_position, hit point}` first (75-1277 ms before), then
  `ItemUseOnEntity`; the client's `Animate "interact"` comes 124-203 ms **after** the transaction. Attacks
  were not captured.
- **MobEquipment** goes out whenever the selected slot or its content changes: slot switch, own placement or
  book edit (same ms), container move (60-130 ms after the response, twice ~700 ms), server `PlayerHotbar
  select_slot true` or resync (43-247 ms), even with a form or container open. It is preceded by
  `Interact MouseOverEntity {0, no position}` exactly when the slot or the item type changed (~40 cases);
  count, damage or NBT changes go alone. None is sent for the inventory at join. Item without stack id.
- Hotbar select to first use: 0.6, 1.05, 2.6 s.
- **Commands**: `CommandRequest {command, origin {type "player", uuid <random v4 per command>, request_id
  "", player_entity_id 0}, internal false, version "latest"}`; CommandOutput 74-113 ms later.

## Open questions

1. Eating (not captured): finishing ClickAir trigger and tick, `StartUsingItem` tick, EATING_ITEM cadence,
   ReleaseItem after a meal, whether the press ClickAir has prediction Failure like other items.
2. Armour by use (not captured): only the ClickAir, or also a request / MobArmorEquipment? Occupied slot?
3. Moving food up from the inventory: one `Place` (as sent) or `Take` to the cursor + `Place`.
4. Books: opening a book that already has pages (we send a plain ClickAir); several pages in one session
   (we send one ReplacePage per page on close); whether `author` is "Author Unknown" on online servers too.
5. Placing ordinary blocks (only signs, beds and boats were placed); attacks; the hover stream while the
   crosshair crosses mobs and the mouse-over-nothing on leave (we send one hover before a click, and
   mouse-over-nothing before a block click after it).
6. Leaving a bed on our own; BDS's own bed refusal texts.
7. The legacy request id counter's start and what else steps it.
