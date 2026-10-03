# 03 — Azalea architecture (reference for a Bedrock high-level layer)

Source: a local checkout of [azalea](https://github.com/azalea-rs/azalea) (workspace `0.16.0+mc26.2`, read 2026-09-30),
GitHub issues, mc-agents/bot-azalea measurements. File refs are relative to that checkout.

## 1. Crate layering (dependency direction: bottom → top)

| Crate | Role | Depends on (azalea-*) |
|---|---|---|
| azalea-buf | `AzBuf` read/write derive, varints | — |
| azalea-registry | Generated registries (BlockKind, ItemKind, EntityKind...) | buf |
| azalea-block | Generated `BlockState` (u16 id) + typed block property structs | buf, registry |
| azalea-core | Positions (BlockPos/ChunkPos/Vec3/Aabb), `GameTick` schedule label, registry holder | buf, registry, chat |
| azalea-inventory | Generated `Menu` enum per container type, slots, item data components, click operations | core, registry, chat |
| azalea-world | Chunk/section/palette storage, `World`, `PartialWorld`, `Worlds` container, heightmaps, find_blocks | block, core, registry (+bevy_ecs for `Entity` ids) |
| azalea-entity | Entity components (Position, Physics, LookDirection, metadata bundles), indexes, EntityPlugin | world, inventory, block, core |
| azalea-physics | Vanilla movement port, generated collision shapes (`collision/blocks.rs`, 15.6k lines), raycast `clip` | entity, world, block, inventory |
| azalea-protocol | Packets (generated + hand-fixed), framing, compression, AES; **also depends on world/entity/inventory** for packet field types | most of the above |
| azalea-client | The bot: bevy `App`, ~27 plugins (packet handling, movement, interact, mining, attack, inventory, chat, chunks...) | everything above |
| azalea | Public façade: `Client`/`Swarm`, `ClientBuilder`/`SwarmBuilder`, events + handler fn, **pathfinder**, container helpers, auto_tool/auto_respawn/auto_reconnect | client + all |

Note: protocol depending on world/entity is a layering smell (packet types reuse domain types). Our crate
already keeps protocol pure — keep it that way; do conversions in the state layer.

## 2. ECS architecture

- **One bevy_ecs `World` for all bots in the process**, behind a single `Arc<parking_lot::RwLock<World>>`
  (`azalea-client/src/client.rs::start_ecs_runner`). Each bot is an ECS entity; every server entity it sees is
  also an ECS entity (shared across bots, see §3).
- **Loop** (`run_schedule_loop`): a `spawn_local` task in a Tokio `LocalSet` runs the `Update` schedule at ≤60 Hz
  and the `GameTick` schedule every 50 ms (catch-up capped at 10 ticks, then skips). Everything runs under
  `ecs.write()`.
- **Packet path**: `ConnectionPlugin` runs `read_packets` in `PreUpdate`: for every bot, `try_read()` drains the
  socket **synchronously on the ECS thread** — AES decrypt, zlib decompress and packet deserialize all happen
  there (`connection.rs:46`, `azalea-protocol/src/read.rs:295`). Handlers (`packet/game/mod.rs`, 1.7k lines)
  mutate components directly via `as_system` and emit bevy Messages (`MessageWriter<ChatReceivedEvent>`) or
  Observer triggers (`commands.trigger(KnockbackEvent)`). Writes go through an unbounded mpsc to a per-bot
  writer task.
- **Plugins** (`azalea-client/src/plugins/mod.rs::DefaultPlugins`): Packet, Entity, Physics, Inventory, Chat,
  Disconnect, Movement, Interact, Respawn, Mining, Attack, Chunks, BlockUpdate, TickEnd, Brand, ClientInformation,
  TickCounter, Pong, Connection, Login, Join, Cookies, (ChatSigning). The `azalea` crate adds Pathfinder,
  AutoRespawn, AutoReconnect, AcceptResourcePacks, container, bot (look/jump helpers), events.
  Each plugin = components + message/observer types + systems ordered with `SystemSet`s
  (e.g. `PhysicsSystems` chain in `GameTick`: fluid state → old pos → swimming → `ai_step` → `travel` →
  block effects, `.after(update_in_loaded_chunk)`).
- **Command pattern**: user calls are thin: `Client::attack(e)` = `ecs.write().write_message(AttackEvent{..})`;
  a system in the next Update/GameTick consumes it, checks cooldown (`AttackStrengthScale`), sends packets.
  Same for `StartUseItemEvent` → inserts `StartUseItemQueued` component consumed next tick; `block_interact`,
  `start_mining`, `set_selected_hotbar_slot`, `ContainerClickEvent`, `look_at` (sets `LookDirection`),
  `walk/sprint/jump` (set `ClientMovementState`). Async helpers poll state: `wait_ticks`, `mine().await`,
  `open_container_at().await`, `goto(goal).await` (trait `PathfinderClientExt`).
- **User code**: `ClientBuilder/SwarmBuilder::new().set_handler(handle).start(addr)` where
  `async fn handle(bot: Client, event: Event, state: State)`. `Client = { entity, ecs: Arc<RwLock<World>> }`
  (cheap clone). `State` is a per-bot `Component + Clone` (users wrap in `Arc<Mutex>`). **Every event spawns a
  new `spawn_local` task** running the handler (`swarm/builder.rs:592`), including `Event::Tick` 20×/s/bot.
- Requires `LocalSet`: whole swarm (ECS + all handlers) is single-threaded by design, so ticks never run in
  the middle of user code. bevy task pools (IoTaskPool/AsyncComputeTaskPool) default to a worker per core.

## 3. World storage and sharing

- `Worlds` resource: `FxHashMap<WorldName, Weak<RwLock<World>>>` (`azalea-world/src/container.rs`). Bots in the
  same dimension name get the same `Arc<RwLock<World>>`.
- `World.chunks: ChunkStorage` = `IntMap<ChunkPos, Weak<RwLock<Chunk>>>` (`chunk/storage.rs`). Chunks are
  **only weakly referenced by the world**; each bot's `PartialWorld.chunks` (`chunk/partial.rs`) holds strong
  `Arc`s in a fixed `(2r+1)²` ring indexed by view center. Chunk freed when the last bot's view drops it.
- **Decode skip**: when a chunk packet arrives and the shared world already has it (and this bot doesn't),
  the bot just takes the shared `Arc` without parsing (`plugins/chunks.rs:79`). Server block updates are
  applied once to the shared chunk.
- Entities: one ECS entity per server entity per world, `LoadedBy(HashSet<Entity>)` lists which bots see it;
  despawned when empty. `EntityIdIndex` is **per bot** (servers may assign different ids per player);
  `EntityUuidIndex` global. Relative-move double-application is prevented with an `updates_received` counter
  per bot vs shared (`packet/relative_updates.rs` header comment).
- Chunk format: paletted containers (`palette/container.rs`, `bit_storage.rs`); the pathfinder keeps its own
  `CachedWorld` of `PalettedContainer` clones per search.

## 4. Physics

- Straight port of vanilla `LivingEntity.aiStep/travel/move` (`azalea-physics/src/lib.rs`, `travel.rs`,
  `collision/mod.rs`): gravity 0.08, drag 0.98, block friction × 0.91, jump power with block jump factor
  (honey), jump boost, sprint-jump boost, climbing, fluids (water/lava push, swimming partial), elytra
  (`travel_fall_flying`), step-up via collide-with-step, sneak-edge. Voxel-shape merging (`mergers.rs`,
  `discrete_voxel_shape.rs`) replicates Mojang's `Shapes.collide` per axis (Y, then larger of X/Z).
- **Collision shapes are generated**: `codegen/lib/code/shapes.py` → `collision/blocks.rs`: ~900 unique
  `LazyLock<VoxelShape>` + a table indexed by block-state id; data extracted from the vanilla jar with the
  Pumpkin extractor mod + Burger. Shape offsets (bamboo/dripstone) just added.
- Missing (README + TODOs): entity pushing, sprint-swimming, world border, slow falling, freezing, vehicles,
  client-side block placement prediction.
- **Testing**: `azalea-physics/tests/physics.rs` builds a headless `App` with plugins, inserts synthetic chunks,
  spawns an entity, runs `GameTick` N times and asserts exact positions (gravity, slabs, walls, negative
  coords, "afk pool"). `azalea-client/src/test_utils/simulation.rs` injects raw clientbound packets
  (`RawConnection::injected_clientbound_packets`) for full-stack tests. Pathfinder tests (`pathfinder/tests.rs`)
  run `Simulation` (fake world + real physics) and assert the bot reaches the goal within N ticks.
  Benches in `azalea/benches/{physics,pathfinder}.rs`.
- Movement sending (`plugins/movement.rs`): vanilla-like — send pos/rot only when changed, force every 20 ticks.

## 5. Pathfinder (`azalea/src/pathfinder/`)

- Baritone-derived **A\*** with Baritone's "best partial path" trick: 7 coefficients
  `[1.5,2,2.5,3,4,5,15]` track best nodes by `h + g/c`; on timeout returns the best partial path
  (`astar/mod.rs:35`). Custom radix-like heap + node map, `RelBlockPos` (compact coords relative to origin).
  Timeouts: `min_timeout`/`max_timeout` as `Time` or `Nodes`.
- **Moves** are successor functions over a `CachedWorld`: `basic` (forward, ascend, descend, diagonal,
  downward), `parkour` (1–3 gaps), `uncommon`; each has a cost (Baritone cost tables in `costs.rs`),
  an execute fn (drives `walk/sprint/jump/look_at` per tick) and an `is_reached` fn. Optional mining
  (`mining.rs` cost via tool speed), custom move sets via `PathfinderOpts`.
- **Goals** (`goals.rs`): `BlockPosGoal, XZGoal, YGoal, RadiusGoal, ReachBlockPosGoal`, combinators
  `Inverse/Or/Ors/And/Ands`; trait = `heuristic(pos)` + `success(pos)`.
- **Execution**: search tries an instant synchronous path from the next node, else spawns on
  `AsyncComputeTaskPool` (off the ECS thread). Executor follows nodes, detects deviation
  (distance-from-line), **patches** stale segments when blocks change (`execute/patching.rs`), recalculates on
  world change. Optional `SimulationPathfinderExecutionPlugin` simulates ahead to skip nodes/sprint-jump
  smoothly (more CPU).

## 6. Performance / scaling

- Measured (mc-agents/bot-azalea, 2026): **15–20 MiB RSS per single-bot process (~8 MiB bot itself)**; 50 bots
  in 50 containers = 396 MiB. Idle bot ~3% of a core; default bevy task pools spun **22 threads waking 60×/s =
  0.25–0.5 core idle** until forced to one thread. (Per-process numbers; swarm shares chunks so is lower/bot.)
- Issue #319 (closed, no maintainer fix documented): swarm pinned to **one core by `LocalSet`**; "swarms cannot
  scale beyond ~100 bots efficiently"; workaround = multiple processes. Issue #325 (open): swarm lag on heavy
  chunk loads/block updates → bots send weird movement (tick stalls because decode+decrypt run on ECS thread).
- Structural costs at 1000s of bots: single global write lock; 60 Hz Update polling every socket even if idle;
  per-event task spawn (20k spawns/s at 1000 bots from `Event::Tick` alone); full physics for every bot every
  tick; bevy schedule overhead is small per system but systems iterate all bots serially.
- Non-goal statement: README explicitly lists "Bedrock edition" as a non-goal.

## 7. Copy vs avoid for Acacia

Copy:
- Crate split: pure protocol → `world` (chunks, palette, shared storage) → `entity/state` → `physics` →
  `client plugins` → `pathfinder`/façade. But keep protocol free of domain types.
- Shared chunk store keyed by (server, dimension): `Weak` in shared map, strong `Arc` ring per bot,
  **skip decode when chunk already shared**. Biggest memory win for many bots at one spawn/AFK area.
- Per-bot entity-id index; shared entity table with ref-count; dedup of relative updates.
- Generated collision-shape table indexed by block state; exact-position physics tests via headless
  tick simulation + packet injection harness; Baritone-style A\* with partial paths, move/goal traits,
  path patching, off-thread search with node/time budget.
- Command/intent model: user API enqueues intents; the tick applies them (no races with tick logic).

Avoid:
- One global ECS behind one lock on one thread; decode/decrypt on the tick thread; 60 Hz polling of idle
  sockets; spawning a task per event; running physics for bots that are idle on ground.
- bevy task-pool defaults (thread-per-core wakeups).

bevy_ecs vs plain structs (for 1000s of mostly-idle AFK bots): ECS gives composability (plugins,
observers) and cache-friendly iteration, but azalea's cost is the architecture around it, not archetype
storage. A plain `struct BotState` owned by each bot's task (our current per-bot-task model) scales across
all cores with no lock, and decode-on-demand stays on the bot's task. Recommendation: plain per-bot
structs + a small trait-based "module" system (on_packet/on_tick hooks), sharded by core; shared
`Arc<WorldShared>` (chunks, entities) with fine-grained locks. Consider a per-shard ECS only if cross-bot
queries (swarm coordination) become central.

Bedrock caveats: Geyser servers validate Java-side physics (Geyser converts PlayerAuthInput to Java moves;
anticheats like Grim run Java simulation with Bedrock allowances), so azalea's Java physics port is the right
reference model, but block states/chunks arrive in Bedrock format (runtime ids, subchunks) — collision shapes
must be keyed via a Bedrock→Java block-state mapping (Geyser mappings) or regenerated for Bedrock ids.
