# Crafting and workstations: vanilla request shapes

What the bot sends for crafting, smelting, brewing, enchanting, anvil, grindstone, stonecutter,
smithing, loom, cartography, beacon and trading (`crates/acacia-bot/src/workstation/`, `items/`),
and why. Researched 2026-10-02 against protocol 2193 (1.26.5x), then fitted to the vanilla capture in
research/vanilla-actions-2026-10-02.md (Windows 1.26.52 on BDS; request ids below are its ids).
**[C]** = confirmed by that capture, a vanilla dump or protocol source, **[I]** = inferred.

## Sources

- The 2026-10-02 capture: every screen below done once by hand (`actions-20261002/*.txt`).
- Vanilla dumps quoted in Geyser issues: [#5290](https://github.com/GeyserMC/Geyser/issues/5290)
  (shift-craft at a table), [#3682](https://github.com/GeyserMC/Geyser/issues/3682#issuecomment-2161793918)
  (two-result recipe), [#2933](https://github.com/GeyserMC/Geyser/issues/2933) (creative take).
- Beacon/cartography: Allay `BeaconPaymentActionProcessor`, Nukkit-MOT, PNX
  `CraftRecipeOptionalProcessor`, Geyser `BeaconInventoryTranslator`/`CartographyInventoryTranslator`.
- Geyser `translator/inventory/*InventoryTranslator.java` (master), PocketMine-MP
  `network/mcpe/handler/ItemStackRequestExecutor.php`, dragonfly `server/session/handler_*.go`
  (local module cache), PowerNukkitX `inventory/request/*`.
- gophertunnel v1.62.1 `minecraft/protocol/item_stack.go`, `recipe.go`, `enchant.go`;
  Mojang [bedrock-protocol-docs](https://github.com/Mojang/bedrock-protocol-docs)
  `additional_docs/ServerAuthInventory/PlayerUIContainer.md`.

## Wire facts

- Action type: varuint variant + u8 legacy id. The legacy id still counts the removed
  `PlaceInContainer`/`TakeOutContainer` (7, 8): variant ≥ 7 → legacy = variant + 2. So
  CraftRecipe 10/12, CraftRecipeAuto 11/13, CraftRecipeOptional 13/15, CraftGrindstone 14/16,
  CraftLoom 15/17, CraftResultsDeprecated 17/19 (gophertunnel `item_stack.go:47-52`). [C]
- One request per packet. Request ids: **-3, -5, -7, … on one counter per session** (the bot keeps
  it per `Bot`, `items::RequestIds`). `custom_names []` and **cause -1** unless renaming. [C]
- Stack ids: a slot is named by its stack's id until an action of the same request changes it;
  from then on by the **request id**, even once emptied (-53 places the enchanted sword into the
  input it just consumed as `#-53`; -31 consumes slot 0 as `#67`, then `#-31`, and places the fences
  into it as `#-31`). The created output is always the request id. Merging onto a stack names that
  stack (furnace -37, `#24`), an untouched empty slot is 0. [C]
- Player slot labels (`request::auto_move`): `Hotbar` (0-8) / `Inventory` (9-35) for clicks the player
  aims (into a station, onto the cursor, off the cursor); **`HotbarAndInventory` for moves the client
  makes itself while a container is open**: craft `Consume`s, anything out of a container, station
  or created output, and filling a crafting grid or trade payment slot (-17, trade -3). [C] Clicks
  the player aims in the own inventory screen use `Hotbar`/`Inventory` [C]; its automatic moves use
  `HotbarAndInventory` too, since BDS answers a recipe-book `Consume` from `Hotbar` with status 27
  (ConsumedItemNotAllowed; live 2026-10-02). BDS opens that screen as the next window id
  with type `Inventory`; the client closes it as `{that id, window type None}`, like every screen. [C]
- Shift-click destinations: partial stacks of the same item first (furnace -37 joined 8 ingots in
  slot 11 while slots 1, 2 were empty), then the first empty slot, counted after the request's own
  Consumes (-31 put the fences where its planks were). [C] The bot only merges when a count above 1
  shows the item stacks (two swords never merge; max stack sizes are not known). [I]
- CraftRecipeAuto (since protocol 2168): recipe net id, `numberOfRequestedCrafts` u8, ingredient
  list (descriptor + u16 count); the old `timesCrafted` byte is gone. [C]
- UI window (124) offsets, used as the slot index in requests: cursor 0, anvil 1/2, stonecutter 3,
  trade 4/5, loom 9/10/11, cartography 12/13, enchanting 14/15, grindstone 16/17, beacon 27,
  2x2 grid 28-31, 3x3 grid 32-40, **created output 50** (`CreativeOutput`), smithing 51/52/53
  (input, material, template). [C] Results always leave from 50.
- Furnace (ingredient 0, fuel 1, output 2) and brewing stand (ingredient 0, bottles 1-3, fuel 4) are
  plain container slots: only Take/Place/Swap. XP for smelting is granted server-side on taking. The
  server sends `InventorySlot` before each response there; finished potions get new stack ids. [C]
- `CraftingData` (2193): recipe ingredient = varuint 0 (invalid + aux) or 1 + type string
  (`name`, `item_tag`, `molang`) + body + varint count; aux 32767 = any. Item tags are not sent:
  the client knows them; the bot carries a table of common ones (`state/recipes/tags.rs`).

## Sequences (one ItemStackRequest each)

Every craft: **craft action → CraftResultsDeprecated → Consume… → [Create] → Take/Place from 50.**
ResultsDeprecated lists each result with its per-craft count; `timesCrafted` is the craft count
for recipe-book crafts (-31: 2) and 1 otherwise, even for 4 crafts (-19). [C]

| Screen | Craft action | Consumes | Result |
|---|---|---|---|
| Recipe book [C] | CraftRecipeAuto{id, n, one descriptor per non-empty cell, per-craft count, recipe aux} | per cell in recipe order, n × count each, from the inventory | Place into the inventory |
| Manual grid [C] | CraftRecipe{id, n} | each occupied grid cell (28-31 / 32-40) × n | Place (shift-click) |
| Several results | as above | as above | per result: Create{index}, then Place |
| Beacon [C] | BeaconPayment{primary, secondary} (no ResultsDeprecated) | Destroy 27 × 1 (not Consume) | none |
| Stonecutter [C] | CraftRecipe{id, n}, **no ResultsDeprecated** | 3 × n | Place |
| Smithing [C] | CraftRecipe{id, 1} | 53, 51, 52 | Place |
| Enchanting [C] | CraftRecipe{option id, 1}; result with the `ench` NBT | 14 × 1, **Place 50 → 14**, then 15 × (index + 1) | moved out by a separate click |
| Anvil [C] | CraftRecipeOptional{0, 0}, **no ResultsDeprecated** | material (2), then input (1) | Place |
| Grindstone [C] | CraftGrindstone{**input stack id**, 1, 0} | 16 (, 17) | Place |
| Loom [C] | CraftLoom{pattern id, 1}; result NBT `{Patterns, Type}` in key order | 9, 10 | Place |
| Trade [C] | CraftRecipe{offer `netId`, 1} | 4 × buyA, 5 × buyB | **Take onto the cursor** |
| Cartography [C] | CraftRecipeOptional{multi recipe or 0, 0}, filter strings `[name or ""]`, cause CartographyText, no ResultsDeprecated | 12 × 1, 13 × 1 | Place |

Anvil: a repair has `custom_names []` and cause -1; a rename `["name"]` with cause AnvilText (-77).
Vanilla repaired and renamed in two crafts; `Bot::anvil` with both does one craft with the name. BDS
crashes on ResultsDeprecated in anvil and stonecutter requests (live-action-tests.md), which vanilla
never sends there. [C] Vanilla's station inputs: whole stacks (enchanting: the one sword), smithing
filled template, material, then base; brewing bottles, ingredient, then blaze powder. The bot
follows those orders. [C]

Recipe book: vanilla's one recipe-book craft (-31, fences) made all it could in one click. The bot
crafts all in one click when that is not more than wanted, else one craft per click
(`book_crafts`). [I] Tag/MoLang cells name the item used up (BDS rejects tag descriptors, status 25).

Manual grid, as vanilla's planks and sticks (-17 to -29): a whole stack for one cell is one Place
from the inventory (label `HotbarAndInventory`); otherwise the stack is taken onto the cursor
(`Hotbar`/`Inventory`), placed per cell, and the rest put back; then a shift-click on the result
sends the craft for all crafts at once. [C] Vanilla took half a stack (right click) and dropped the
rest on another empty slot; the bot takes the whole stack and returns the rest to where it was. [I]
Several results (#3682): ResultsDeprecated lists both, then `Create{0}` + move, `Create{1}` + move.

Beacon (vanilla dump -137/-139): the payment goes into `BeaconPayment` slot 27 by an earlier Place
of 1 (the server gives it a new stack id); confirming sends `BeaconPayment{primary, secondary 0}` then
`Destroy{1, BEACON_PAYMENT 27, that id}`; the server sends no BlockEntityData. Allay (`BeaconPaymentActionProcessor`) and Nukkit-MOT require exactly that Destroy next; PNX looks
for any later Destroy; dragonfly removes the payment itself and ignores the Destroy; Geyser reads
only the first action and sends Java `SetBeacon` (id − 1); PMMP has no beacon. [I] Effect ids:
speed 1, haste 3, strength 5, jump boost 8, regeneration 10, resistance 11, none 0; the
secondary must be 0, the primary (level II) or regeneration, on a level-4 pyramid.

### What each server checks

- **Geyser**: crafting runs a strict state machine (craft → ResultsDeprecated with exactly one
  item → ≥ 1 Consume → Take/Place from 50); `Create` is rejected, so multi-result recipes fail
  there. The bot cannot tell Geyser from BDS and sends the vanilla shape anyway: the request is
  rejected (`ActionError::Rejected`) and nothing changes.
  Recipe-book crafts need an empty cursor and grid, Consumes grouped per non-empty recipe cell in
  recipe order, each group summing to ResultsDeprecated's `timesCrafted`. Placed output counts
  must be multiples of the per-craft result count. Loom reads the pattern from the result's NBT.
  Enchanting, stonecutter and trades map the craft action to a Java button/trade selection. The
  created-output source id must be negative.
- **PMMP**: crafting (manual and auto) and enchanting only; recipe index = net id − 1; all created
  items must be taken in the same request (each before the next `Create`); Consume counts must
  equal n per ingredient; no anvil/grindstone/loom/stonecutter/smithing/trading/beacon. Grid and
  cursor leftovers go back to the inventory on close (`Player::doCloseInventory`); dragonfly
  (`MoveItemsToInventory`) does the same.
- **dragonfly**: recomputes everything, ignores Consume and ResultsDeprecated; recipe net ids are
  `index + 1`; enchant option ids 0-2.
- **BDS**: accepts the vanilla shapes above; sends no slot update for predictable results
  (live-action-tests.md).

### Trades and enchant options

- `UpdateTrade.offers` NBT: `Recipes` list of `{buyA, buyB, sell, buyCountA, buyCountB, uses,
  maxUses, tier, netId, traderExp, rewardExp, demand, priceMultiplierA/B}`; items are
  `{Name, Count, Damage, tag?}`. `netId` is the CraftRecipe id (Geyser: index + 1). [C]
  `buyCountA/B` hold the discounted price. `UpdateTrade` itself opens the screen (no
  ContainerOpen; +68 ms after the ItemUseOnEntity); the bot treats it as the open window. [C]
- Vanilla trade (one offer, 1 emerald → packed ice, twice): Place the **whole** emerald stack (32)
  `HotbarAndInventory 19 → Trade2Ingredient1 4`; per trade `CraftRecipe{netId, 1}` +
  ResultsDeprecated (with the block's runtime id) + `Consume 1` + **`Take` CreatedOutput 50 →
  Cursor** (destination id = the cursor stack's id from the second trade on); then `Place` cursor →
  an empty inventory slot (label `Inventory`), `Place` the 30 leftover emeralds back to slot 19
  (`HotbarAndInventory`), and `ContainerClose{window_id None, window_type None}`; the server echoes
  window 6. [C] The bot does the same; a cursor that cannot take the next result (other item, or
  past 64) is emptied first. [I]
- `PlayerEnchantOptions` comes empty with the open; the 3 options arrive with the response to the
  item's Place. Each `option_id` is the CraftRecipe id (4778-4780 on BDS). Cost = level requirement;
  lapis spent = index + 1 (the 2nd option, cost 3, took 2 lapis). [C]

## Open questions

1. Recipe book "craft one" vs "craft all": only one recipe-book click was captured (all, times 2).
2. Consume order when a cell draws on several stacks: inventory slots ascending (hotbar first). [I]
3. Anvil: repair and rename together in one craft (not captured); combining two damaged items.
4. Enchanting: the result's enchantments come from the option's list (BDS sends no update).
5. Grindstone `cost` 0 and a second item in 17: only a single disenchant was captured.
6. Manual grid: take half vs whole stack, and where leftovers go (see above); grid leftovers after
   an error are shift-clicked back although servers return them on close.
7. Several results: no vanilla capture on 1.26; order of ResultsDeprecated assumed to be the
   recipe's output order.
8. Trade results that are blocks go out with block runtime id 0 in ResultsDeprecated and the
   prediction (vanilla: the block's id; the offer NBT only names the block). Offer `tag` NBT
   (enchanted books) is not carried into the predicted result either.
9. Max stack size is not in the item registry: merges assume 64 (16-stacks like eggs over-fill).

## Cartography

Vanilla dump (windows 21, 22): input Place → `CartographyInput 12`, additional → `CartographyAdditional
13`. Every craft is `CraftRecipeOptional{recipe, filtered_string_index 0}` + Consume 12 × 1 (+ Consume
13 × 1) + Place CreatedOutput → `HotbarAndInventory`, with **no ResultsDeprecated**, `custom_names
[name or ""]` and cause CartographyText. [C]

- Clone (filled map + empty map): recipe = the `CraftingData` multi recipe `442d85ed-…` (4760 on
  that BDS), Place **2**. [C]
- Taking an unchanged map out: recipe 0, `[""]`. Renaming a stack of 2 right after the clone, in the
  same window: recipe 4760, `["2"]`, Consume 1 + Place 1 per item, so **one craft per map**. [C]
  Whether a rename in a fresh window sends 0 or the clone id is unknown; the bot sends 0 unless an
  additional item selects a recipe. [I]
- Zoom out (paper, `8b36268c-…`) and lock (glass pane, `602234e4-…`) were **not captured**; the bot
  sends them like the clone with Place 1. The server makes a new map id for both, so the bot's
  prediction of the result (the input's NBT) is wrong until the server resends the slot. [I]
- Making a map: using a held empty map sends `ItemUse ClickBlock (PlayerInput, Failure)` on the
  aimed block + `ClickAir (UnknownValue, Failure)`, **no StartItemUseOn and no swing**; the server
  resends the inventory and map data. 423 ms later the client sends `MapInfoRequest {map_id,
  ≈16384 client_pixels}`: its own render of the area. That was the only MapInfoRequest of the
  session: none on receiving the cloned or renamed maps. `Bot::make_map` sends the ClickAir and no
  MapInfoRequest (the bot cannot render terrain). [C]
- Not implemented: map upgrading (compass, `98c84b38-…`), making an empty map from paper.

## Timing

From the 2026-10-02 capture (`human.rs`), all screens together (one human, one pass each):

| Delay | Constant | Samples (ms) |
|---|---|---|
| open → first click | `SCREEN_OPEN_LOOK` 600–1700, plus a `CLICK` | n=15, median 1.5 s: stations 942–1899 (7), 2963, 4538, 4572, loom 9961; trader 2210, beacon 1100, cartography 1183, 999 |
| click → click (from the response) | `CLICK` 180–1450 | stations n=25, 226–1544, median 767; trader/beacon/cartography n=14, 477–1152 |
| last input → craft click / option | `CHOOSE` 500–1400 | n=8: 549, 668, 696, 734, 830, 928, 1011, 1364 |
| beacon payment → confirm | `BEACON_CONFIRM` 1500–2500 | n=1: 1972 |
| last click → close | `SCREEN_LINGER` 400–2500 | n=13: 388, 570, 669, 743, 810, 843, 959, 1080, 1213, 1308, 3110, 3384, 4051 |

Left out as idle: 4.5 s and 6.4 s pauses mid-screen, and the 9-12 s waits for a smelt or brew,
which the bot replaces by waiting for the result.
