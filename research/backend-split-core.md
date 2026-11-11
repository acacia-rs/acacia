# Backend split: Bot core, ingest loop, state, world

Recon for build-order step 4 of docs/java-edition.md. Paths are relative to `crates/acacia-bot/src` unless
prefixed. Read-only survey on branch java/edition, 2026-11-10.

## 1. The `Bot` struct and its loop

`Bot` is defined at bot.rs:91-132. Its fields, by role:

| Field | Type | Edition |
|---|---|---|
| `client` | `acacia_client::Client` (bot.rs:92) | Bedrock |
| `state` | `GameState` (state/mod.rs:127) | mixed, see section 2 |
| `subscribe` | `PacketFilter` (Bedrock packet ids) | Bedrock |
| `world` | `Option<WorldTracker>` (world/mod.rs:53) | Bedrock |
| `movement` | `Option<Movement>` (Bedrock physics + `Controls`) | Bedrock, but `Controls` is neutral intent |
| `idle`, `subchunks` | `(Idle, SpawnSequence)`, `SubChunkRequester` | Bedrock (loading screen, sub-chunk requests) |
| `sync` | `PredictionSync` | Bedrock |
| `ticker` | `cadence::Ticker` | neutral |
| `auto_respawn`, `respawn_in` | | neutral intent, Bedrock handshake (respawn.rs:1-3) |
| `recorder` | `trace::Recorder` | Bedrock packet traces |
| `events`, `pending` | `EventSource`, `VecDeque<BotEvent>` | mostly neutral, `BotEvent::Packet(RawPacket)` is Bedrock |
| `human`, `survival`, `governor`, `act_timing`, `cancel`, `reflexes` | | neutral logic |
| `queued_flags` | `Vec<InputData>` | Bedrock (PlayerAuthInput flags) |
| `ride`, `bed`, `mining`, `travel`, `closing` | | neutral intent, Bedrock wire |
| `request_ids`, `in_flight` | `RequestIds`, `Pipeline<StackUndo>` | Bedrock (ItemStackRequest ids) |
| `spawned`, `strict`, `closed` | | `closed: Option<DisconnectReason>` is acacia-client's type |
| `lag` | `Option<LagLine>` holding `RawPacket`s | Bedrock |

Ownership and threading:

- `acacia_client::Client` is a handle to a spawned connection task: commands go over an unbounded mpsc,
  events come from an `EventReceiver` (acacia-client/src/handle.rs:16-23). `send` is **sync** and only queues
  (handle.rs:59); `recv` is async and cancel-safe; `try_recv` is non-blocking.
- No locks and no background work in the bot itself (lib.rs:4-5). The only shared state is `SharedWorlds`
  (`Arc<Mutex<HashMap<(server, dim), Weak<World>>>>`, world/mod.rs:31-51), so bots on one server store each
  chunk once.
- The loop is caller-driven. `Bot::next` (bot.rs:261) and every action's waits (`wait.rs:37` `progress`,
  `wait_until` wait.rs:72, `next_tick` wait.rs:92) call `Bot::step` (bot.rs:278). `step` flushes the world's
  sub-chunk requests, then either `try_recv`s a queued event or `tokio::select!`s on `client.recv()`, the lag
  line's timer, and `ticker.wait()` (bot.rs:298-306).
- A packet goes through `Bot::apply` (bot.rs:330-361): `state.apply`, then `world.apply`, `movement.apply`,
  the container close, `sync`, `subchunks`, travel, auto-respawn, reflexes, fetch, and `events.on_packet`.
  Every consumer receives the same `&RawPacket`.
- A tick goes through `Bot::on_tick` (bot/tick.rs:16-43): governor, `entities.advance`, respawn, reflexes,
  survival, then either `idle_tick` (spawn sequence plus a stand-still `PlayerAuthInput`) or `physics_tick`
  (bot/tick.rs:113-149: equipment, abilities and solid entities go into `Movement`, then
  `movement.tick(&PhysicsWorld{view, registry})` yields a `PlayerAuthInput`, which is sent).
- Actions wait on Bedrock packets. `wait_until`/`next_tick` take `FnMut(&Bot, &RawPacket)` closures. There
  are 43 call sites in 26 files.

## 2. How state consumes packets

There is **one central dispatch**. `GameState::apply` (state/mod.rs:170-243) routes by id: each tracker
declares `const PACKETS: &[u32]`, and `apply` runs `if X::PACKETS.contains(&id) { self.x.apply(packet) }`.
The subscription filter is the union of those lists (`Trackers::packet_ids`, state/mod.rs:85-116, plus
bot.rs:139-146). `BlockEntityData` gets special handling: it is decoded once and fed to two trackers
(state/mod.rs:173-180). Outside `GameState`, the world, movement, sync, sleep, dimension and subchunks
modules also declare `PACKETS` and are called from `Bot::apply`.

| Tracker (file) | Bedrock packets read | Meaning |
|---|---|---|
| player (state/player.rs:87) | StartGame, MovePlayer, CorrectPlayerMovePrediction, UpdateAttributes, SetHealth, Set/UpdatePlayerGameType, ChangeDimension, SetSpawnPosition, Respawn, MobEffect, UpdateAbilities, + SetEntityData | **neutral** (position, health, food, xp, game mode, abilities, effects). Fields use proto types (`Vec3f`, `GameMode`, `BlockCoordinates`, player.rs:27-34), and `runtime_entity_id: u64` is Bedrock-shaped |
| entities (state/entities/apply.rs:13, lean list :32) | AddPlayer, AddEntity, AddItemEntity, UpdateAttributes, RemoveEntity, MoveEntity(Delta), MovePlayer, SetEntityMotion, SetEntityData, MobEffect, MobEquipment, MobArmorEquipment, ChangeDimension, SyncEntityProperty | **neutral** in meaning. `kind: String` ids mostly match Java's. `metadata` is keyed by Bedrock dictionary keys and `properties` are Bedrock-only (entities.rs:27-56) |
| inventory (state/inventory.rs:55) | InventoryContent, InventorySlot, MobEquipment, PlayerHotbar, ItemStackResponse, InventoryTransaction | contents and selected slot are **neutral**. `ItemStack.network_id`, `stack_network_id`, `block_runtime_id` (inventory/stack.rs:8-21) are **Bedrock** |
| containers (state/containers.rs:61) | ContainerOpen/Close/SetData, InventoryContent/Slot, ItemStackResponse, UpdateTrade, BlockEntityData | open window + slots **neutral**. `ContainerSlotType` addressing is **Bedrock** |
| items (state/items.rs:21) | ItemRegistryPacket | **Bedrock** (network id ↔ name table) |
| recipes (state/recipes.rs:149) | CraftingData | **Bedrock** shape. Java has its own recipe book |
| creative (state/creative.rs:35) | CreativeContent | **Bedrock** |
| stations (state/stations.rs:80) | PlayerEnchantOptions, UpdateTrade, ContainerClose | enchant/trade offers are neutral in meaning, Bedrock-shaped |
| scoreboard (state/scoreboard.rs:81) | SetDisplayObjective, RemoveObjective, SetScore, SetScoreboardIdentity | **neutral** |
| player_list (state/player_list.rs:47) | PlayerList | **neutral** (Java: PlayerInfoUpdate) |
| environment (state/environment.rs:42) | StartGame, SetTime, SyncWorldClocks, LevelEvent, BossEvent, ChangeDimension | **neutral** (time, weather, boss bars) |
| riding (state/riding.rs:57) | AddEntity, AddPlayer, RemoveEntity, SetEntityLink, SetEntityData, ChangeDimension | **neutral** |
| fishing (state/fishing.rs:45) | AddEntity, RemoveEntity, EntityEvent, SetEntityMotion, ChangeDimension, AddItemEntity, TakeItemEntity | **neutral** |
| hurts (state/hurts.rs:18) | EntityEvent | **neutral** |
| signs (state/signs.rs:18) | OpenSign, ChangeDimension | **neutral** |
| block_entities (state/block_entities.rs:43) | BlockEntityData + chunk packets | neutral in meaning, but Bedrock NBT shapes |
| skins (state/skins.rs:66) | PlayerList, PlayerSkin | **Bedrock** skin format |
| maps (state/maps.rs:37) | ClientboundMapItemData | **neutral** (map colours) |
| forms (forms/mod.rs:107) | ModalFormRequest, ClientboundCloseForm | **Bedrock-only** |
| effects/abilities/metadata/kept/queries | helpers with no packets of their own | effect ids are Bedrock numbers, metadata is Bedrock |

Other packet consumers outside `GameState`:

- WorldTracker (world/mod.rs:69). Bedrock.
- Movement (movement/packets.rs:25: MovePlayer, CorrectPlayerMovePrediction, SetEntityMotion, Respawn,
  MobEffect, UpdateAttributes, MovementEffect, ChangeDimension). Bedrock.
- PredictionSync (prediction_sync.rs:52). Bedrock.
- sleep (sleep.rs:20). Text and Animate.
- dimension (bot/dimension.rs:11). ChangeDimension, PlayerAction, ShowCredits.
- SubChunkRequester (subchunks.rs:37).
- EventSource (`events/`), which produces Chat, Form, Health, Died, PlayerJoined, Window*, Title and others
  (events/mod.rs:64-94). Nearly all of them are neutral. `Form` and `Packet(RawPacket)` are Bedrock.

## 3. The world

- **Storage.** `acacia_world::World` holds chunks as `Arc<SharedChunk>` with an RwLock. It is shared across
  bots through `SharedWorlds`. Each bot holds a `ChunkView` (acacia-world/src/view.rs:10), which keeps its
  chunks alive and supports `retain_within` for idle bots.
- **Ids.** Blocks are **Bedrock runtime ids (u32), two layers**: layer 0 is the block, layer 1 is
  liquid/waterlogging. `BlockAccess::{block, liquid}` (acacia-world/src/access.rs:2-7) reads them, and
  unloaded positions read as air. The wire id (runtime or FNV hash) is converted at insert (world.rs:66,
  89-103).
- **Per-state data.** `acacia_world::BlockRegistry` (registry/mod.rs:25) is a
  `Vec<acacia_world::BlockState>`. Each entry carries a Bedrock `name`/`properties` string, collision
  `boxes`, `selection`, `friction`, `flags`, `liquid_depth`, `mining`, light, and
  `neutral: Option<acacia_registry::BlockState>` + `NeutralFlags` (registry/state.rs:57-85). The registry is
  built from StartGame, with custom blocks merged in (world/mod.rs:121-125).
- **Queries callers use.**
  - `WorldTracker::{view(), registry(), knows_block()}` (world/mod.rs:101-117).
  - `Bot::block_at(pos) -> (runtime id, &acacia_world::BlockState)` (interact/mod.rs:112-117). Public
    wrappers are `block_name` and `block_state` (interact/direct.rs:106-113).
  - `has_support_below`, `is_chunk_loaded` (world/mod.rs).
  - Physics: `PhysicsWorld = RegistryWorld<ChunkView>` (world/adapter.rs).
  - Pathfinding: `trait Blocks { fn ids(pos) -> Option<(u32,u32)> }` (pathfind/terrain.rs:26-36) plus
    `&BlockRegistry` lookups for boxes and flags. Some rules read `prop::` via `.neutral` (44 sites in 26
    files).
  - Usage counts: `.view()` 24 sites, `.registry()` 17 sites. 35 files name `ChunkView`, `BlockRegistry`
    or `BlockAccess`, many of them tests.
- **acacia-java-client's World** (acacia-java-client/src/world.rs:29-34) is a plain
  `HashMap<(cx,cz), Vec<Section>>` of **Java state ids**, owned by `Client`. Its API is
  `block([x,y,z]) -> Option<u32>` (`None` when unloaded) and `min_y()`. It also holds a minimal `entities`
  map. There is no sharing, no layer 1 and no block entities. Java physics reads it through
  `acacia_java_physics::Blocks` (collide.rs:14).
- **Key fact.** Neutral `acacia_registry::BlockState` ids below `JAVA_STATE_COUNT` *are* Java 26.3 palette
  ids (acacia-registry/src/state.rs:8-23). A Java world's native id is already its neutral state, and Java
  collision comes from `java_collision()` (acacia-registry/src/collision.rs:42).
- **Difficulty: moderate, and mostly mechanical.** The cheapest path keeps the query API and swaps what
  sits under it:
  1. Build an `acacia_world::BlockRegistry` keyed by Java state id. Each entry gets: name from the neutral
     block, boxes from `java_collision`, flags/friction/mining from the neutral data, and
     `neutral = Some(id)`. `BlockRegistry` fields are private (registry/mod.rs:25-34), so this needs a
     constructor in acacia-world.
  2. Implement `BlockAccess` and the pathfind `Blocks` trait for a Java world. `liquid()` is synthesized
     from `waterlogged`/fluid state, so pathfinding's two-layer model holds.
  3. Abstract `WorldTracker` behind a small enum (see section 5) so that `view()`/`registry()` still hand
     out `&dyn BlockAccess` + `&BlockRegistry`.

  Harder parts:
  - `selection` boxes and `SelectionOffset` are BDS-derived. Java needs its own outline shapes for
    sight/raycast (`interact/sight.rs`, `build/reach.rs`).
  - Bedrock `name` strings are used by `block_name` and by string-matching callers. Step 1 of the doc
    (neutral enums) has to land first.
  - The JS host reads `bot.world()?.view()?.world().clone()` (an `Arc<acacia_world::World>`) for snapshot
    readers. A Java world would need the same Arc/shared shape, or that JS path stays Bedrock-only.

## 4. Public Bot API, grouped (uses counted in /srv/agents/work/acacia/acacia-js)

| Group | Methods | Guess |
|---|---|---|
| Lifecycle/loop | `connect`, `next`, `disconnect`, `disconnect_reason`, `wait_ticks` (215 JS uses), `cancel`, `cancel_handle`, `abandon_action`, `governor` | neutral, except `connect(ClientBuilder, ..)` and `DisconnectReason`, which are Bedrock types |
| Raw access | `client()` (JS: `.command` 52, `.display_name` 8, `.close` 4, `.chat` 2, `.send` 2), `packet_trace`, `trace_mark` | Bedrock. Wrap command, chat, name and close in neutral Bot methods; `send` stays in `bot.bedrock()` |
| State read | `state()` (JS: `.player` 32, `.entities` 24, `.inventory` 15, `.item_name` 15, `.block_entities` 11, `.containers`, `.stations`, `.forms`, `.skins`...) | neutral meaning, Bedrock-typed fields (section 2) |
| World read | `world()`, `block_name`, `block_state`, `block_position`, `eye_position`, `facing`, `sees_face` | neutral if `world()` is narrowed. It currently returns `&WorldTracker` |
| Movement | `controls()` (26), `movement()` (66, mostly `corrections`/`teleports`/position), `goto`, `goto_with`, `stop_pathing`, `fly_to`, `boat_to`, `look_at` | `controls`, goto and look are neutral. `movement()` exposes the Bedrock `Movement` |
| Digging/placing | `break_block`, `start_mining`/`stop_mining`, `abort_break`, `mining_progress`, `place_block(_at)`, `use_item(_on_block)`, `release_item`, `swing`, `right_click_block` | neutral API, Bedrock wire |
| Items/containers | `select_hotbar`, `move_item`, `swap_items`, `drop_item`, `quick_move`, `take_all`, `click_slot`, `open_container(_at)`, `close_container`, `close_window`, `find_item`, `equip`, `equip_best_tool` | neutral API. `item_stack_request` is Bedrock-only |
| Workstations | `craft`, `smelt`, `enchant`, `anvil`, `trade`, `loom`, `brew`, `smith`, `stonecut`, `grindstone`, `beacon`, `cartography` and their `*_choices` helpers | neutral API. `take_creative`/creative and recipe ids are Bedrock-shaped |
| Survival/other | `attack`, `interact_entity`, `mount`/`dismount`, `sleep`/`wake`, `fish`, `consume`, `equip_elytra`/`start_gliding`/`boost_with_firework`, `write_sign`, `write_book`/`sign_book`, `respawn` | neutral |
| Bedrock-only | `answer_form`/`close_form`/`wait_form`, `item_stack_request`, `boat_corrections`/`boat_state`, `client().send`, `trace_mark`/recording | go behind `bot.bedrock()` |

63 files hold `impl Bot`. 56 `client.send` sites in 20 files emit Bedrock packets directly.

## 5. Recommended seam

**Use an enum backend inside the one `Bot`, with neutral logic left on `Bot`:**

```rust
pub struct Bot {
    backend: Backend,          // edition-specific connection + trackers + wire encoding
    state: GameState,          // neutral fields; edition-only parts move into the backend
    ticker, governor, human, survival, cancel, pending, ...  // unchanged neutral logic
}
enum Backend { Bedrock(Box<BedrockSide>), Java(Box<JavaSide>) }
```

- `BedrockSide` takes the Bedrock-only fields from section 1 (client, subscribe, WorldTracker, Movement,
  idle/subchunks, sync, queued_flags, request_ids, in_flight, recorder, lag).
- `bot.bedrock()`/`bot.java()` return `Option<&BedrockSide>` by matching the enum.
- Neutral actions call a small set of backend verbs (`send_input`, `click_block`, `use_item`,
  `container_click`, `world()`), and those verbs `match` on the backend.

Trade-offs:

1. **Enum vs trait object: async in the hot loop.**
   - `step` runs `tokio::select!` on `client.recv()` on every packet (bot.rs:298).
   - An enum keeps static dispatch and lets each arm select on its own concrete future.
   - `dyn Backend` would need boxed futures per recv (async_trait), or a `poll_recv` design.
   - `bot.bedrock()` with a trait object needs `Any` downcasts. The enum gives it for free, as the doc's API
     asks.
   - Cost of the enum: a closed set, which suits two editions pinned to one version each.
2. **Enum vs generic `Bot<E>`.**
   - The doc settled on one concrete `Bot` for swarm and JS.
   - The JS host already calls 60+ distinct Bot methods (counts above).
   - Generics would push a type parameter through swarm, JS, and 63 `impl Bot` files.
   - Generic edition code *inside* a backend (`Terrain<B: Blocks>`, pathfind/terrain.rs:109) is still fine
     and already the pattern.
3. **Where the packet type leaks.** This is the real cost, whatever the seam:
   - `Step::Packet(RawPacket)` (bot.rs:82-88).
   - `BotEvent::Packet(RawPacket)` (events/mod.rs:66).
   - The 43 `wait_until`/`next_tick` closures over `&RawPacket` (wait.rs:72-102).
   - `LagLine` stores `RawPacket`s.

   Options:
   - (a) An `enum WirePacket { Bedrock(RawPacket), Java(java::Packet) }`. This is mechanical, but every
     Bedrock waiter must handle the Java arm.
   - (b) Turn waiters into state predicates (`FnMut(&Bot) -> Option<T>`, checked after each applied packet).
     Most waiters already read state through `&Bot`, so they become neutral.

   Prefer (b) for neutral actions and keep (a) for Bedrock-only waits.
4. **The Java client needs a driver before it can slot in.**
   - Bedrock `send` is sync and queued (handle.rs:59).
   - Java `Client::next`/`step`/`send` are async `&mut self` and not cancel-safe (join.rs:88, 114-117;
     movement.rs:125; frame.rs:98).
   - The Java `Client` also owns its World and Movement and applies packets itself (join.rs:109-110).
   - Before the split, give acacia-java-client a spawned connection task with a sync `send` and a cancel-safe
     `recv` handle, mirroring acacia-client. Then let `JavaSide` own World/Movement, so that `Bot::step`
     keeps one shape and the 56 sync send sites map to backend verbs.
5. **State: keep one `GameState` and split ingest, not storage.**
   - The dispatch is already central (state/mod.rs:170). Add a sibling `GameState::apply_java` that routes to
     `tracker.apply_java`.
   - Then retype edition-specific fields: `Vec3f` → `[f32;3]`/`[f64;3]`, entity id `u64` → a neutral id,
     `ItemStack.network_id` → neutral `Item` + an edition extra for `stack_network_id`.
   - Bedrock-only trackers (forms, creative, skins, item registry) stay in `GameState` as always-empty on
     Java, or move into `BedrockSide`. Moving them is cleaner, but it breaks the JS `state().forms` path.
6. **World: put a small enum behind `bot.world()`.**
   - Use `Terrain { Bedrock(WorldTracker), Java(JavaWorld) }`, both exposing `BlockAccess` + a
     `&BlockRegistry` keyed by native id (section 3).
   - Pathfinding and build keep working on native ids + registry records. Their neutral prop reads go
     through `.neutral`, which is the identity for Java.
   - Physics stays edition-specific (`PhysicsWorld` vs `acacia_java_physics::Blocks`), behind the physics
     trait the doc calls for.
