# Viewer: what it does, against the native client

State on 2026-10-09. `acacia-viewer` is a playable client in two looks (Bedrock, Java). How each
piece works is in the crates' READMEs (acacia-render, acacia-ui); this is the checklist.

"Live" means seen in a testbox screenshot (`tools/testbox-shot.sh`) against a BDS; "tests" means
unit or pack tests only.

## Done

| Area | What | Checked |
|---|---|---|
| World | Both looks' blocks, biomes' colours, lighting, liquids, block-entity models | live; Java blocks by `lookbake java-check` |
| World | Vines, glow lichen, sculk veins and resin clumps on the faces they cling to; levers | live, both looks. The Bedrock look's lever is Java's model (the pack has no shape for it): at rest and thrown on a floor and a wall look the same in both |
| World | Piglin heads (Java's model: the pack ships no geometry for it) on floors and walls; tripwire as a string over the floor | live, both looks. In the Bedrock look tripwire is drawn both ways across its block whichever way it runs |
| World | Nether portal blocks: a translucent sheet across the frame, with the pack's swirl | live, both looks (a frame lit with fire) |
| Sky | Sun, moon, stars, sky disc, sunrise glow, fancy and fast clouds (Options) | live (fast clouds: Java look) |
| Weather | Rain, storm sky and clouds, lightning bolts and flash | live |
| Weather | Snow by biome, none in dry biomes | tests (the test worlds have no cold biome) |
| In fluids | Water, lava and powder snow fog per look; Java's underwater overlay | live (powder snow: not seen) |
| Particles | Server particles; torches, fires, furnaces, campfires, drips; block chips | live, both looks |
| Entities | Pack models and animations, skins, name tags, dropped items | live |
| Entities | Held items in both hands, worn armour, hurt flash | live |
| Entities | Java look: 15 mob kinds on Java's models (cows, mooshrooms, pigs, sheep, chickens, creeper, spiders, zombies, husks, drowned, skeletons, strays, wither skeletons, bogged; own-model babies) posed by a port of their `setupAnim`; armour on Java's slot models with the equipment assets' textures; wool, clothes and the creeper's aura as their own models; lit by the lightmap and Java's two lights | parity harnesses `entity_models`, `entity_poses`, `entity_layers`; not seen live this session |
| Items | Java look: items from the 26.3 item models (every display context, tints, foil), the GUI slot icons drawn from them, first-person placement with swings and use animations | parity harnesses `item_models`, `item_transforms`; slot icons by tests; not seen live this session |
| Entities | Leather dyed worn, held and in slots: undyed brown, or the stack's own dye | undyed: live; a stack's dye: tests (no command dyes armour) |
| Entities | Enchantment glint on held items, worn armour and slot icons | live in the Java look (armour: faint on netherite, not told apart on diamond); Bedrock look: faint, its rules assumed; dropped items and other entities' hands: tests |
| Entities | Death topple | tests |
| Entities | Round shadows on the ground (radius from the hitbox) | live |
| Entities | Translucent shells (slime, sulfur cube) | live (slime) |
| Entities | The shield as its model in either hand; first person draws both hands | live, both looks: an armour stand's main hand, first person at rest; Java look only: the player's two hands from behind. Raised while sneaking: live (Java look), in first person and from the front (Java's `shield_blocking` placement; animations read `query.blocking` and the held items' names) |
| Entities | The charged creeper's aura | live, both looks, beside a plain creeper |
| Entities | Boat paddles and a dry hull | live, both looks (an empty boat on a pool) |
| Entities | Paddles rowing by the boat's paddle times | live in the Java look (one frame, from behind); the cadence is assumed |
| Entities | Fishing lines (hook to its owner's rod) and leads (mob to its holder or fence knot), sagging as Java's | fishing line: live, first person (Java look); a lead from a cow to another player's hand: live (Java look); other players' fishing lines: tests; other holders' hand anchors are approximate |
| Entities | Players crouch while sneaking (the server's flag; the own player's input) and raise a shield with it, the arm in Java's blocking pose | live for another player (Java look, the `props` bot). BDS did not set the blocking flag on it: a sneaking shield holder counts as blocking |
| Entities | Capes: the skin's cape image on the game's cape sheet, hung at Java's rest angle, leaning with a sneaking wearer and lifting with its pace | live on a standing, sneaking player from behind and the side (Java look, `props`); lifted behind a walking one (`props … pace`; any direction lifts it, no sideways sway); a persona skin's cape image would arrive the same way but none of the pool skins has one |
| Entities | A shield's banner: the cloth and patterns from `shield_patterns`, on the model and the slot icon | live (Java look): a shield the `props … shield` bot crafted a loomed banner onto, raised in its hand; the slot icon looked at from both packs' textures (`cargo run -p acacia-render --example shield_icon`) |
| Maps | A filled map held in first person shows the picture the server sends, on the map sheet: in both hands lying flat until the holder looks down (Java's placement), or beside the view when the other hand is full | live (Java look): lying flat, upright and one-handed. Markers are arrowheads in the marker's colour (neither pack ships the game's marker images): the holder's seen live, at its place and pointing its way. The arms holding it are drawn (seen live under the upright map and beside the one-handed one). Not drawn: marker labels; other hands and slots show the item's icon |
| Item frames | Frames on walls, floors and ceilings with their item at half size, or a map's picture over the block's face (asked of the server once) | live (Java look): a sword, an apple, a block and a map on a wall, a sword on the floor and under a ceiling, filled by the `frames` bot. A turned item turns clockwise (seen live); which way a lying item points is unchecked |
| First person | The player's bare arm while the main hand is empty, placed and swung as Java's | live (Java look): at rest, and swinging while it mines (`ACACIA_ATTACK=1`) |
| Signs | Text on standing, wall and hanging signs, dye and glow | live, both looks |
| Banners | Standing and wall banners with their dye and patterns, the ominous banner; the flag sways | live, both looks (20 of 42 patterns seen; composition and sway by tests) |
| Moving | Walking, sprinting, sneaking, jumping, swimming (acacia-physics, server-checked) | bot traces |
| Moving | Creative flight (Space twice) | live, 0 corrections on a strict BDS |
| Moving | Dimension travel: by command, joining in another dimension | live, Nether and End, with their fog and sky |
| Moving | Dimension travel through Nether and End portals | bot drill `dimensions` (0 corrections); not seen in the viewer |
| Moving | Riding: boat, horse, pig, minecart; seat camera, sitting pose | live (pig pose: not seen) |
| Camera | View bobbing, sprint and flight FOV, hurt roll, death tilt, F5 views | live (death tilt: tests) |
| Acting | Mining with cracks and predicted break, placing, using, attacking | live |
| Acting | Eating, drinking and the bow in first person | bow pose live, not compared with the game; eating and drinking: the pose by test (bottom centre through the use), the rise and the emptied bottle seen live |
| HUD | Hotbar, hearts, food, armour, air, XP, item name, effects, boss bars, titles, action bar | live |
| HUD | Chat with translations (rawtext and keys), Tab list, sidebar, F3 | live |
| HUD | Java look: crosshair, hotbar, offhand, hearts, food, armour, air and XP drawn by the ported 26.3 HUD (`acacia_ui::java`) with the game's fonts and sprite scaling. Effects, titles, boss bars, chat and screens still come from the older layout | the draw calls match the game's (parity `hud`, `font`); not seen live this session |
| Screens | Inventory with the player's figure, chests, recipe book and crafting by click | live |
| Screens | Crafting by hand: the 2×2 grid and the crafting table's 3×3; a click takes the result onto the cursor, Shift into the inventory | table live; live |
| Screens | Anvil with its name box and level cost, smithing table, stonecutter, enchanting table | live (the cost: both looks, a rename). A cost above the player's levels is red, and 40 or more reads "Too Expensive!": the red line by its pixels in a testbox shot (a levelless rename, image not looked at), "Too Expensive!" by the bot's tests only; the result stays empty in both cases |
| Screens | A scroller on the stonecutter's and the trader's lists when they are longer than their window; the wheel moves them a row | live (Java look): the stonecutter's moved by the wheel, the trader's seen at rest Bedrock look: the stonecutter's cuts for stone, one picked and its tooltip (2026-10-11). |
| Screens | Enchanting options: the server's words in the galactic alphabet (Java's `ascii_sga.png`, also in the Bedrock look), and under them the level and the enchantment hinted at | live, both looks. A Java look baked before 2026-10-11 lacks the alphabet: run `lookbake java` again |
| Screens | Grindstone | live (Java look): an enchanted sword put in, the plain one shown as the result, taken into the hotbar with its experience Bedrock look: opened with an enchanted sword in, the result and the sword's tooltip shown (2026-10-11). |
| Screens | The name of the stack under the mouse, in Java's tooltip box, while no stack is carried: slots, creative items, a stonecutter's cuts | live over a hotbar slot (Java look), with an enchanted sword's two enchantments in grey under its name. An anvil's name (italic) and lore lines by the same code, unseen. None on recipe-book results, trades or patterns; no attribute or durability lines |
| Screens | Cartography table, with a name box where the title would be (Bedrock's table renames; Java's has no box, so the place is ours) | live (Java look): a map put in and named, the result shown; taking it and the paper and pane recipes unseen. The box was moved 3 pixels down after the shot Bedrock look: opened with a map in, the name box and tooltip shown (2026-10-11). |
| Screens | Sign editor: the board with its four lines, Done or Esc writes; a hanging sign's smaller board and narrower lines | live, both looks |
| Screens | Books: a right-click opens the held book's pages; a book and quill takes typing at its caret (arrows, Home and End move it; Backspace and Delete) and new pages (the page keys turn them), and is signed under a title | live; the caret inside a word seen live (Java look), no Up or Down |
| Screens | Loom: patterns as buttons (the pattern item's one, else the plain 32); the result shows the banner it makes, and so does a large preview in the frame above it (the result's icon at twice its size, not Java's banner model) | live, both looks |
| Screens | Beacon: Java's screen in both looks (its sheet and button sprites in the Java look, plain panel and cells in the Bedrock look), each power a button with its effect's icon; a power the pyramid under it is too small for is disabled | live, both looks (a two-layer pyramid: four powers open, three disabled, haste picked). No Cancel button (Esc closes), no payment-item pictures, level II shows "II" for the power's icon. The Bedrock look's headings were made dark after the shot |
| Screens | Creative inventory (E in creative mode): four tabs, a click takes one, Shift a stack; a stack carried onto the items is destroyed; a fifth tab ("Self") shows the player's own screen under the tab buttons; a sixth ("Find") lists every tab's items whose identifier holds the typed text | live (the search: "diamond s" left the sword, spear and shovel); the fifth tab by tests and by pixels sampled from a shot on testbox (the image was not looked at). The search reads identifiers, not the translated names, and has no text box art Bedrock look: the search ("lantern") and the own-screen tab seen (2026-10-11). |
| Screens | Trading: the offers of a villager or wandering trader, a click trades once; a villager's level beside its name, later levels' offers crossed out ; its experience bar under the name | live (wandering trader; a villager: "Mason - Apprentice" after 16 trades, its bar a tenth full and only its two levels' offers open, Java look). acacia-proto reads `UpdateTrade`'s tier and unique ids as plain varints though they are zigzag; the bot undoes that (`state/stations.rs`) until the generated reader is fixed |
| Screens | Furnace family, hopper, dispenser/dropper, brewing stand | live (acacia-46) |
| Screens | Server forms, pause, options (two columns), death and respawn | live |
| Sound | Each look's own samples: Bedrock `.fsb` (FADPCM), Java `.ogg` | logs; played into a real audio device once (2026-10-11, the dev machine, Bedrock look: 47 sounds mixed in 45 s, steps and hits among them), with nobody listening |
| Sound | Music: a track of the dimension's set five seconds after joining (the creative set in creative mode), then the pack's pause before the next; decoded on its own thread | the first track's start in the log (`RUST_LOG=music=info`), and a pack test decodes a track (261 s of stereo); never heard (no audio device). Neither asset set ships music: bake the Java look with `LOOKBAKE_MUSIC=1` (248 MB), which both looks then play. Bedrock's numbered tracks are found under Java's titles (`acacia-sound/src/java_names.rs`), the pairing unchecked by ear |

## Not done

- **Screens:** Bedrock's own beacon screen and its two-page book (the Bedrock look puts the pack's
  cover and page under Java's one-page layout, seen live; the Java look draws `book.png`, seen
  live; both looks' turn buttons are plain buttons; a look baked before 2026-10-11 lacks the art).
- **Dimensions:** a nether portal was walked through in the viewer (2026-10-10, creative, to a
  crimson forest), and an end portal block into the End (2026-10-11: the dimension change in
  the log, the dragon's bar in the shot) and back out of it (a portal block on the End's platform: the
  credits skipped, the respawn at the world's spawn in the log and the shot), and back from the Nether through a
  portal lit around the player there (the change to the overworld at eight times the
  coordinates in the log, the shot in the overworld). An
  arrival can take one correction when the server lags (docs/DESIGN.md "Dimension travel").
  The end portal's surface is the Bedrock pack's still image in both looks (seen live; the
  Java game draws it with a shader). A Java look baked before 2026-10-10 lacks it: run
  `lookbake java` again.
- **Entities:** a banner stack's patterns were seen on the model in another player's hand
  (2026-10-10, Java look); its slot icon and a shield's banner are by tests only. A camel was seen standing, saddled and ridden (2026-10-10), not
  sitting or dashing. The Bedrock attachable's own shield placement (Java's is used in both
  looks).
- **Sky:** The Nether's per-biome fog is in (unit-tested; only the crimson
  forest was seen live, before it landed).
- **Bedrock look:** weather, clouds, sky colours and first-person poses use Java's rules; Bedrock's
  own are unmeasured.
- **Sound:** ambience, and Java's event mapping where a Bedrock sound has no Java file. Music
  plays by dimension only (no biome sets, no menu music), has no volume of its own, and counts
  its pause from four minutes after a track starts, not from its end.
- **Movement on the dev machine (Windows, 37 ms to the server):** felt wrong to the user
  (2026-10-11, "teleported around"). Measured there: 20 ticks took 1.0 to 1.8 s with stalls of
  several hundred ms, and the server corrected the player 13, 3 and 0 times in three like walks.
  The tick schedule no longer slides when a wake-up is late (`acacia-bot/src/cadence.rs`): in
  eight runs with it there, 20 ticks took 1.0 to 1.1 s throughout, the longest gap mostly 64 to
  136 ms. Corrections still come and go, cause unknown: of five walks with sprint-jumping, three
  had none, one had one and one had five, each in mid-jump (the same route run again had none,
  so it is not a block on the way); and once, standing where it joined, the player was
  corrected 19 times in 6 s (two later joins: none). The user has not yet said how it feels.
  2026-10-21 ("sprint teleports back / cancels"): releasing Ctrl with W held ended our sprint but
  not the server's, which then corrected every ~1.3 s (13 times in one sprint-jump run). The key
  now latches with mouse input (DESIGN.md "Movement vs the servers"): 0 corrections in 7 runs.
  Draw stalls of 0.3 to 5.4 s were the light thread holding its write lock over a whole batch of
  columns (each frame reads light for entities and particles); it now locks per column, and frames
  never wait on it (`gpu/light_probe.rs`: while it is held, last frame's values): no draw over
  100 ms after joining in two walks (before: 17 in the first 5 s). Slow draws log their stages
  (`RUST_LOG=frame=warn`, `gpu/stages.rs`). The 0.8 to 4.3 s frame at joining, and 0.3 s ones
  when a sound first played, were sound loading on the frame: it has its own thread now
  (`audio.rs`). The world's setup still takes 0.1 to 0.9 s once per join (`frame=debug` logs
  its steps; mostly the block table and the texture upload).
- **Text:** the only font is the Java look's `ascii.png`: without a baked Java look the viewer
  has no text (it warns). Baked looks are found beside `ACACIA_ASSETS` (`<pack>/../looks`).
- **Six-way blocks** (pistons, dispensers, droppers, observers) face their `facing_direction`
  (`blocks/facing.rs`): seen live from the front (2026-10-21), sideways pistons and dispensers also
  from above (2026-10-22). The observer's top arrow points at its back as in Java's model, which
  flips that face; not seen live (four testbox shots missed the row).
  `RUST_LOG=moves=debug,keys=debug` logs every correction with the blocks under the feet, the
  position and longest tick gap each 20 ticks, and each scripted key.
- **Performance:** on a GPU only a light scene was measured (2026-10-11, Intel Arc integrated,
  Vulkan, 2560×1440, vsync off, the flat test world at radius 8: 257 sections, about 2k quads):
  166 to 375 fps over 45 s, 420 MB resident. No terrain-heavy world, no frame-time spread, no
  comparison with the game. On lavapipe (CPU) testbox gives 25 to 40 fps at 1280×720.

## Testing it

- `tools/viewer-shot.sh` prints only the last 15 lines of the viewer's log: pass `LOG=<file>` to
  keep all of it (2026-10-11: three runs misread because the dimension lines had scrolled out).
- `tools/testbox-shot.sh out.png` with `NAME=<yours>` (same-name runs kick each other) and a BDS
  port. Env: `ACACIA_PLAY`, `ACACIA_LOOK`, `ACACIA_SETTINGS`, `ACACIA_SHOT_AFTER`.
- `ACACIA_COMMANDS="tp @s 20 -60 20;wait 3;summon zombie ~ ~ ~4"`: operator commands, `wait secs`
  between them (`src/net/setup.rs`). A summon right after a far teleport fails: wait first.
- `ACACIA_KEYS="5 +W; 9 -W; 12 +Use; 15 click 640 300; 16 type Hello"`: scripted input
  (`src/keyscript.rs`). Its clock starts at the player's first tick, `ACACIA_SHOT_AFTER`'s at
  launch (4 to 9 s earlier): leave that much between the last step and the shot.
- `/enchant @s ...` sent through `ACACIA_COMMANDS` had no effect (2026-10-09, no output either);
  written to the BDS console fifo (`echo "enchant <name> sharpness 1" > work/bds-forms/console`)
  it works. Armour is put on by holding it and `ACACIA_KEYS="18 +Use; 18.3 -Use"`.
- Patterned banners: no command writes them, so `cargo run -p acacia-bot --example banners -- <server> [x y z]`
  looms and places a row (it runs commands only on itself and within 40 blocks).
- Item frames: no command fills one, so `cargo run -p acacia-bot --example frames -- <server> [x y z]`
  hangs a row on a wall and fills it, the last with a map.
- A second player: `cargo run -p acacia-bot --example props -- <server> [x y z]` stands there
  in a cape (`ClientBuilder::skin`), facing north, leashes a cow, looms a patterned banner into its hand and sneaks with a shield
  for a minute.
- `RUST_LOG=chat=debug` logs each chat line's raw packet. A setup command's syntax error is not
  sent back: write the command to the BDS console fifo and read `bds.log` (the BDS on 19174 runs
  in `work/bds-render`).
- A villager with trades: `summon villager x y z 0 0 minecraft:spawn_farmer` (`villager_v2` is
  not a name this BDS takes).
- The action-test pack (BDS 19174) rebuilds its scene around wherever the player joins, which
  is where it last left: stay there, summon on a stage some 40 blocks off and shoot it with
  `ACACIA_AT`. A charged creeper left near the join point blows up at the next join.
- `testbox-shot.sh` keeps local mtimes: a file edited while the first build of a directory ran
  is older than that build and never rebuilt. `touch` it there (2026-10-10: three shots showed
  the old binary).
- A boat's paddle times are in the `riding` log line (`src/ride.rs`).
