# Viewer: what it does, against the native client

State on 2026-10-09. `acacia-viewer` is a playable client in two looks (Bedrock, Java). How each
piece works is in the crates' READMEs (acacia-render, acacia-ui); this is the checklist.

"Live" means seen in a testbox screenshot (`tools/testbox-shot.sh`) against a BDS; "tests" means
unit or pack tests only.

## Done

| Area | What | Checked |
|---|---|---|
| World | Both looks' blocks, biomes' colours, lighting, liquids, block-entity models | live; Java blocks by `lookbake java-check` |
| Sky | Sun, moon, stars, sky disc, sunrise glow, fancy clouds | live |
| Weather | Rain, storm sky and clouds, lightning bolts and flash | live |
| Weather | Snow by biome, none in dry biomes | tests (the test worlds have no cold biome) |
| In fluids | Water, lava and powder snow fog per look; Java's underwater overlay | live (powder snow: not seen) |
| Particles | Server particles; torches, fires, furnaces, campfires, drips; block chips | live, both looks |
| Entities | Pack models and animations, skins, name tags, dropped items | live |
| Entities | Held items, worn armour (leather dyed its default), hurt flash | live |
| Entities | Enchantment glint on held items, worn armour and slot icons | live in the Java look (armour: faint on netherite, not told apart on diamond); Bedrock look: faint, its rules assumed; dropped items and other entities' hands: tests |
| Entities | Death topple | tests |
| Entities | Round shadows on the ground (radius from the hitbox) | live |
| Entities | Translucent shells (slime, sulfur cube) | live (slime) |
| Signs | Text on standing, wall and hanging signs, dye and glow | live, both looks |
| Banners | Standing and wall banners with their dye and patterns, the ominous banner; no sway | live, both looks (20 of 42 patterns seen; composition by tests) |
| Moving | Walking, sprinting, sneaking, jumping, swimming (acacia-physics, server-checked) | bot traces |
| Moving | Creative flight (Space twice) | live, 0 corrections on a strict BDS |
| Moving | Dimension travel: by command, joining in another dimension | live |
| Moving | Dimension travel through Nether and End portals | bot drill `dimensions` (0 corrections); not seen in the viewer |
| Moving | Riding: boat, horse, pig, minecart; seat camera, sitting pose | live (pig pose: not seen) |
| Camera | View bobbing, sprint and flight FOV, hurt roll, death tilt, F5 views | live (death tilt: tests) |
| Acting | Mining with cracks and predicted break, placing, using, attacking | live |
| Acting | Eating, drinking and the bow in first person | bow pose live, not compared with the game; eating: untested, never caught in a shot |
| HUD | Hotbar, hearts, food, armour, air, XP, item name, effects, boss bars, titles, action bar | live |
| HUD | Chat with translations (rawtext and keys), Tab list, sidebar, F3 | live |
| Screens | Inventory with the player's figure, chests, recipe book and crafting by click | live |
| Screens | Crafting by hand: the 2×2 grid and the crafting table's 3×3 | table live; 2×2: tests |
| Screens | Anvil with its name box, smithing table, stonecutter, enchanting table | live |
| Screens | Grindstone, cartography table (no renaming) | tests only, never opened live |
| Screens | Loom: patterns as buttons (the pattern item's one, else the plain 32); the result's icon shows no patterns | live |
| Screens | Beacon: powers, second power and confirm as buttons (not either game's own screen) | live |
| Screens | Creative inventory (E in creative mode): four tabs, a click takes one, Shift a stack | live |
| Screens | Trading: the offers of a villager or wandering trader, a click trades once | live (wandering trader) |
| Screens | Furnace family, hopper, dispenser/dropper, brewing stand | live (acacia-46) |
| Screens | Server forms, pause, options (two columns), death and respawn | live |
| Sound | Each look's own samples: Bedrock `.fsb` (FADPCM), Java `.ogg` | logs (testbox has no audio device) |

## Not done

- **Screens:** the loom's banner preview; the beacon's own art and which powers its pyramid allows; trader levels; a
  cartography table's name box, the anvil's level cost; a scroll bar on the stonecutter's and
  trader's lists (the wheel scrolls them); enchanting hints and the galactic text; craft
  results onto the cursor (they go into the inventory); books; maps; the game's own sign editor
  (a four-line form stands in); the creative inventory's search, its survival-inventory tab and
  deleting items into it.
- **Dimensions:** portals were checked in the bot's drill only, not walked through in the viewer;
  an arrival can take one correction when the server lags (docs/DESIGN.md "Dimension travel").
- **Entities:** the charged creeper's aura, patterns on held banners and shields, the banner's
  sway, a leather
  stack's own dye, the off hand, capes, boat paddles, camels, leads, fishing lines.
- **Sky:** Java's fast clouds; the Nether's fog per biome (the nether wastes' is used; never seen
  live); the End's sky was seen live, its terrain was not (see Dimensions).
- **Bedrock look:** weather, clouds, sky colours and first-person poses use Java's rules; Bedrock's
  own are unmeasured.
- **Sound:** music, ambience, and Java's event mapping where a Bedrock sound has no Java file.
- **Performance:** only measured on lavapipe (CPU); no GPU frame times recorded.

## Testing it

- `tools/testbox-shot.sh out.png` with `NAME=<yours>` (same-name runs kick each other) and a BDS
  port. Env: `ACACIA_PLAY`, `ACACIA_LOOK`, `ACACIA_SETTINGS`, `ACACIA_SHOT_AFTER`.
- `ACACIA_COMMANDS="tp @s 20 -60 20;wait 3;summon zombie ~ ~ ~4"`: operator commands, `wait secs`
  between them (`src/net/setup.rs`). A summon right after a far teleport fails: wait first.
- `ACACIA_KEYS="5 +W; 9 -W; 12 +Use; 15 click 640 300; 16 type Hello"`: scripted input
  (`src/keyscript.rs`).
- `/enchant @s ...` sent through `ACACIA_COMMANDS` had no effect (2026-10-09, no output either);
  written to the BDS console fifo (`echo "enchant <name> sharpness 1" > work/bds-forms/console`)
  it works. Armour is put on by holding it and `ACACIA_KEYS="18 +Use; 18.3 -Use"`.
- Patterned banners: no command writes them, so `cargo run -p acacia-bot --example banners -- <server> [x y z]`
  looms and places a row (it runs commands only on itself and within 40 blocks).
- `RUST_LOG=chat=debug` logs each chat line's raw packet.
