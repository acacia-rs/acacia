# Java Edition support

One library, both editions: the same `Bot` drives a Bedrock or a Java server, and pathfinding, building and
survival logic are shared. Decisions settled with the user on 2026-11-07.

## Decisions

| Area | Choice | Why |
|---|---|---|
| Home | Inside acacia, behind a cargo feature | One library; swapping editions is a connect-time choice |
| API shape | One `Bot`; edition chosen at connect (`Connect::java(..)` / `Connect::bedrock(..)`); edition-only extras via `bot.bedrock()` / `bot.java()` → `Option<_>` | swarm, the JS package and pathfinding keep one concrete type; dispatch cost is negligible next to the network |
| Names | Generated neutral `Block` / `Item` enums with typed properties (`state.get(prop::Facing)`); strings parse in either edition's spelling | Code is edition-free; per-block state structs (azalea-style) rejected as too large, string properties rejected because property names differ (`pillar_axis` vs `axis`) |
| World storage | Each edition's world keeps its native state ids; the neutral state is a view | Geyser's Java→Bedrock map is many-to-one; physics and placement need the exact native state |
| Java version | Pin 26.3, bumped together with the Bedrock pin (26.50) | Same single-version rule as Bedrock; ViaVersion servers accept newer clients |
| Codecs | Own `acacia-java-proto`, stable Rust, network-free like `acacia-proto` | azalea's 26.3 branch needs nightly-2026-08-15, git deps block crates.io, and its layouts were wrong (Login game mode, LevelParticles) |
| Codec source | Packet ids and registries from the 26.3 server jar's data-generator reports; layouts hand-written, checked with `javap -p -c` on the codec classes, round-tripped against a vanilla server and client | Reports don't carry field layouts |
| Physics | Bit-exact against the vanilla client: port LocalPlayer/Entity movement from the unobfuscated 26.3 jar (doubles where Java uses doubles); judged per tick against traces recorded by a mixin in the acacia-java-bridge Fabric mod under Xvfb | Exact positions keep Grim-style anticheats quiet; same bar as the Bedrock side |
| Bedrock-typed accessors | `client()`, `movement()`, `world()` move to `bot.bedrock()` (2026-11-10); no shims | Nothing outside depends on them yet; the JS branch (`js/cancel`) and the bridge's pinned rev update when they next take this branch |
| Shared logic | Pathfinding, build and survival plan against edition-neutral world + a physics trait; costs (jump/sprint reach) come from the edition's physics | Movement envelopes differ between editions |
| Licence | MIT; Mojang's jar is read for behaviour, not copied; GPL sources (Pumpkin, ViaBedrock) only as reference | Same rule as DESIGN.md |

## Build order

Each step ships on its own; Bedrock behaviour stays unchanged throughout.

1. **Neutral enums.** Generate `Block`, `Item` and typed properties from both editions' data (Bedrock registry +
   26.3 reports; Geyser mappings for the joins). Add a compat shim, then migrate acacia crate by crate off
   string names (~1,700 sites across 214 files on 2026-11-07; the bridge is the only external user).
2. **Java client, read-only.** `acacia-java-proto`; TCP + zlib + AES-CFB8 transport; login → configuration →
   play; Java auth via acacia-auth (Minecraft-services XSTS). A bot that joins and fills a Java world from
   chunk and entity packets. Java collision shapes come from the jar, not BDS.
3. **Java physics.** Port and fit against client traces until bit-exact. Crate `acacia-java-physics`; recordings
   in its `tests/traces`, made with the bridge's `tools/move-trace.sh` (branch `java-physics-trace`).
4. **Backend split.** Move the ~97 `acacia-bot` files that touch Bedrock packets behind edition backends
   (movement, interaction, inventory). Java bots path, dig and place. Done 2026-11-10 for goto with digging and
   bridging (`acacia-bot` example `java_goto`). Boats too: `SetPassengers` seats the player
   (`acacia-java-client` `riding.rs`); a driver ticks the boat (`acacia-java-physics` `boat.rs`, `AbstractBoat.tick`
   on the previous tick's keys), then sends `PaddleBoat`, `MovePlayerRot` and `MoveVehicle`; holding shift
   dismounts. Example `java_boat` (paddling over grass, a drop into a pool, turns, reversing): 0 alerts, 0
   `MoveVehicle` corrections; 0.042 for the 0.04 paddle push gives 63 Simulation alerts. Boat block effects run
   twice a tick as vanilla's do (`boat_inside.rs`, sharing `inside.rs` with the player): a cobweb row stops the
   boat with 0 alerts, against 1 alert and an eject without them. Bubble-column launches come as entity events
   71/72, which Grim doesn't model (1 Simulation alert, 0 corrections, as a vanilla client would get). Vehicles
   collide with solid and pushable entities, the player with solid ones (sizes and kinds from
   `acacia-java-proto/tools/EntityDump.java`); a boat stops at a NoAI pig, 0 alerts. The driven boat is never
   pushed (it carries a rider). A teleport of it interpolates over three ticks (`boat_lerp.rs`); Grim flags a
   tp of a ridden boat whether it snaps or interpolates. Boarding's teleport isn't a setback. Not modelled:
   the scale attribute and per-type shapes (slimes, pufferfish, shulker peeks), other vehicles; babies (the dump's
   `BABY_DIMENSIONS` or age scale), player poses and a still happy ghast are sized. Every teleport goes through
   `acacia-java-physics` `Placement::teleported`, `ROTATE_DELTA` included. Elytra flight too: a jump in the air
   with a usable elytra worn starts the glide (`START_FALL_FLYING`), and each firework rocket attached to the
   player (entity data 9) boosts the physics after the player's tick (example `java_elytra`: two boosts, ~930
   blocks, 0 alerts). Dialogs answer as forms (see "Forms" below). Item use in the air came 2026-11-10: eating (`consume`, auto-eat) with the using-item slowdown, health and food from `SetHealth`; example
   `java_eat`, 0 alerts while walking. Threat perception too: the Java client's entities mirror into
   `state.entities` each tick (Bedrock kind names; proto `entity_types.rs` from `tools/gen_entity_types.mjs`), and
   Java goto perceives hazards from them (example `java_avoid` swerves 4 blocks round a zombie, 0 alerts).
   Entity clicks and striking back too: `ATTACK` + swing / `INTERACT`. Entities are held where a vanilla client sees
   them (`acacia-java-client` `interpolation.rs`, 26.3's stepped interpolation); the newest packet position gave
   Grim Reach flags. A mob's aggressive flag stands in for Bedrock's `TargetEid`, since Java zombies chase at
   stroll speed. Sent yaw stays continuous (AimModulo360). A 1-wide corridor with a chasing zombie: killed in
   every run, 0 alerts.
5. **Inventory and crafting.** Java container clicks (state ids, carried item) behind the same API as
   Bedrock's ItemStackRequest. Own-inventory moves done 2026-11-10: `Bot`'s item moves become window-0 clicks
   predicted as `doClick` (`acacia-java-client` `clicks.rs`, bot `items/java_clicks.rs`); live, goto pulls a tool
   from the main inventory with 0 Grim alerts. Containers too (`OpenScreen` → `state.containers.open`; example
   `java_chest` opens, fills, empties and reopens a chest with 0 alerts). Crafting too: Java servers send no
   usable recipes, so `Recipes::java()` serves 26.3's vanilla ones (acacia-registry `recipe`), and every craft lays
   the grid out by hand and shift-clicks the result unpredicted (`workstation/java_craft.rs`; example `java_craft`
   makes planks, sticks and a pickaxe at a table with 0 alerts). Furnaces need nothing Java-specific (example `java_smelt`); their
   timers mirror into `containers.data` under Bedrock's numbering. Enchantments and plain custom names too: they
   are hashed with `HashOps` (map and string encodings) against the enchantment registry's names from
   configuration, and mirrored into Bedrock's `ench` list (so Efficiency counts for digging). Live: 0 hash misses
   moving an enchanted, named shovel, against 1 with the name hash broken on purpose (`Inventory::resent` counts
   misses). Stations too (`workstation/java_station.rs`): inputs go in by clicks, then the station's own packet
   (anvil rename; an enchanting option as a menu button), and the result the server fills in is shift-clicked out
   unpredicted. Bedrock's plans are skipped, since Java's anvil and XP rules are the server's. The stonecutter's
   button is the recipe's index among those accepting the input, in `UpdateRecipes` order (`java-proto/recipes.rs`).
   Example `java_stations`: anvil rename, grindstone, netherite upgrade, enchanting and 4 stone slabs, 0 alerts.
   Trading (`workstation/java_trade.rs`) picks the offer with `SelectTrade`, which has the server fill the payment;
   one shift-click on the result takes every trade it covers, else each trade is a click and a put-away.
   `MerchantOffers` mirrors into `stations.trade`. Example `java_trade`: 1 trade by clicks and 3 by one shift-click,
   0 alerts. The loom's button is the pattern's index in the `no_item_required` tag (or the pattern item's own),
   cartography crafts are a click on the result each (Java renames maps at the anvil), and the beacon is `SetBeacon`
   with Java's `mob_effect` ids. Stacks keep `map_id`, or every click on a filled map misses its hash and the
   server's stale cursor resend lands after the next predicted click. `java_stations` runs all nine stations with 0
   alerts. A craft matching an unlocked recipe (`RecipeBookAdd`, matched by result and cell items) is laid out by
   `PlaceRecipe`, one per craft up to 8, as a player clicking the book; others are placed by hand. `java_craft`
   now crafts planks, sticks and the pickaxe from the book, 0 alerts.
6. **JS package.** Expose the edition in `@acacia-rs` bindings. Done 2026-11-10 on branch `java/js` (java/edition
   merged with js/cancel): `createBot({ edition: 'java' })`, port 25565 by default, offline or Microsoft (the host's
   sign-in thread hands the account over; untested live, since it needs an online-mode server). Snapshots read the
   Java body through `Backend::motion`; chat goes through the inbox, since only Bedrock has a lock-free sender.
   Dialogs are `FormBody::Dialog` / `FormAnswer::Dialog`, and Bedrock-only actions reject `NOT_POSSIBLE`. Live
   on the Paper gate from Node: join, chat, goto, walking and crafting planks from the book, 0 alerts.

## Forms

Java dialogs (`ShowDialog`, inline or a registry entry) are `Bot` forms. Confirmation → `Modal`; no inputs →
`Simple` (buttons in order); inputs → `FormKind::Dialog`, answered with `FormReply::Dialog { button, values }`.
Inputs map to Bedrock elements: text → Input, boolean → Toggle, single_option → Dropdown, number_range → Slider
(snapped to its step). A button sends a command (unsigned `ChatCommand`, templates filled as vanilla does) or a
`CustomClickAction`; `Close` runs the cancel action (notice → its button, confirmation → no, else `exit_action`)
when escape may close it. Registry dialogs come from configuration's `minecraft:dialog` data; the built-in ones
(sent without data, since the client claims the vanilla packs) show as a form with no buttons titled by index.
Example `java_dialogs` runs the four dialogs in `tools/gate-datapack`, 0 alerts.

## Test servers

- Vanilla 26.3 server jar for codec round-trips and behaviour.
- Real 26.3 client (Fabric, Xvfb, `acacia-java-bridge/tools/client-shot.sh` pattern) for physics traces.
- Paper + Grim as an anticheat gate: `tools/grim-gate.sh` drives `acacia-java-client`'s `java_gate` example over key
  scripts; judge by server corrections and Grim's verbose flags. `ACACIA_GATE_CHEAT=1` is the control that must fail.
  Other examples run through it behind a wrapper that keeps `<host> <port>` and drops the gate's other arguments
  (e.g. `java_goto` with `ACACIA_DIG`/`ACACIA_BRIDGE`).
