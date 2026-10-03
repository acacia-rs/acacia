# Gameplay actions against a real BDS (2026-10-02)

Every gameplay action of `acacia-bot` run against BDS 1.26.52.3 (Linux, offline mode, RakNet) with
`examples/actions.rs` and the scene pack `tools/actiontest-pack` (setup and rerun: its README). Where
BDS sends the client nothing, the check asks the pack for the server's own view (`actiontest:inv`).

## Results

Physics bot (`BotConfig { physics: true, trackers: Trackers { entities: true } }`), final run: 23/23.

| Action | Result | Notes |
|---|---|---|
| craft (2x2 and table, recipe book) | PASS | after fix 1 |
| smelt | PASS | |
| brew | PASS | 3 water bottles + nether wart, blaze powder fuel |
| enchant | PASS | after fix 2; server shows the enchantment |
| anvil repair | PASS | after fix 3; server shows damage 0 |
| anvil rename | PASS | after fix 3 |
| grindstone | PASS | after fix 4; server shows no enchantments |
| stonecut | PASS | after fixes 5 and 7 |
| smith | PASS | netherite upgrade |
| loom | PASS | server-side pattern not checked (no script API) |
| trade (wandering trader, villager) | PASS | after fixes 6 and 7 |
| consume | PASS | |
| equip (armour), equip_best_tool | PASS | |
| write_sign | PASS | server text checked by the pack |
| write_book / sign_book | PASS | after fix 8 |
| sleep / wake | PASS | after fix 8 (failed when holding the mispredicted book) |
| mount / dismount (pig, boat) | PASS | after fixes 9 and 10; boat flaky, see below |
| fish | PASS | after fix 11 |
| equip_elytra, start/stop gliding, firework boost | PASS | |
| answer_form | not rerun | verified earlier (tools/formtest-pack) |

Without physics (`... ActionBot idle`) BDS first ignored every block click: idle bots sent
`block_runtime_id` 0, and BDS uses a click only when that id is the server's block.
Idle bots now decode the sub-chunks within one column of them
(`world/near.rs`, feat/itemuse); craft, smelt, write_sign and sleep then passed idle (2026-10-03).
`equip_best_tool` and gliding are physics-only by design.

mount_boat ("not seated") and fish (hook landed on the ground) then still failed idle: idle bots could
not turn, so their inputs kept the server-known rotation, BDS's mount look-ray missed the boat and
the cast went the wrong way. `Bot::look_at` now turns idle bots too (`Idle::look_at`, dropped when the
server moves the player), and idle dismounts step off at the exit spot from the nearby terrain
(`riding/tick.rs`, matches BDS's to 0.001). In mount_pig,mount_boat,fish runs (feat/idlefix,
2026-10-03): fish 23/23 idle; mount_boat 25/27 idle, 12/13 physics (the flake below). The ride check
now compares where the bot stepped off with the server's first positions: later the server pushes the
player off the boat, which itself gets pushed.

feat/itemuse added `equip_use` (armour by use) and `pickup` (the pack drops wheat and apples at the
bot's feet; the bot's slots must match the server's). Both pass, physics and idle.

## Bugs fixed

| # | Symptom on BDS | Root cause | Commit |
|---|---|---|---|
| 1 | Sticks: status 25 (FAILED_TO_MATCH_EXPECTED_ALLOWED_ANYWHERE_CONSUMED_ITEM) | `CraftRecipeAuto` listed the recipe's item-tag ingredient; BDS matches consumed items only against concrete descriptors. Tag/MoLang cells now name the item used | e43ec98 |
| 2 | Enchant: status 37 (FAILED_TO_ENCHANT); then the bot's sword had no enchantment | minecraft-data types `EnchantOption.option_id` zigzag32, BDS writes an unsigned varint (4851 read as -2426); codegen override. BDS sends no slot update after the craft, so the result is now predicted with the option's enchantments | ac9798a |
| 3 | Anvil: **BDS crashed** (whole server) on every anvil request; then status 5 | A `CraftResultsDeprecated` in an anvil request crashes BDS, even with the exact result; without it the anvil works. The material `Consume` must be what the repair uses up (4 ingots for damage 200, not 1): `workstation/repair.rs` | 1b80cde |
| 4 | Grindstone: status 9 (INVALID_CRAFT_RESULT) | Its "recipe" id is an `ItemStackNetIdVariant` (Mojang docs): the input stack's id, not 0 | 1b80cde |
| 5 | Stonecutter: **BDS crashed** | Same as 3: no `CraftResultsDeprecated` in stonecutter requests | cfa74a5 |
| 6 | Trades: "not enough emerald to pay" | Offers carry payment `Damage` 32767 (any aux); the bot only took 0 as a wildcard | fd62edb |
| 7 | Trade paid with an awkward potion: status 25; stonecutter after grindstone never opened | Damage 0 is a real aux value. BDS adds `RepairCost 0` to the grindstone result; the bot's held copy lacked it, and BDS rejects (resyncs) a block click made holding an item that differs from its copy | 6db12ac |
| 8 | sign_book "not signed"; bed click after it ignored | BDS echoes neither page edits nor signing. The held book now gets the edits (`pages: [{photoname, text}]`) and becomes the written book locally, as the client does | 2f566c3 |
| 9 | After leaving the boat the bot stood a block underground | Leaving a client-predicted vehicle subtracted the eye height from the vehicle's position | ce10ea8 |
| 10 | Same symptom, from the server side | `CorrectPlayerMovePrediction` of type Vehicle (the driven boat) was applied to the player | 144ebdb |
| 11 | fish: reeled in at once; real catches never seen | The Geyser dip heuristic took the cast's apex for a bite (now needs 3 resting updates). BDS reports the catch only as `AddItemEntity` (`is_from_fishing`) + `TakeItemEntity`, never as a slot update | 531383c |

After the refit to the vanilla capture (feat/livefix):

| # | Symptom on BDS | Root cause |
|---|---|---|
| 12 | craft: status 27 (ConsumedItemNotAllowed) on the first recipe-book craft | In the own inventory screen the recipe-book `Consume`s named the slot `Hotbar`; BDS only lets crafts consume from `HotbarAndInventory`. Automatic moves now use it on every screen |
| 13 | trade_villager: "no trade screen opened", only late in a full run | Scene artifact, not the bot: the summoned farmer does not keep its job and takes one at a nearby station (seen as shepherd and cleric, even with a composter beside it); between jobs it cannot trade. The check retries and logs the villager's profession (`actiontest:ent`) |

Also: the session logs BDS's `PacketViolationWarning` (it drops the client after one without a
`Disconnect`), 76a1150.

## What BDS showed about vanilla's requests

- **Slot updates are not sent for client-predictable changes**: craft results, enchanting, grindstone,
  anvil, page edits, signing. Script-made changes (`Container.addItem`) and `/enchant` were not sent
  either. **Pickups are sent**: an S>C `InventoryTransaction` (Container slot → new stack, plus a
  WorldInteraction action) just before `TakeItemEntity`. Armour by use gets `InventorySlot` Armor and a
  full `InventoryContent`. The bot has to predict exactly; a wrong held item gets its next block click
  rejected (BDS resends nearby blocks and the inventory).
- **BDS crashes** on `CraftResultsDeprecated` in anvil and stonecutter requests (crafting table, recipe
  book, enchanting, grindstone, smithing, loom and trades accept it). Vanilla evidently does not send it
  there; Geyser's dumps only covered crafting.
- `CraftGrindstone`'s id field is an `li32` on the wire (minecraft-data is right, gophertunnel's varint
  is malformed for BDS: PacketViolationWarning "the input must be a number >= 0") and holds the input
  stack's id.
- Anvil renames with filter string + cause AnvilText are accepted; the material count is checked.
- Fishing: BDS sends `FishHookTease` (many), `FishHookPosition`, then one `FishHookHook` for the bite.
- Trades: payment `Damage` 32767 means any aux value; wandering trader offers include potions (water).
- Villagers are now `minecraft:villager` (`villager_v2` no longer parses in `/summon`).
- While sleeping, BDS keeps the player at the bed; physics bots now report it (`sleep/tick.rs`): 0 corrections
  in 40 ticks asleep, one on waking (the server's stand-up spot).

## Still open

- ~~mount_boat right after mount_pig~~ (no link, no refusal): fixed on feat/refix-riding by aiming at the boat's
  real hitbox (BDS's look-ray check missed a player-sized box above it) and leaving the pig at the server's
  exit spot; 10/10 boat mounts in the sleep,mount_pig,mount_boat,fish sequence.
- mount_boat "not seated" in ~1 of 13 runs, idle and physics alike. The one diagnosed case: the boat had
  been pushed to x 1.73 (1.70 in every passing run), bot and server agreed on it, aim and eye as usual.
- Idle bots cannot click blocks outside the decoded columns, or in sections whose blob came from an
  earlier session's hash-only cache (`NotPossible`).
- Physics while sleeping and vehicle physics are not modelled (see above, and
  research/riding-fishing-elytra.md).
- Not covered: cartography, beacon, manual grid crafting (another branch), horse riding.
