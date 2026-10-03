# Acacia — design

An optimized Rust client library for Minecraft: Bedrock Edition. It aims to be a faster
replacement for PrismarineJS `bedrock-protocol`. It is a library only: the AFK service and
the website get rebuilt on top of it later.

Research behind these decisions: `docs/research/`.

## Decisions

| Topic | Decision | Why |
|---|---|---|
| Versions | Latest protocol only, one per release. Start with 1.26.51 (protocol 2193) | No per-version branching. Geyser servers accept the latest client. Older versions: pin an older crate version |
| Packets | Generate all of them; decode on demand | A packet arrives as `id + Bytes`. `pkt.decode::<T>()` parses only when asked, so ignored packets (chunks) cost only framing |
| I/O | Network-free core plus a thin tokio layer | Protocol logic is bytes in, bytes and events out, with no sockets and no async. The same code runs the real client, the fake-server tests and benchmarks |
| Transport | RakNet and NetherNet (direct connect, Realms via the signaling service, or LAN); `TransportKind::Auto` prefers RakNet | Real servers still answer RakNet (probe on 2026-09-29); BDS 26.60 drops it. Auto takes RakNet if it pings, else NetherNet if `GET /v1/join` answers. NetherNet needs an online login. Realms: `ClientBuilder::signaling`; LAN: `discover_lan` + `ClientBuilder::lan`; friends' worlds: `Account::friend_worlds` + `friend_builder` (`ClientBuilder::friend`), MPSD membership over RTA held for the connection. All trickle ICE (`trickle.rs`), host candidates first, own sans-IO TURN client as Realm fallback; spec docs/research/nethernet-signaling.md, docs/research/friends-join.md |
| WebRTC | str0m 0.24 (`rust-crypto`), sans-IO | Runs inside the network-free design and lets ICE go through the SOCKS5 relay; no C toolchain |
| Fingerprint | Bots look like a vanilla **Android** player and never share identifiers | Online logins are Android-titled, so ClientData, input modes and behaviour must agree with it. Device ids derive from the account (`login/device.rs`); DTLS certs are per connection. Evidence: docs/research/{vanilla-capture-2026-10-01,vanilla-input-gaps}.md |
| Idle input | Bots without physics still send the vanilla standing-still `PlayerAuthInput` every tick (`movement/idle.rs`), the vanilla spawn sequence with its frame hitches (`spawn.rs`), sub-chunk requests (`subchunks.rs`) and ClientMovementPredictionSync (`prediction_sync.rs`), on a jittered ~51 ms tick (`cadence.rs`) | A silent player is the easiest bot tell; full physics only for bots that walk |
| Blob cache | Reported on, like vanilla. The session answers per tick from a `BlobStore`; `blob_cache_dir` keeps one file per account across joins, holding hashes only unless the bot reads terrain | The cache status shows on every join; a returning player has a warm cache (docs/research/blob-cache.md) |
| Events | Typed `BotEvent`s, each group opt-in (`BotConfig.events`); chat patterns are regexes over plain text | Bots on chat-spam servers don't decode what nobody reads; Geyser sends player chat as raw lines |
| Gameplay actions | Forms, crafting and workstations, eating/equip, signs/books/beds, riding, fishing and elytra as awaitable `Bot` methods; human-looking delays from `human.rs` | Built from observed server behaviour first, then refit to a vanilla capture (docs/research/vanilla-actions-2026-10-02.md). Gliding and vehicle physics wait for the physics rework |
| Reaction times | The session defers login, pack and chunk-radius replies by vanilla's measured delays (`session/deferred.rs`) | Instant replies are a timing tell |
| Auth | Separate `acacia-auth` crate | Device code, Xbox/XSTS, PlayFab, session/start and the multiplayer token, with a pluggable `TokenCache`. Offline mode for tests. Browser or password logins stay outside and hand in tokens |
| Proxies | SOCKS5 UDP relay per client, in v1 | Lets bots spread across IPs. The auth HTTP calls can use the same proxy |
| Performance | Targets set after v1 works | Keep the hot path free of allocation and copying. The benchmark setup (fake server, replayed traffic, CI tracking) comes in the optimization phase |
| License | MIT; ported code keeps its notice (`acacia-physics/LICENSE-bedsim`) | Never port from GPL/LGPL sources (Pumpkin's NetherNet, PocketMine, PowerNukkitX); citing their behaviour is fine |

## Crates (dependencies point downward only, no cycles)

```
acacia-client             tokio sockets and timers, SOCKS5 (UDP + CONNECT), NetherNet signaling I/O, public Client API
 ├─ acacia-session        network-free session over a RakNet or message link: handshake, ECDH + AES-CTR, compression, batching;
 │                        `server::ServerConnection` is the server side (FakeServer, mitm)
 │   ├─ acacia-raknet     network-free RakNet client and server: reliability, split packets, ACK/NACK, ordering
 │   └─ acacia-proto      generated packet structs + codec (varints, lazy Packet)
 ├─ acacia-nethernet      network-free NetherNet: signaling HTTP bytes (both sides), signal text, signaling-service
 │                        session, STUN/TURN client, a=identity, fragment framing, str0m connection (offer or BDS-style answer)
 └─ acacia-auth           MSA device code → Xbox → XSTS → PlayFab → session/start → multiplayer token
acacia-viewer             (app) world viewer: a bot on a tokio thread, winit window, fly camera
 └─ acacia-render         wgpu terrain renderer over acacia-world (no network): pack loading, meshing, drawing
acacia-mitm               proxy library (record, drop, rewrite, inject) and recording CLI for vanilla-client
                          captures over RakNet or NetherNet direct connect, local BDS or (`--online`) real servers
tools/codegen             minecraft-data bedrock/<ver>/protocol.json → acacia-proto sources
tools/capdiff             compares what two clients sent in captures (vanilla vs bot): order, delays, cadence, fields
acacia-testserver         (dev) FakeServer replays a recorded BDS join over loopback RakNet, then takes send/recv/kick
                          from the test; capture reader
```

Fingerprint checks without BDS: `crates/acacia-bot/tests/fake_server.rs` joins an idle bot to the replayed
session and asserts the vanilla timeline; `FAKE_CAPTURE=<file>` saves the exchange for `capdiff`.
`fake_server_handle.rs` shows server-driven scenarios (forms, kicks) after the join. New
scripts: `cargo run -p acacia-testserver --example make_script` on a capture of a bot (never the real game:
scripts ship in the repo and replay PlayerList and skins).

`acacia-mitm` terminates only the encryption handshake (each side gets its own key) and re-signs the
game's Login: offline by default, or with `--online <account>` as that signed-in account (sign the game
into the same one, since its client data passes through). Everything else passes through as it came, cache and pack replies
included. `--transport nethernet` (needs `--online`) serves BDS-style signaling to the game and dials the
NetherNet BDS; add the server by this machine's LAN IP, not 127.0.0.1. Capture format: `crates/acacia-mitm/src/record.rs`. `name` is acacia-proto's struct name; captures
made before 2026-10-02 used gophertunnel's names. As a library (`Proxy`), per-player `Interceptor`s get each
packet as a lazy `RawPacket` and forward, drop or replace it; an `Injector` adds packets either way. Changed
batches are re-encoded with acacia-session's codecs for that side. Recording stays a built-in layer ahead of the
interceptors: it must see the codec-switching packets the proxy owns, and logs packets as they arrived
(`crates/acacia-mitm/src/intercept.rs`).

## Reference implementations

- **gophertunnel** (Go, MIT): the gold standard for the client flow and the new auth tokens.
- **PrismarineJS bedrock-protocol + minecraft-data**: source for code generation and behaviour to match.
- **CloudburstMC Protocol** and **Mojang bedrock-protocol-docs**: for cross-checking fields.

## Milestones

1. ✅ **RakNet client core** plus the tokio layer.
2. ✅ **Code generator**: all 246 packets, byte-exact against bedrock-protocol fixtures.
3. ✅ **Session**: offline login reaches spawn on BDS 1.26.52; 10 concurrent bots with 0 failures.
4. ✅ **Auth**: the full chain is verified live. A real account joined BDS in `online-mode=true`. Accounts that
   need Yoti age verification fail at SISU with `Error::ActionRequired { url }`.
5. ✅ **SOCKS5** UDP relay (tested against a mock proxy). Auth HTTP can use a proxy via the `socks` feature.
6. ◐ **Examples** (`afk`, `swarm`, `device_login`). Next: an online join on a real server, then the optimization phase.
7. ✅ **NetherNet** direct connect: joins and rejoins BDS 26.5x directly, over the LAN and through SOCKS5;
   Auto picks it on a NetherNet-only server; HTTPS-then-HTTP signaling and server identity checked.
   ✅ Realms (JSON-RPC signaling) and ✅ LAN (game-hosted world) joined live 2026-10-02
   (`examples/realm_join.rs`, `examples/lan_join.rs`). ◐ Friends' worlds (`examples/friend_join.rs`): unit-tested,
   not yet joined live.
8. ◐ **Indistinguishable bots** (see Fingerprint above): signaling, DTLS cert, Login, persona, idle input and
   spawn/respawn sequences match vanilla.
9. ◐ **Swarm** (docs/swarm.md): sharded bots across servers and realms with reconnect policy and a
   coordinator-ready handle. Tested against fake servers; next: a live run and the load benchmark.
10. ◐ **Renderer** (crates/acacia-render/README.md): ✅ world viewer on BDS 26.52 (textured terrain, greedy
   meshing with AO, fly camera), biome colours. ~175 MB committed at radius 8, of which ~24 MB Rust heap
   (the rest is the Vulkan driver; DX12 is far worse). Next: lighting, cave culling.

## Gotchas / open items

- BDS 26.60 removes RakNet, so local RakNet tests need BDS ≤ 26.5x with `transport=raknet`. BDS also refuses
  `online-mode=false` while `allow-list=true`.
- NetherNet vs BDS (verified 2026-10-01; wire spec docs/research/nethernet-wire.md):
  - The offer must carry `a=identity` with a real MultiplayerToken, even on `online-mode=false`; otherwise the
    `POST /v1/join` answer is HTTP 200 with body `37` (IdentityNotAllowed).
  - BDS sends ServerToClientHandshake but expects **no** encryption afterwards (DTLS covers it); encrypting
    makes it ignore the client silently. Batches carry no 0xfe header.
  - A local close must send a `Disconnect` packet and close SCTP/DTLS, or a rejoin gets `ServerIdConflict`.
  - str0m buffers at most 128 KiB, so frames are capped there (conn.rs `MAX_FRAME`).
- Login: vanilla 1.26.52 sends a token-only envelope and alphabetically sorted client-data claims
  (`login/request.rs`). BDS online-mode accepts it (verified 2026-10-01).
- RakNet client GUIDs must be negative: go-raknet (Dragonfly) rejects positive ones and blocks the IP 10 s.
- Bedrock UUIDs travel as two little-endian u64 halves; `proto::manual::Uuid` keeps wire bytes and converts
  only in `Display`/`FromStr`.
- Joining BDS sends `Respawn(searching)` without a death; vanilla answers none of them (`handlers.rs`).
- The shield item ID is process-global in `acacia-proto` (see docs/proto.md). The session sets it from
  ItemRegistry, so every connection in a process shares it. That's fine while servers use vanilla item IDs.
- Resource pack responses need both the status code and its exact name string
  (`downloadingfinished` / `resourcepackstackfinished`). minecraft-data only types the name as a string.
- Geyser/Boar compatibility: the client-data ServerAddress needs `:port`, and NetworkStackLatency probes need
  replies. See `session/handlers.rs`. Local BDS tests catch neither, so test against a live Geyser server.
- Geyser ignores `ItemStackRequest`s unless an inventory screen is open; items/ opens and closes the own
  inventory around requests, and the bot acknowledges server-initiated `ContainerClose`.
- `MobEquipment` sends the held item **without** its stack id. Geyser (`CodecProcessor.fakeItemDescriptorRead`)
  reads a present id as a tagged variant and rejects the untagged form. That is an "Illegal packet": the lab
  disconnects, and on a live Geyser server it hung the session ("already online" on relog). BDS rejects the
  tagged form. Both accept no id. Found with the local Geyser lab (`tools/lab.ps1`), since the live symptoms alone
  were misleading.
- Crafting/workstation requests (docs/research/workstations.md): stack-request action legacy ids are the
  variant + 2 from `LabTableCombine` on, and the created-output stack (UI slot 50) is named by the request's
  own id. Geyser rejects multi-result crafts and wants recipe-book `Consume`s grouped per recipe cell.
- Gameplay actions vs BDS (verified live 2026-10-02, docs/research/live-action-tests.md; rerun with
  `tools/testbox-bds.sh` + `examples/actions.rs`): BDS sends no slot update for client-predictable changes
  (craft results, page edits, signing, pickups) and rejects the next click made holding a mispredicted item,
  so predictions must be exact. It crashes on `CraftResultsDeprecated` in anvil and stonecutter requests.
- Local Geyser lab (`tools/lab.ps1`: Paper 26.2 + Geyser Standalone 2.11.3 + Boar, Bedrock :19150) mirrors
  a typical Geyser + Boar server. Use it before any live test: its logs show decode errors and Boar flags.
- Movement vs the servers (verified with `examples/course.rs` and `examples/drills.rs` on BDS and the lab):
  - Keyboard input is only ever -1/0/1: BDS moves at full speed whenever Up is set, whatever the
    move vector says, so fractional input drifts until corrected. `Controls` quantizes (movement/mod.rs).
  - The sprint key only starts a sprint: it lasts until forward is released (Movement latches it;
    BDS and Boar ignore StopSprinting with forward held). Diagonal keys are normalized to length 1
    before the simulation's 0.98 input scale, as the client does.
  - `PlayerAuthInput.position` is feet + 1.62 in every pose; the pose eye height is for raycasts only.
  - BDS stamps corrections with the input tick whose end state they give, a teleport stamped T holds
    the player at the target through tick T+1 (liquid currents still push it) and simulates on from
    T+2, and a knockback stamped K moves the player on tick K+1. Each can arrive after we simulated
    that tick (even 20 ticks late under load): the bot rewinds from a per-tick state history and
    replays its inputs (movement/rewind.rs). Geyser stamps tick 0 and Boar holds the player on the
    acknowledging tick, so tick-0 teleports and knockbacks apply on the next tick.
  - Water contact counts any liquid cell the shrunk box overlaps, however shallow (liquid.rs).
  - Dead players don't move: movement pauses while dead and auto-respawn (`respawn.rs`) handles both
    BDS (`Respawn` searching) and Geyser (no searching packet; dying is the trigger).
- Checking physics: record drills against BDS with `server-authoritative-movement-strict=true`,
  `player-position-acceptance-threshold=0.0001` and `player-rewind-min-correction-delay-ticks=0` (BDS
  then corrects on the first diverging tick, so each mismatch is a one-tick error; with the default
  delay it corrects every ~6 ticks and traces mislead), `BEDROCK_RECORD=<file>` on `examples/drills.rs`, then iterate offline with
  `examples/replay.rs <file>`: it replays the trace through `Movement` and lists corrections the
  simulation disagrees with. `examples/fuzz.rs` does the same with random terrain and controls
  (`FUZZ_OFF` bisects a mismatch to a feature). Drill pads float above spawn in peaceful mode: mobs,
  ocean water and fall deaths all show up as fake mismatches. Background on other trace sources:
  research/golden-traces.md.
- `CommandRequest.version` must be `"latest"`: BDS disconnects on anything else, although gophertunnel calls it unused.
- The Mojang chain endpoint rate-limits (HTTP 429). Reuse one client key per account (`Account::login_credentials`).
- Xbox can withhold tokens pending Yoti age verification; that surfaces as `auth::Error::ActionRequired { url }`.
- Re-check which transports target servers answer with `node docs/research/transport-probe.mjs <host>`.
