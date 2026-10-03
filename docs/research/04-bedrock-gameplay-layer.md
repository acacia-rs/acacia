# 04 — Bedrock gameplay layer (movement, physics, world, interactions) for Geyser + Boar

Target: Bedrock 1.26.51 / protocol 2193 (`tools/codegen/data/protocol.json`). Packet/field names below are
OUR generated names (`crates/acacia-proto/src/generated`). Researched 2026-09-30.
Sources read: Boar master (MIT, pushed 2026-09-30), Geyser master (MIT), Mojang bedrock-protocol-docs
`additional_docs/{PlayerMovementOverview,MovementProtocolDeprecation,AntiCheatServer}.md`, gophertunnel
`packet/player_auth_input.go`, oomph-ac/bedsim (MIT), minecraft-data bedrock 1.26.30 data.

## 0. Key facts up front

- 1.26 has **no client-authoritative mode**. `StartGame` no longer carries a movement mode (only
  `rewind_history_size`, `server_authoritative_block_breaking`, `block_network_ids_are_hashes`); authority is
  `SetMovementAuthority{movement_authority: client|server|server_with_rewind}`. Clients send **only
  `PlayerAuthInput`** for self-movement (client→server `MovePlayer` is deprecated).
- Geyser `buildStartGamePacket`: `rewind_history_size=0`, `server_authoritative_block_breaking=true`,
  `server_authoritative_inventory=true`, never sets hashed block ids (so **runtime-ID palette, not hashes**),
  custom blocks go in `block_properties`. Boar then rewrites StartGame to `SERVER_WITH_REWIND`, rewind history 20.
- Geyser chunks: full `LevelChunk` (`cache_enabled=false`, no sub-chunk-request mode), section format **v9**,
  3D biome palettes per section. No `SubchunkRequest` traffic needed on Geyser.
- Boar only **kicks** for: NaN/Inf pos/rot/delta (BadPacketA), yaw not in [-180,180] or pitch not in [-90,90]
  (BadPacketB), `tick < 0`, and **unanswered NetworkStackLatency for > 15 s** (`max-latency-wait`, "Timed out!").
  Everything else (Prediction/Timer/Reach) = **flag + rewind/setback**, but servers can hook violations via the
  Boar API to punish — treat any flag as a ban risk.

## 1. Movement — `PlayerAuthInput` (sent once per client tick, 50 ms)

Wire order (our struct, matches gophertunnel `Marshal`):

| field | what to send |
|---|---|
| `pitch`, `yaw` | degrees; yaw **wrapped to [-180,180]**, pitch clamped [-90,90] (Boar kicks otherwise) |
| `position` | **eye position** = feet + 1.62 (sneak 1.27, crawl/swim 0.4); Boar/Geyser subtract `PLAYER_ENTITY_OFFSET` |
| `move_vector` | WASD vector, x=strafe, y=forward, each in [-1,1]; (0,0) idle |
| `head_yaw` | = yaw |
| `input_data` | `Vec<InputData>` = varint count + zigzag32 flag ids (list, not bitset, in 1.26) |
| `input_mode` | Mouse(1) for a PC-like bot |
| `play_mode` | Normal(0) |
| `interaction_model` | Crosshair(1) for KBM (Touch=0, Classic=2) |
| `interact_rotation` | (pitch, yaw) — used by Boar reach & Geyser touch aim |
| `tick` | client tick counter, +1 per packet, never negative |
| `delta` | **tick-end velocity** (velocity after gravity/drag), NOT pos diff. Idle on ground: (0, -0.0784, 0) |
| `transaction` | Some only with `ItemInteract` flag (UseItem transaction in auth-input) |
| `item_stack_request` | Some only with `ItemStackRequest` flag |
| `block_action` | Some only with `BlockAction` flag (server-auth breaking) |
| `vehicle_rotation`, `predicted_vehicle` | None unless riding a client-predicted vehicle |
| `analogue_move_vector` | (0,0) for KBM |
| `camera_orientation` | unit look vector from yaw/pitch |
| `raw_move_vector` | = move_vector before sneak/permission scaling |

Flags that matter: `VerticalCollision` (Geyser `onGround = VerticalCollision && delta.y < 0` — an idle
player standing on ground MUST set it with negative delta.y, else Java sees it floating), `HorizontalCollision`,
`Up/Down/Left/Right/UpLeft/UpRight/DownLeft/DownRight` (match move_vector), `Jumping`/`JumpDown`/`StartJumping`
(StartJumping only on the tick the jump fires, and only when on ground), `StartSprinting`/`StopSprinting`
(Boar strips StartSprinting if forward input <= 0), `Sneaking`/`SneakDown`/`StartSneaking`/`StopSneaking` +
`SneakCurrentRaw` (Boar uses the Raw flags as truth on Mouse input mode), `HandledTeleport`, `MissedSwing`,
`BlockBreakingDelayEnabled`. Collision flags are overwritten by Boar with its own prediction before Geyser sees them.

**Idle cadence.** Vanilla sends `PlayerAuthInput` every tick (20/s) regardless of movement — it is the clock of
server-auth movement (Mojang PlayerMovementOverview). Geyser forwards a Java move only on change or every 20
ticks ("position reminder"), and sends `ServerboundClientTickEnd` per PlayerAuthInput.

**Timing (Boar Timer, `check/impl/timer/Timer.java`).** Balance = wall-time − (tickDelta × 50 ms). Flag when
balance > ~63 ms ahead ("player is ahead") → packet cancelled + setback. Falling behind (lag) is tolerated and
clamped at `max-balance-advantage` 2000 ms. Rules for us: never burst to catch up; one packet per real 50 ms;
tick +1 per packet. Timer inactive while in loading screen and for 20 packets after it.

**Loading screen.** Boar sets `inLoadingScreen=true` on StartGame; cleared only by `ServerboundLoadingScreen`
`{type: 2 (End; 1 = Start), loading_screen_id}` (id matching `ChangeDimension`'s, or None at spawn). Prediction/Timer don't run until then
(+2/+20 ticks). Vanilla sends Start/End; a bot should too, after chunks around it are loaded.

**Corrections (client side must emulate):**
- `CorrectPlayerMovePrediction{prediction_type, position, delta, rotation, angular_velocity, on_ground, tick}` —
  rewind to `tick`, set pos/vel/on_ground, re-simulate stored inputs up to now. No explicit ack flag; Boar
  processes it when the NSL sent with it is answered, then replays its own history.
- `MovePlayer{mode: Teleport|Reset|Normal, position(eye), tick,...}` for our runtime id — Boar converts
  Normal→Teleport for own player. Next PlayerAuthInput must report exactly that position (Boar tolerance 1e-3)
  **and** carry `HandledTeleport`, delta zeroed; otherwise Boar re-sends the teleport. Geyser ignores all movement
  until position matches (`session.confirmTeleport`).
- `SetEntityMotion{runtime_entity_id, velocity, tick}` (knockback) — apply as velocity at start of next tick;
  Boar tries with/without velocity so modest timing slop is fine.
- `SetEntityData`/`UpdateAttributes`/`MobEffect` for self may carry a `tick` and affect movement (speed attribute
  `minecraft:movement`, size/flags). `UpdateAbilities` (may_fly/flying → Boar exempts movement).
- Unloaded chunk at player position → Boar teleports back to last valid pos every tick until chunk acked.

## 2. Physics data & constants

**Block ids.** Geyser → runtime-ID mode. Palette = vanilla canonical states + custom blocks from
`StartGame.block_properties`, sorted (vanilla client sorts by FNV-1 64 of block name, states in canonical order);
runtime id = index. If `block_network_ids_are_hashes` ever true: id = FNV-1a-32 of LE-NBT `{name, states}`.
Verify palette via `StartGame.block_pallette_checksum` / by checking air id in first chunk.

**Sources (licence):**
- minecraft-data bedrock (MIT): `bedrock/1.26.30/{blocks,blockStates,blockCollisionShapes}.json` is what
  1.26.51 maps to in `dataPaths.json`. blockStates = 16 913 states in runtime-id order (name, states, version);
  blockCollisionShapes = per-block shape ids → 342 AABB lists. Java-derived for many blocks → patch list below.
- pmmp/BedrockData (CC0): `canonical_block_states.nbt`, `block_state_meta_map.json`, item/runtime tables.
- Dragonfly (MIT, Go) `server/block/model/*`: Bedrock-accurate `BBox` per block model (fence, wall, stairs,
  door, trapdoor, chest=0.875…); good for hand-porting shapes and friction.
- Boar `collision/BedrockCollision.java` (MIT): list of Bedrock-vs-Java collision diffs: chest/ender chest,
  scaffolding (dynamic), cauldron, trapdoor, door, sea lantern, bell, bamboo, pointed dripstone, end portal frame,
  powder snow, sea pickle, turtle egg, dragon egg, honey, lectern, conduit, cactus.
- Mojang/bedrock-samples (licence NOASSERTION — data only, don't vendor): vanilla block/entity definitions.

**Constants (bedsim `constants.go` / Boar `PlayerData`):** gravity 0.08, vertical drag ×0.98, horizontal
`friction×0.91` with default block friction 0.6 (ice 0.98, slime 0.8), ground accel
`movement×0.21600002/f³`, air accel 0.02 (0.026 sprint, independent of attribute), movement attribute 0.1
(sprint ×1.3 via modifier), jump 0.42 (+0.1/level jump boost), **step height 0.5625** (Java 0.6), bbox 0.6×1.8,
eye 1.62 / sneak 1.27, climb 0.2, water drag 0.8 (sprint-swim 0.9), terminal velocity -3.92 (emergent),
jump delay 10 ticks, sneak impulse ×0.3, input ×0.98. Bedrock-vs-Java: step height, eye/sneak heights,
yaw wrapping, jump-delay semantics, bamboo/dripstone offsets, several collision shapes above.

## 3. World / chunk state

- Geyser path: `LevelChunk{x, z, dimension, sub_chunk_count, highest_subchunk_count, cache_enabled=false,
  payload}`; payload = `sub_chunk_count` sections + biome sections (count = dim height>>4, 127<<1|1 = "copy
  previous") + border byte + block entities (NBT). Then `UpdateBlock`, `UpdateSubchunkBlocks`,
  `NetworkChunkPublisherUpdate` (radius/center), `ChunkRadiusUpdate` (reply to `RequestChunkRadius`).
- Sub-chunk-request path (BDS/other servers): `LevelChunk.sub_chunk_count` = -2/-1 → send `SubchunkRequest{
  dimension, origin, requests: Vec<Vec3i8>}` → `Subchunk{cache_enabled, dimension, origin, entries}` with per-entry
  `result` (Success / SuccessAllAir / ChunkNotFound…) and `payload`. Client blob cache: send
  `ClientCacheStatus{enabled:false}` to avoid `ClientCacheMissResponse`/blob handling entirely.
- Section v9: `u8 version=9, u8 storage_count, i8 y_index`, then per storage: `u8 (bits<<1)|1(runtime)`, words
  `ceil(4096/(32/bits))` u32 LE (bits ∈ 0,1,2,3,4,5,6,8,16; bits 0 = single entry, no words), `zigzag varint
  palette_len`, palette entries zigzag varint runtime ids. Storage 0 = blocks, storage 1 = liquid layer
  (waterlogged water) — needed for liquid physics. Index order XZY: `(x<<8)|(z<<4)|y`. v8 = same minus y_index.
  Boar's `util/geyser/ChunkDecoder.java` + `PaddedBitArray`/`Pow2BitArray` are a ~200-line reference.
- For collision you need storage 0 (+1 for water), not biomes/lighting/block-entities (skip, but must parse
  length to reach block entities only if you want signs/chests). Keep palette+packed bits per section:
  ~0.3–2 KB typical, 8 KB worst (u16 flat). Overworld 24 sections → ~10–60 KB/chunk; radius 8 (~290 chunks)
  ≈ 5–20 MB.

## 4. Interactions

- **Attack:** look at target first (Boar Reach ray-casts from eye with your rotation vs interpolated hitbox,
  ≤ 3.005 blocks; "failed to find entity in sight" otherwise). Send `Animate{action_id: SwingArm,
  has_swing_source/swing_source}` then
  `InventoryTransaction` with `types::Transaction` UseItemOnEntity: `{entity_runtime_id, action_type: Attack(1),
  hotbar_slot, held_item, player_pos(eye), click_pos(0)}`. Air swing = `MissedSwing` flag in PlayerAuthInput.
  Respect 1.9+ attack cooldown (Java backend).
- **Use item / place block:** `InventoryTransaction` (legacy request id 0, empty actions) with
  `TransactionUseItem{action_type: ClickBlock|ClickAir|BreakBlock, trigger_type: PlayerInput, block_position,
  face(0-5), hotbar_slot, hand, held_item, player_pos(eye), click_pos(0..1 in block), block_runtime_id(clicked
  block), client_prediction: Success, client_cooldown_state}`. Geyser handles ClickBlock/ClickAir from the
  standalone packet; in auth-input it only accepts action_type 2 (BreakBlock) via `ItemInteract` flag.
- **Break (server-auth breaking, Geyser+Boar):** PlayerAuthInput with `BlockAction` flag and
  `block_action = [PlayerAuthInputBlockActionItem{action, position, face}]`: `StartBreak` → `ContinueBreak`
  (each tick while holding) → `PredictBreak` when done, `AbortBreak` to cancel; within 12 blocks. Java backend
  enforces real break time → compute vanilla break time from hardness/tool (minecraft-data `blocks.json`).
  Swing via `Animate` periodically like vanilla.
- **Containers:** `Interact{action_id: OpenInventory, target_entity_id: own runtime id}` for own inventory; world containers
  via ClickBlock on the chest → server `ContainerOpen{window_id, window_type, coordinates, runtime_entity_id}`;
  close with `ContainerClose{window_id, window_type, server: false}` (answer server-initiated closes too).
- **Moving items:** server-authoritative inventory → `ItemStackRequest{requests:[{request_id(negative,
  decrementing), actions: Take|Place|Swap|Drop|Destroy|CraftRecipe|…, filter_strings, cause}]}` →
  `ItemStackResponse` (status ok → apply new stack net ids). Can also ride in PlayerAuthInput via
  `ItemStackRequest` flag (vanilla does so for some actions).
- **Hotbar:** `MobEquipment{runtime_entity_id, item, slot, selected_slot, window_id=0}`.
- **Look:** only via PlayerAuthInput yaw/pitch/head_yaw (+ interact_rotation, camera_orientation).

## 5. State tracking packets

- Entities: `AddPlayer`, `AddEntity`, `AddItemEntity`, `RemoveEntity`, `MoveEntity` (absolute),
  `MoveEntityDelta` (flags + changed coords), `MovePlayer` (other players), `SetEntityData` (flags, size, name),
  `SetEntityMotion`, `SetEntityLink` (riding), `UpdateAttributes`, `MobEffect`, `EntityEvent`, `TakeItemEntity`,
  `Animate`. Keep runtime-id→entity map; interpolate like Boar `PositionInterpolator` for aim.
- Self: `StartGame` (runtime id, pos, gamemode, dimension), `PlayStatus{PlayerSpawn}` → send
  `SetLocalPlayerAsInitialized`; `UpdateAttributes` (`minecraft:health`, `minecraft:movement`, hunger…),
  `SetHealth`, `SetPlayerGameType`/`UpdatePlayerGameType`, `UpdateAbilities`, `Respawn` (state flow: server
  searching → client sends `Respawn{ClientReadyToSpawn}` + `PlayerAction{Respawn}`), `ChangeDimension` →
  `ServerboundLoadingScreen`, `SetSpawnPosition`, `MovePlayer` teleports.
- Inventory: `InventoryContent{window_id,…}` (0 = inventory, 119 offhand, 120 armor, 124 UI/cursor),
  `InventorySlot`, `PlayerHotbar`, `ItemStackResponse`, `ContainerOpen/ContainerClose`,
  `CreativeContent`/item registry from StartGame `itemstates` for item ids.
- UI: `SetDisplayObjective`, `SetScore`, `RemoveObjective`, `BossEvent`, `SetTitle`, `Text`.

## 6. Reference implementations to port from

| project | lang/licence | what | verdict |
|---|---|---|---|
| **oomph-ac/bedsim** | Go, MIT, active (2026-09) | standalone Bedrock player movement sim extracted from Oomph: collisions, step, edge-avoid (sneak), liquids, bubble columns, climbing, gliding, riptide, pose sizes, teleports; provider interfaces for world/effects/equipment | **Best port target for physics** — library-shaped, Bedrock-accurate, MIT |
| **Boar** (oryxel1/Boar) | Java, MIT, active | vanilla-accurate prediction engine (`prediction/engine/*`, `ticker/*`, `collision/*`), chunk decoder, the AC we must pass | Port-compatible (MIT); use as the **oracle**: run our sim vs Boar logic in tests |
| Oomph (oomph-ac/oomph) | Go, **SSPL** | full MITM anticheat, origin of bedsim | read-only reference; do not copy code |
| Dragonfly (df-mc/dragonfly) | Go, MIT | server; trusts client movement; has block BBox models, item/entity physics (`entity/movement.go`) | use for block shapes/friction, not player physics |
| Geyser `CollisionManager`/`BlockCollision` | Java, MIT | Java-shape collision for Bedrock pos | secondary |
| prismarine-physics PR #152 | JS, MIT, draft RFC (2026-09-20) | Bedrock mode for mineflayer physics (fits speeds) | approximate, not prediction-grade |
| torzodmc/mineflayer-for-bedrock | TS, no licence | bot w/ AABB physics + PlayerAuthInput loop | inspiration only (unlicensed) |
| prismarine-chunk (bedrock) | JS, MIT | v8/v9 sub-chunk + hashed-id decode | chunk-format reference |
| gophertunnel | Go, MIT | protocol only, no physics | wire reference |

## 7. Anticheat-safe minimum for an AFK bot (Geyser + Boar)

Hard requirements (kick otherwise):
1. Answer every `NetworkStackLatency` with `needs_response` **in order**, timestamp × 1e6 (non-PS platform
   magnitude; Boar divides by 1e6, or 1e3 for PS5 device OS), within 15 s. Verified live: this alone kept an
   idle bot 76 s+.
2. If you send PlayerAuthInput at all: finite numbers, wrapped yaw, clamped pitch, non-negative monotonic tick,
   ≤ 20 packets/s.

Option A — send nothing movement-wise (current state). Boar never runs prediction/timer (no auth input), Geyser
sends no Java moves; Java vanilla doesn't kick a client that never sends move packets. Risk: server-side AFK
plugins and "not ticking" heuristics; `ClientTickEnd` never reaches Java (Java ACs that expect it could
notice, though Floodgate players are usually exempt from Java ACs).

Option B — vanilla-faithful idle (recommended once physics exists, cheap even without full physics):
1. After `PlayStatus PlayerSpawn`: `SetLocalPlayerAsInitialized`, `ServerboundLoadingScreen` End.
2. Every 50 ms: PlayerAuthInput with pos = last server pos (eye), rot constant, move_vector 0,
   flags `{VerticalCollision}` (+ raw sneak/jump flags off), delta (0,-0.0784,0) when standing on a full block,
   tick+1. This equals Boar's prediction for a grounded idle player (offset 0) as long as the block below is
   solid — requires knowing ground (at minimum: server teleport Y is on-ground).
3. Obey teleports (`MovePlayer` → echo pos + `HandledTeleport`) and corrections (`CorrectPlayerMovePrediction`
   → adopt pos). Apply `SetEntityMotion` knockback via physics or you'll be rewound (not kicked).
4. Occasional small rotation changes are free (rotation is not predicted; only wrap/clamp checked).
5. Suggested validation: MITM a real 1.26.51 client through gophertunnel's proxy to capture a vanilla idle
   PlayerAuthInput stream (exact flag set incl. `BlockBreakingDelayEnabled`, raw flags) and diff ours against it.
