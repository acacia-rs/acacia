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
| Entities | Death topple | tests |
| Entities | Round shadows on the ground (radius from the hitbox) | live |
| Signs | Text on standing, wall and hanging signs, dye and glow | live, both looks |
| Banners | Standing and wall banners with their dye and patterns, the ominous banner; no sway | live, both looks (20 of 42 patterns seen; composition by tests) |
| Moving | Walking, sprinting, sneaking, jumping, swimming (acacia-physics, server-checked) | bot traces |
| Moving | Creative flight (Space twice) | live, 0 corrections on a strict BDS |
| Moving | Riding: boat, horse, pig, minecart; seat camera, sitting pose | live (pig pose: not seen) |
| Camera | View bobbing, sprint and flight FOV, hurt roll, death tilt, F5 views | live (death tilt: tests) |
| Acting | Mining with cracks and predicted break, placing, using, attacking | live |
| Acting | Eating, drinking and the bow in first person | bow pose live, not compared with the game; eating: untested, never caught in a shot |
| HUD | Hotbar, hearts, food, armour, air, XP, item name, effects, boss bars, titles, action bar | live |
| HUD | Chat with translations (rawtext and keys), Tab list, sidebar, F3 | live |
| Screens | Inventory with the player's figure, chests, recipe book and crafting by click | live |
| Screens | Furnace family, hopper, dispenser/dropper, brewing stand | live (acacia-46) |
| Screens | Server forms, pause, options (two columns), death and respawn | live |
| Sound | Each look's own samples: Bedrock `.fsb` (FADPCM), Java `.ogg` | logs (testbox has no audio device) |

## Not done

- **Screens:** anvil, enchanting, trading, beacon, loom and the other Ui-slot stations; books; maps;
  the game's own sign editor (a four-line form stands in); creative inventory tabs; the crafting
  table's 3×3 grid by hand (the recipe book crafts).
- **Entities:** enchantment glint, creeper and slime overlays, patterns on held banners and shields, the banner's sway, a leather
  stack's own dye, the off hand, capes, boat paddles, camels, leads, fishing lines.
- **Sky:** the End's and the Nether's skies, Java's fast clouds.
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
- Patterned banners: no command writes them, so `cargo run -p acacia-bot --example banners -- <server> [x y z]`
  looms and places a row (it runs commands only on itself and within 40 blocks).
- `RUST_LOG=chat=debug` logs each chat line's raw packet.
