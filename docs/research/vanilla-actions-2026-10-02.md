# Vanilla client gameplay actions (Windows 1.26.52, BDS offline, 2026-10-02)

Transparent tools/mitm capture of a fresh join followed by scripted gameplay
(`.testserver/mitm/20261002-214027.jsonl`, git-ignored). **t = ms since Login.** Spawn (PlayStatus PlayerSpawn) at
t 17516. Decoded per-action dumps: `.testserver/mitm/actions-20261002/<action>.txt` plus `item-ids.txt`. Regenerate:

```
cargo run -p acacia-testserver --example capdump -- <capture.jsonl[#n]> [--only A,B] [--exclude A,B] [--all]
    [--from ms] [--to ms] [--client|--server] [--pai changes|all|none] [--cap n]
```

Redacted: player name (`<player>`), xuid, uuids, skin. Item ids: oak_log 17, coal 304, iron_ingot 307, diamond_sword 318,
saddle 374, oak_boat 378, fishing_rod 395, lapis 417, writable_book 521, written_book 522, carrot_on_a_stick 527,
firework_rocket 529, elytra 574.

## What was skipped or differs from the plan

- **Skipped:** helmet drag and chestplate right-click (no armor item ever leaves the client), `/effect hunger` and steak
  eating (only 3 CommandRequests in the session; hunger stays 20), cartography zoom with paper, pig sneak-off (LeaveVehicle
  was sent without any Sneak flag).
- **Different:** planks and sticks were crafted by hand in the grid; only oak_fence used the recipe book. The diamond
  sword was enchanted, not the iron sword. Fishing had 4 cast/reel pairs: one catch and three early reels. Bed wake-up
  came from the server when the night was skipped.
- Extra: a first pass at 53.5–65.5 s opened and closed workbench, furnace, brewing stand and enchanting table with no
  clicks, and the wandering trader came before crafting.

## Cross-cutting rules (hold for every action below)

- **Opening a block** (workstation, sign re-edit, bed): a 0–8 ms burst of `PlayerAction StartItemUseOn {pos,
  result_position, face}`, `Animate SwingArm swing_source "interact"` (`"build"` when placing a block, boat, sign or
  bed), `InventoryTransaction ItemUse ClickBlock {trigger PlayerInput, client_prediction Success, cooldown Off,
  hotbar_slot, held_item (has_stack_id false), click_pos fractional in the block, block_runtime_id}`. Then `StopItemUseOn {pos = start's result_position, result (0,0,0), face 0}` follows 15–308 ms later, sometimes
  **after** ContainerOpen. ContainerOpen arrives 16–157 ms after Start, `runtime_entity_id -1`.
- **result_position:** equals pos when an item is held. It is the **neighbour cell** (pos+face) when the hand is empty
  or holds a placeable block (brewing, enchanting, cartography with fences). StopItemUseOn then uses that neighbour.
- **Held right-click:** after a ClickBlock or entity interact, while the button is held, the client repeats
  `ItemUse ClickBlock` with `trigger SimulationTick, prediction Failure` (often 2–4 within 20 ms), sometimes mixed with a
  `ClickAir trigger UnknownValue, face 255, block (0,0,0), Failure`.
- **ItemStackRequest:** one request per packet. `request_id` is negative and odd, **-3, -5, -7 … stepping -2 on one
  counter for the whole session** (-163 at the end). `custom_names []`, `cause Unknown(-1)` unless renaming.
  Responses take 17–164 ms. On the next request the client reuses the ids from the response.
- **Container labels:** sources in hotbar slots 0–8 say `Hotbar`, 9–35 `Inventory`; shift-click/auto destinations,
  craft Consumes and fills of a crafting grid or trade slot `HotbarAndInventory`; manual drops `Inventory`. Output is `CreatedOutput` (decoder: CreativeOutput) slot 50 with
  **stack_id = the request's own request_id**.
- **Crafted-result flow:** `CraftRecipe… → ResultsDeprecated → Consume(s) → Place from CreatedOutput 50`. There is no
  Take via cursor unless the user drags. Merging onto an existing stack puts that stack's id in the destination
  (else 0).
- **MobEquipment:** sent whenever the content of the selected hotbar slot changes (container move, placement, use
  consumption, server `PlayerHotbar select_slot true`). It is always preceded in the same ms by
  `Interact MouseOverEntity {target 0, has_position false}`. The item has `has_stack_id false`.
- **ContainerClose:** C>S `{window_id N, window_type None, server false}`. **Trader uses window_id None.** The server
  echoes identical fields 3–92 ms later. The last click comes 0.4–9 s before the close. A PAI movement flag (Right/Left/
  Down) often lands 1–170 ms before the close (player walking away).
- **Window ids:** they increment per open, 2 → 32, and the inventory screen counts too. Inventory screen:
  `Interact OpenInventory target 1` → `ContainerOpen window_type Inventory, coordinates = player block`, 17–93 ms. While
  riding, the target is **the vehicle's runtime id** (pig 22).
- **Entity interact:** `Interact MouseOverEntity {target rid, has_position true, position = ray hit}` streams while the
  crosshair crosses mobs (about 40 over the trader approach), and sends `target 0, has_position false` on leave.
  `ItemUseOnEntity {rid, action Interact, hotbar_slot, held_item, player_pos, click_pos = world hit point}`. **The
  client's Animate "interact" comes 120–200 ms *after* the transaction** (after the server's own Animate echo), unlike
  block clicks.
- **Commands:** `CommandRequest {command, origin {type "player", uuid <fresh random per command>, request_id "",
  player_entity_id 0}, internal false, version "latest"}`. CommandOutput arrives about 74–113 ms later.
- **ClientCameraAimAssist {preset "", Clear, allow false}:** sent at spawn, on every mount, on every dismount, on
  wake-up and after teleport. The server answers CameraAimAssist Clear 27–108 ms later.

## Join / spawn

| t | Δ vs spawn | Packet |
|---|---|---|
| 3526 | | ClientCacheStatus enabled (before ResourcePackClientResponse HaveAllPacks 3600, Completed 3770) |
| 6847 | −10669 | S>C StartGame |
| 14970 | −2546 | SetPlayerGameType Fallback + RequestChunkRadius {12, max 28} + **ServerboundLoadingScreen type 1**, same ms |
| 17516 | 0 | S>C PlayStatus PlayerSpawn (with SetHealth) |
| 17998 | +482 | first ClientCacheBlobStatus |
| 18083 | +567 | ClientCameraAimAssist Clear + Interact MouseOverEntity 0 + EmoteList (4 pieces), same batch |
| 18085 | +569 | first PlayerAuthInput, **tick 16**, pos (0.5,72.62,0.5) |
| 18086 | +570 | second ClientCameraAimAssist Clear; server's CameraAimAssist ×2 at 18111/18112 |
| 20023 | +2507 | **ServerboundLoadingScreen type 2** |
| 20029 | +2513 | SetLocalPlayerAsInitialized |
| 21249 | +3733 | first SubchunkRequest (1.2 s after loading screen end) |
| 345747 | +328 s | first ClientMovementPredictionSync |

- No PlayerAction Respawn at spawn. The three S>C Respawn packets (9606 ×2 states 0, 11693 state 1) were not answered
  by any client Respawn.
- Unlike the 2026-10-01 capture, Interact/EmoteList come **before** the server's CameraAimAssist, in the same ms as the
  first ClientCameraAimAssist. The aim-assist pair comes 0.57 s after spawn, not 1.6 s.
- PAI gaps after spawn: 360, 587, 972 ms (loading). It is steady 20 Hz after about 21 s.
- **ClientMovementPredictionSync** is not periodic: only 3 are sent in 8.5 min (t 345747, 453252, 510523). Fields:
  bbox 1/0.6/1.8 (0.6 high while gliding), speed 0.1, underwater/lava 0.02, jump 0, health 20, hunger 20,
  `is_flying false`. While gliding, data_flags gains bit 32.
- Decoder gap: S>C CreativeContent fails (`ItemExtraDataWithBlockingTick.blocking_tick` EOF).

## Forms (20677–34518)

| form | shown | type | response data | delay |
|---|---|---|---|---|
| 1 | 20677 | `form` header + button + divider + button | `"0\n"` (header/divider not counted) | 2944 |
| 2 | 23839 | `modal` | `"true\n"` | 1712 |
| 3 | 25792 | `custom_form` label, input, toggle, slider, dropdown | `"[null,\"\",true,5,1]\n"` | 4557 |
| 4 | 30445 | `form` one button | `"0\n"` | 4073 |

All responses have `has_response_data true, has_cancel_reason false, content None` and a **trailing `\n`**. A label is
`null`, an empty input is `""`, the slider is an int, the dropdown is an index. No PAI flag change happened while forms
were up. The server's /give arrives as S>C InventoryTransaction Normal (Container slot + Creative action) +
InventoryContent ×4 + PlayerHotbar(select false) + Text. When the empty selected slot got oak_log, the client sent
MouseOverEntity 0 + MobEquipment 91 ms later, even with a form open.

## Wandering trader (67149–96632)

- **Misclicks on a llama:** ItemUseOnEntity rid 25 + same-ms `ClickAir UnknownValue Failure`, and no window opened.
- **Trader click:** 79349 ItemUseOnEntity rid 23 (held coal, no ClickAir pair). **S>C `UpdateTrade` at +68 ms, no
  ContainerOpen.** Fields: window 6 Trading, new_trading_ui true, economic_trades true. Recipe netId 4771 =
  1 emerald → packed_ice.
- **Requests** (first 2210 ms after UpdateTrade): -3 Place **32** (whole stack) HotbarAndInventory 19 →
  `Trade2Ingredient1` slot 4; -5 back, -7 in again;
  - -9 `CraftRecipe {4771, times 1}` + `ResultsDeprecated packed_ice ×1` + `Consume 1 Trade2Ingredient1 4` +
    **`Take 1 CreatedOutput 50 (sid -9) → Cursor 0`**
  - -11 same; the Take destination is the cursor's stack 65
  - -13 Place 2 Cursor → `Inventory` 32
  - -15 Place 30 leftovers back
- Click gaps were 477–843 ms, plus one 10.8 s idle. Close: `ContainerClose {window_id None}`; the server echoes
  window 6. No PAI was sent from 79583 to 96487 (no flag changes in the UI).

## Crafting table (108376–127143, window 7)

- -17 Place 4 oak_log HotbarAndInventory 0 → **CraftingInput slot 32** (grid slots 32–40), 1.9 s after open.
- -19 `CraftRecipe {715, times 4}` + `ResultsDeprecated oak_planks ×4 times 1` + Consume 4 + Place **16**
  CreatedOutput (sid -19) → HotbarAndInventory 0. A shift-click: times = 4 but ResultsDeprecated stays times 1.
- -21 Take 8 Hotbar 0 → Cursor (new cursor sid). -23 Place 1 → CraftingInput 36. -25 Place 1 → 39.
- -27 `CraftRecipe {1581, 1}` + Results stick ×4 + Consume 36 + Consume 39 + Place 4 → HotbarAndInventory 33.
  -29 Place 6 Cursor → Inventory 34.
- -31 recipe book: **`CraftRecipeAuto {659, times 2, ingredients ×6 (count 1, metadata 32767)}`** + Results
  oak_fence ×3 times 2 + Consumes + Place 6 → slot 0.
- Click gaps: 0.74–1.5 s, one of 6.4 s. Close 4.1 s after the last click.

## Furnace (127358–143640, window 8)

- -33 Place 2 raw_iron Hotbar 2 → `FurnaceIngredient` 0. -35 Place 2 coal → `FurnaceFuel` 1. The server sends
  InventorySlot before each response. The fuel lights in 42 ms, and the server sends BlockEntityData.
- The coal came out of the held slot, so the client sent MouseOverEntity 0 + MobEquipment(empty).
- Ingot ready about 10 s later (InventorySlot). -37 **Place** 1 `FurnaceOutput` 2 → HotbarAndInventory 11, with the
  destination sid set to the existing ingot stack (24). Only 1 of the 2 ingots was taken.

## Brewing stand (143931–155967 window 9; 165308–168995 window 11)

- Empty-hand open: result_position and Stop pos are the neighbour cell, click_pos z 0.875.
- **Visit 1:** Place 1 water bottle → `BrewingResult` 1, 2, 3 (-39/-41/-43, 226–276 ms apart), nether_wart →
  `BrewingInput` 0 (-45), blaze_powder → `BrewingFuel` 4 (-47; consumed instantly).
- **Visit 2:**
  - -59 took the unbrewed bottle from slot 1, 408 ms before the brew finished (about 20 s after the fuel), so only 2
    awkward potions were made.
  - Finished potions get **new stack ids** (75/76) via InventorySlot.
  - -61/-63 Place BrewingResult → HotbarAndInventory.

## Enchanting table (156078–164815, window 10)

- PlayerEnchantOptions comes **empty** with the open. The 3 options arrive only with the response to Place item →
  `EnchantingInput` 14 (-49). Options: option_id 4778/4779/4780, cost 1/3/4.
- -51 Place 8 lapis → `EnchantingLapis` 15.
- -53, 696 ms later: **`CraftRecipe {recipe_network_id = option_id 4779, times 1}`** + Results (sword with ench NBT) +
  Consume input + **Place CreatedOutput → EnchantingInput 14 (dst sid -53)** + Consume **2** lapis (the 2nd option costs
  2 lapis, not its "cost" 3). The server clears the options.
- -55 moves the sword out (a separate request). -57 takes the remaining lapis back.

## Anvil (171000–184684, window 12)

- -65 Place pickaxe → `AnvilInput` 1 (response carries durability_correction 200). -67 Place 9 ingots →
  `AnvilMaterial` 2.
- -69 repair: **`Optional {recipe 0, filtered_string_index 0}`** + Consume 4 material + Consume input + Place
  CreatedOutput → HotbarAndInventory 6.
- -71 back in. -73 Take 5 material → Cursor. -75 Place to Inventory.
- -77 rename: `Optional {0,0}` + Consume + Place, with **`custom_names ["rename"], cause AnvilText`**.
- Repair and rename were two separate results. 4.5 s from open to first click.

## Grindstone, stonecutter, smithing, loom, beacon

- **Grindstone** (185123–193667, window 13): -79 Place sword → `GrindstoneInput` 16. -81
  **`CraftGrindstoneRequest {recipe_network_id 74 (= input's stack id), times 1, cost 0}`** + ResultsDeprecated +
  Consume + Place CreatedOutput. 3 xp_orbs spawn.
- **Stonecutter** (194233–197840, window 14, face 1, click y 0.5625): -85 Place 4 → `StonecutterInput` 3. -87
  `CraftRecipe {450, times 4}` + Consume 4 + Place 4. **No ResultsDeprecated.**
- **Smithing** (202280–213461, window 15):
  - template → slot 53, material → 52, input → 51 (the player took the ingot out once in between)
  - -99 `CraftRecipe {1567, 1}` + Results netherite_sword + 3 Consumes + Place

  Gaps 549 ms – 5.1 s.
- **Loom** (215987–234455, windows 16, 17):
  - -101 banner → `LoomInput` 9, -103 dye → `LoomDye` 10
  - -105 **`CraftLoomRequest {pattern "vh", times 1}`** + Results banner (NBT Patterns Color 1) + 2 Consumes + Place
  - The colour travels only in the Results NBT. The pattern slot was not used. Visit 2 put the banner in and took it out.
- **Beacon** (278854–283312, window 23, face 2): -137 Place iron_ingot → `BeaconPayment` 27 (new sid 89). -139
  **`BeaconPayment {primary 3, secondary 0}` + `Destroy` 1** (not Consume). No BlockEntityData.

## Cartography (234811–277456, windows 18, 20, 21, 22)

- **Making a map:** 245703, right after a ContainerClose, while holding empty_map:
  - `ItemUse ClickBlock (PlayerInput, Failure)` + `ClickAir (UnknownValue, Failure)` in the same ms, **no
    StartItemUseOn, no Animate**, then 4 SimulationTick repeats
  - The server resyncs with UpdateBlock ×N, PlayerHotbar select_slot true ×5 and InventoryContent ×5
  - **`MapInfoRequest {map_id, client_pixels ≈16384 × {rgba, index}}`** 423 ms later (the full 128×128 upload)
- **Every cartography craft is `CraftRecipeOptional`**, with `cause CartographyText` and one custom_names entry:
  - no-op with input only: recipe **0**, `custom_names [""]`
  - clone with an empty map in `CartographyAdditional` 13: recipe **4760**, Place 2
  - renames: recipe 4760, `custom_names ["2"]`, **one craft per item** (2 crafts for a stack of 2)
- Slots: `CartographyInput` 12, `CartographyAdditional` 13.

## Inventory moves (armor slot; elytra; sign/book/bed/saddle setup)

- **Drag move:** `Take all → Cursor`, then `Swap Cursor ↔ Hotbar n`, then `Place displaced → origin slot`. Three
  single-action requests about 450 ms apart (requests -141/-143/-145).
- **Elytra equip:** -161 `Take 1 Inventory 28 → Cursor`, then -163 `Place 1 Cursor → Armor slot 1`. The server sends
  InventorySlot Armor before the response. There is no MobArmorEquipment from the client.
- Plain hotbar fills are `Swap Inventory n ↔ Hotbar m` or `Place`. MobEquipment follows if the selected slot changed,
  even while the inventory is open.

## Sign (295314–304945)

- **Place** (sign in hotbar 3), within 9 ms: Start (face 1), Animate "build", ItemUse ClickBlock **with
  legacy_request_id -70, legacy_transactions [{container 29, slot 3}]** and action `Container window 0 slot 3: sign ×2 →
  ×1`, an **extra ClickAir UnknownValue Failure**, MobEquipment(sign ×1). Stop 32 ms later.
- The server sends OpenSign `{pos, is_front true}` 178 ms after the click.
- **Text:** C>S `BlockEntityData` with the full sign NBT (FrontText + BackText), `Text "t\na\n"` (trailing newline
  kept), `LockedForEditingBy` echoed. Sent 2883 ms after OpenSign. The client never sends is_front.
- Re-edit: block click → OpenSign +92 ms → BlockEntityData +2150 ms. Legacy request ids also run negative-even
  (-70, -76, -78).

## Book and quill (311650–318821)

- **Open** (use in air): `BookEdit ReplacePage page 0 ""` + `ReplacePage page 1 ""` + `ItemUse ClickAir` (legacy -76,
  action slot 0: book → book with `pages []`) + MobEquipment, all in the same ms.
- **Text goes out on UI close only** (no per-keystroke edits):
  - `InventoryTransaction Normal` (Container slot 0 old → new NBT, plus Creative actions)
  - `BookEdit ReplacePage {page 0, "ass", photo ""}`
  - MobEquipment
- **Sign:** `InventoryTransaction Normal` (legacy -78; writable_book 521 → written_book 522 with author/title/xuid) +
  `BookEdit Sign {title, author "Author Unknown", xuid <redacted>}`. The server answers with InventoryContent and
  PlayerHotbar select_slot true. Book changes never use ItemStackRequest.

## Bed (333219–343837)

- `/time set night` → CommandOutput +74 ms.
- **Place:** Start + Animate "build" + ItemUse ClickBlock with the slot action (bed → air), no ClickAir, MobEquipment
  air. Stop +233 ms.
- **Use:** 334716 Start (result = neighbour) + Animate "interact" + ClickBlock (empty hand). The server sends
  SetSpawnPosition +86 ms and LevelEventGeneric sleepingPlayerCount 1.
- **`PlayerAction StartSleeping` (all zeros) +513 ms after the click.** The PAI flags do not change while asleep.
- The server's `Animate WakeUp` came 4501 ms later. **Client `StopSleeping` +105 ms**, then ClientCameraAimAssist
  Clear +57 ms. No sneak, Interact or Respawn.

## Pig (345208–407982)

- **Saddle:** ItemUseOnEntity rid 22 holding saddle. The server answers with InventoryContent + PlayerHotbar select
  true. Client MobEquipment(air).
- **Mount:** ItemUseOnEntity with empty hand. SetEntityLink type 1 at +95 ms; client Animate +127 ms;
  ClientCameraAimAssist +288 ms; then held-button ClickBlock repeats (Failure).
- **Riding a pig: PAI has no ClientPredictedVehicle / vehicle_rotation / predicted_vehicle** (server-driven mount). PAI
  pos is pinned at the seat (y 70.85). Steering with the carrot stick is ordinary mv/rotation.
- **Dismount:** `Interact LeaveVehicle {target 22, has_position true, position = dismount point}`, with no Sneak flags.
  A PAI in the same ms already has the new position. ClientCameraAimAssist +2/+32 ms. SetEntityLink type 0 +81/+185 ms.

## Boat (422133–440534)

- **Place:** Start (face 1) + Animate **"build"** + ItemUse ClickBlock with legacy slot action (boat → air), then
  MobEquipment air. AddEntity boat +172 ms. Stop +308 ms.
- **Mount:** ItemUseOnEntity rid 38, empty hand. SetEntityLink +110 ms. First vehicle PAI +305 ms: pos = boat pos,
  flags `{ClientPredictedVehicle}`, `vehicle_rotation (0, yaw)`, `predicted_vehicle = boat unique id`.
- **Riding:** every tick has ClientPredictedVehicle. `PaddlingLeft` comes only with Left, `PaddlingRight` only with
  Right. The vehicle yaw is unwrapped (135 → 766).
- **Exit:** `LeaveVehicle {target 38, position}` + PAI with `flags {}` in the same ms, no sneak. SetEntityLink type 0
  +78 ms.

## Fishing (444784–483231)

- **Cast and reel are identical:** `Animate "useitem"` + `ItemUse ClickAir {UnknownValue, face 255, Failure}` + one PAI
  tick with `StartUsingItem` (may be 76 ms later). No PlayerAction.
- The server answers each with CompletedUsingItem {395, -1} + InventoryTransaction ItemRelease + InventoryContent.
  A cast also gets AddEntity fishing_hook after 96–186 ms.
- Casts 446178 / 452078 / 454149 / 481511, with reels after 5365 / 1624 / 27158 / 1569 ms. The catch was #3:
  - FishHookHook at 480788, reeled **518 ms** later
  - The rod gets a new stack id and Damage 1, and the server sends `PlayerHotbar select_slot true`
  - Client MobEquipment +97 ms
  - Salmon AddItemEntity → TakeItemEntity +380 ms
- Re-cast came 204 ms after the catch reel.

## Elytra (487770–521830)

- `/tp @s ~ ~40 ~` → MovePlayer Teleport +93 ms → PAI `HandledTeleport` +168 ms.
- **Glide start:** PAI `StartGliding` on a jump-press tick (`JumpDown, JumpPressedRaw, Jumping, WantUp`) **706 ms after
  the command**, already 6.6 blocks lower. **No PlayerAction StartGlide.** There is no persistent gliding flag.
- **Firework:** MobEquipment(select hotbar 1) → `Animate "useitem"` + `ClickAir UnknownValue Failure`, **no
  StartUsingItem**. Then MovementEffect GLIDEBOOST (duration 56, then 42) +83–241 ms, PlayerHotbar select true,
  AddEntity fireworks_rocket, and client MobEquipment with count−1 about 250 ms later.
- **Stop:** `StopGliding` came on a jump press in mid-air (y 76), and on the second glide **1 tick after touching
  ground**. A second jump press in the air (1.14 above ground) starts gliding.
- PlayerArmorDamage arrives about 1/s while gliding. It decodes as `armor_slot Leggings` although the elytra is in the
  chest slot, which suggests the decoder's bitset enum is off by one (unverified).
