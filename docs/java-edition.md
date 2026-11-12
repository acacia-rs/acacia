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
   pushed (it carries a rider). A teleport of it interpolates over three ticks (`lerp.rs`); Grim flags a
   tp of a ridden boat whether it snaps or interpolates. Boarding's teleport isn't a setback. Mounts the player
   controls (saddled horses, camels, nautili; pigs and striders with their stick in hand; a harnessed happy
   ghast not holding still) are driven too: `acacia-java-physics` `mount.rs` runs `travelRidden` on the player
   body with the mount's size, step, speed and input, a tick before the rider; the client sends the jump bar's
   `START_RIDING_JUMP`, then input, look and `MoveVehicle` (`acacia-java-client` `mount_drive.rs`). Example
   `java_mount` (ahead, a turn, a charged jump or dash, reversing): pig, horse, camel, strider on lava and happy
   ghast all 0 alerts, 0 corrections; a strider's rider stepping off onto lava gets 1 (the player's own lava
   moves, gone with `ACACIA_STAY`). Other vehicles seat the player at the type's passenger attachment. A
   minecart has no controlling passenger, so the server moves it: the rider sends only `PlayerInput` (forward
   nudges a still cart, `getLastClientMoveIntent`) and `MovePlayerRot`, never `MoveVehicle`; drill `minecart`
   (powered run, a hill, a halt, dismount) 0 alerts, 0 setbacks. The dismount's exit teleport isn't a setback. Other
   entities are sized as `getDimensions` and `makeBoundingBox` do (`acacia-java-client` `entity_size.rs`): pose,
   baby (the dump's `BABY_DIMENSIONS`, or camels' `BABY_STANDING_DIMENSIONS`, or age scale), the scale attribute
   from `UpdateAttributes` with each type's `sanitizeScale`, and the shapes that change a box: slime/magma/sulfur
   cube size, pufferfish puff, salmon variant, phantom size, goat long jump, sitting camel, digging warden and
   sniffer, and a shulker's lid animated client-side (peek eased 0.05 a tick, the box grown out of its attach
   face). Pushes, vehicle collisions, riptide hits, mount bodies and seats, and the bot's attack reach
   (`interact/entity.rs`) all use that box. Lids push (`lids.rs`, `lid_shape.rs`): after the player's tick, as
   entities and then block entities tick, a shulker's peek and an opening shulker box (its `BlockEvent`) move the
   player `Entity.move(SHULKER)`-style (`move_pushed`: no edge back-off) when it's in the swept box; an open box
   lid is also solid. While riding, the lid moves the vehicle in its box (driven: as simulated, then reported;
   otherwise where it shows, `acacia-java-physics` `shove`) and the pushed passenger until `positionRider` reseats
   it. A shulker box can't open under a boat (`canOpen` counts solid entities). A seat (`seat.rs`, physics
   `mount_seat.rs`) sits the player at `getPassengerRidingPosition` for its place among the riders the client
   knows (boats 0.2 fore and 0.6 aft for two, chest boats 0.15; camels' back seat 0.7 aft on its own anchor curve;
   happy ghasts' four), plus a rearing horse's `standAnimO` shift and a strider's walk bob, less its own scaled 0.6
   attachment; a passenger's yaw is clamped to a boat's (`clampRotation`). A camel's front seat follows
   `getBodyAnchorAnimationYOffset` through sitting down and standing up, and a
   driven camel's body takes its standing size on `standUp`; `/data merge` of a negative `LastPoseTick` sits a
   ridden camel down live (`readAdditionalSaveData`). A camel's
   rider sprints by `aiStep`'s rules with `vehicleCanSprint` for food, and the flag is sent only beside
   `MoveVehicle`; a camel never rears on a dash, and any synced dash item restarts an idle cooldown. Not modelled:
   the mount's own effects, an `Animal` passenger's seat shift, other `Vec3` seat attachments' double precision
   (the table holds floats; only the happy ghast's are exact). Every teleport goes through
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
(snapped to its step). A button sends a command (`sendUnattendedCommand`, templates filled as vanilla does) or a
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
- Drills, after Bedrock's: `tools/java-drills.sh <java_drill binary> [drill...]` runs the `java_drill` example's
  scenarios on one Paper + Grim server, a fresh connection each. A drill sets itself up by op commands on its own
  strip and passes with 0 Grim alerts, 0 setbacks after setup and its own check (speed, slowness, jump_boost,
  levitation, slow_falling, hunger, powder_snow, nether_lava, push, death, chat, crowd, ten `/attribute` courses
  in `attrs.rs`, riptide, riptide_short, rain_riptide, rain_roofed, rain_relit, minecart, the inside-block drills cobweb,
  berry_bush, cauldron, the size drills crowd_scaled, baby_push, slime_push and camel_dash, riptide_armor_stand,
  sulfur_push, the seat drills camel_sit, scaled_rider, boat_pair, boat_back_seat, camel_back_seat (a `Peer` bot
  drives), horse_rear and camel_sit_down, checked against vanilla's seat geometry since Grim never sees a
  passenger, and the vehicle lid drills boat_shulker_push and horse_lid_push; powder_snow_fall, shulker_push and shulker_box_push run only by name: the server pushes
  the player too and `isEntityCollidingWithAnythingNew` teleports it back when its push leads, 1-3 setbacks a run). Grim tolerates a missing push, so the size drills check the push (or its absence) themselves. `DRILL_TRACE=1` prints the player
  every tick with a UTC clock to match Grim's log; `RUST_LOG=acacia_bot::bot::java=debug` logs each setback.
  Every drill server requires a resource pack, so each join also checks the pack answers. Setup waits out Grim's
  silent setbacks (random negative teleport ids, no alert) after the far setup `/tp`. A real 26.3 client gets them
  after every far tp (3-4 each, `tools/tp-trace.sh` in the bridge repo). Grim resyncs any move made while the
  player's chunk is missing or the ping Grim sent after that chunk is unanswered, and vanilla keeps ticking there.
  acacia's player waits for its chunk, so it gets fewer.
- `tools/java-switch.sh <java_drill binary>`: the `switch` drill behind Velocity (`tools/java-proxy/velocity.toml`,
  jar at `~/java-truth/velocity/velocity.jar`) with two Paper + Grim backends; `/server b` reconfigures the client
  in play, and it must walk on b with 0 alerts on both and 0 setbacks.

## Joining

Login answers cookie requests (none stored yet) and plugin queries ("no payload"), then sends the vanilla brand
and client information. Configuration (`configuration.rs`) answers keep-alives, pings, known packs, cookies
(`StoreCookie` keeps them for the connection), the code of conduct (accepted) and resource packs: every pack is
reported accepted, downloaded and loaded without being fetched, since declining a required one is a kick.
`StartConfiguration` in play (a proxy switching backends) is answered on the link's reader: the writer drops
play packets until `FinishConfiguration`, the client holds its tick, and the next `Login` places a new player.
Not done: `Transfer` (reconnecting elsewhere with the cookies).

## Status, chat and loading

- Health, food and mob effects come from `SetHealth` / `UpdateMobEffect` / `RemoveMobEffect` into the physics
  (`acacia-java-client` `status.rs`): sprinting needs food > 6, FAST_LAVA is the Nether's, leather boots walk on
  powder snow. Remote mobs push the player after its tick, as `pushEntities` does client-side (`push.rs`), filtered
  by team collision rules (`teams.rs`); the tab list is `players.rs`.
- Attributes: `UpdateAttributes` replaces each attribute's base and modifiers (`acacia-java-physics`
  `attributes.rs`), summed in fastutil's hash order (`open_map.rs`) so float results match. Sprint is the
  attribute modifier, as vanilla's `setSprinting`, and the server's shared flags overwrite it.
- Riptide: a trident with the enchantment held 10+ ticks in water or rain launches on release (`riptide.rs` in client
  and physics), then spins until it hits a living entity (the generated table's `living` column) or a wall. Rain is `Level.isRainingAt` (`rain.rs`): the
  rain level from `GameEvent`, sky light 15 from the server's light data (`sky_light.rs`), relit by the client
  after its tick for block changes as `SkyLightEngine` does its column sources (`relight.rs`, whose doc has the
  boundary; drill `rain_relit`), the MOTION_BLOCKING heightmap scanned from the blocks, and the biome's precipitation through
  `BiomeManager`'s zoom (`biomes.rs`; climates from acacia-registry `climate`, or the server's biome data).
- Block effects: `checkInsideBlocks` tests each block's inside shape (`inside_shape.rs`: cauldron contents,
  tripwire, end portal, a falling body's 0.9 of powder snow) and fluid box, and only touched blocks act or count as
  visited. Replay trace `snowfall` (a real client's 4-block drop onto powder snow) lands on the 0.9 top. Grim
  sticks any body in the snow's cell (`StuckSpeed`), resets its fall and expects it to fall through: GroundSpoof,
  Simulation and a setback loop, as a vanilla client gets, so drill `powder_snow_fall` is opt-in.
- Death: health 0 is `BotEvent::Died` (`PlayerCombatKill`'s text a chat event); `ClientCommand` 0 respawns (auto-respawn after the death screen).
  `Respawn` rebuilds the player, keeping what its `dataToKeep` byte says, and a new level only on a dimension change.
- Chat: system, player (signed), disguised and action bar messages decode to `BotEvent::Chat` (`messages.rs`), with
  the chat type's name from configuration's `minecraft:chat_type`.
- Secure chat (`chat.rs`): on an online-mode server the profile key's session signs chat and each command's
  message arguments, found by brigadier's parse of the server's tree (`brigadier.rs`, argument readers in
  `arg_types.rs`; the boundary is in `arg_types.rs`). Without a session the same packets go unsigned, with
  last-seen data. Dialog commands go as `sendUnattendedCommand`. The key is fetched again past its
  `refreshed_after`, at most hourly (`chat_key.rs`). Live check: `tools/java-online.sh … chat`.
- Loading follows `LevelLoadTracker` (`loading.rs`): `GameEvent` 13, then the player's chunk, then 10 ticks before
  `PlayerLoaded`; the player neither ticks nor moves until then. Sending it on the first position made a join in
  mid-air fall early, and Grim flagged Simulation.
