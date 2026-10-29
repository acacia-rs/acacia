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
| License | MIT; ported code keeps its notice (`acacia-physics/LICENSE-bedsim`) | Never port from GPL/LGPL sources (Pumpkin's NetherNet, PocketMine, PowerNukkitX); citing their behaviour is fine. One exception: `tools/lookbake` is GPL-3.0-or-later and holds what is ported from Pomme (`LICENSE-pomme`). It is a separate program that no crate depends on (docs/java-look.md "Licensing") |

## Crates (dependencies point downward only, no cycles)

```
acacia-client             tokio sockets and timers, SOCKS5 (UDP + CONNECT), NetherNet signaling I/O, public Client API
 ├─ acacia-session        network-free session over a RakNet or message link: handshake, ECDH + AES-CTR, compression, batching;
 │                        `server::ServerConnection` is the server side (FakeServer, mitm)
 │   ├─ acacia-raknet     network-free RakNet client and server: reliability, split packets, ACK/NACK, ordering
 │   └─ acacia-proto      generated packet structs + codec (varints, lazy Packet)
 │       └─ acacia-nbt    NBT value tree, network and little-endian flavours; benches, fuzz targets and a
 │                        corpus of real documents (its README)
 ├─ acacia-nethernet      network-free NetherNet: signaling HTTP bytes (both sides), signal text, signaling-service
 │                        session, STUN/TURN client, a=identity (both directions), fragment framing, str0m connection
 │                        (offer, or BDS-style answer, embedded or trickled), `Host`: many answering connections on one socket
 └─ acacia-auth           MSA device code → Xbox → XSTS → PlayFab → session/start → multiplayer token
acacia-viewer             (app) world viewer: a bot on a tokio thread, winit window, fly camera
 └─ acacia-render         wgpu terrain renderer over acacia-world (no network): pack loading, meshing, drawing
acacia-mitm               proxy library (record, drop, rewrite, inject) and recording CLI for vanilla-client
                          captures over RakNet or NetherNet direct connect, local BDS or (`--online`) real servers
                          and (`--realm`) realms; depends on acacia-client for the realm's route and raw link
acacia-anvil              Bedrock chunks (acacia-world) → Java 26.3 Anvil world (.mca) for BlueMap: generated state/biome
                          mapping from Geyser, neighbour and block-entity fixups, Java light, debounced region writes
acacia-molang             Molang compiler and evaluator, no dependencies; expected values come from BDS
                          (tools/molang-oracle). Used by acacia-render (entity looks and animations)
                          and acacia-bot (MoLang recipe ingredients)
tools/codegen             minecraft-data bedrock/<ver>/protocol.json → acacia-proto sources
tools/lookbake            bakes a look pack for acacia-render from a resource pack (docs/java-look.md)
tools/capdiff             compares what two clients sent in captures (vanilla vs bot): order, delays, cadence, fields
tools/nbtcorpus           extracts acacia-nbt's corpus from the recorded BDS join and the captured chunk fixtures
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
(`crates/acacia-mitm/src/intercept.rs`). Over RakNet a server's Transfer is followed, so the game stays on
the proxy (`crates/acacia-mitm/src/transfer.rs`). What is left: [mitm-next.md](mitm-next.md).

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
- RakNet server (`acacia-raknet/src/server/`) is built for untrusted peers: address cookies, `max_peers`, a
  handshake deadline and bans (`handshake.rs`), per-peer buffer limits (`RecvLimits`), a congestion window
  (`reliability/congestion.rs`) and scheduled instead of scanned peers (`schedule.rs`). The client shares the
  code but sends unwindowed: its pacing is part of the fingerprint. Still open: RakNet has no integrity, so a
  spoofed *connected* datagram can still end a session; pings are not rate limited; no per-IP peer limit.
- A bad link to test against: `cargo run -p acacia-raknet --example lossy_proxy -- <listen> <server>
  [--delay ms] [--jitter ms] [--loss %] [--up-loss %] [--down-loss %] [--seed n]` relays UDP, delaying,
  reordering and dropping datagrams each way; a client connects to it in place of the server.
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
- Geyser answers a client close with an echo plus a server close that must be acked before the next command,
  since it reuses window ids: see `interact/closing.rs`. Clicks in server-side GUIs often get no
  `ItemStackResponse` (`ActionError::Timeout`, sent anyway). Chest GUIs are titled by the fake block entity's
  `CustomName` (`Container::title`). `Bot::packet_trace` keeps the last 256 inbound packet ids for stall reports.
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
  - With mouse input the sprint key only starts a sprint: it lasts until forward is released (Movement
    latches it; strict BDS 1.26.52 keeps sprinting through our StopSprinting with forward held, and
    corrected every ~1.3 s, 2026-10-21). With touch input a key sprint ends with the key (fuzz,
    4805df7); only a double-tap one is latched. Diagonal keys are normalized to length 1
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
  - Parkour legs (2026-10-15, Linux BDS 1.26.52.3 on testbox, strict keys of docs/testing.md, bot op by
    `default-player-permission-level`; `examples/course.rs`, release). Corrections / ticks per leg:

    | run | gap3 | gap4 | up1 | down2 | diag | stones | sprint40 |
    |---|---|---|---|---|---|---|---|
    | Optimal (2 runs) | 0 / 36 | 0 / 46 | 0 / 28 | 0 / 22 | 0 / 60 | 0 / 52 | 0 / 161 |
    | Human seed 1 | 0 / 54 | 0 / 55 | 0 / 44 | 0 / 32 | 0 / 84 | 0 / 87 | 2 / 161 |
    | Human seed 2 | 0 / 54 | 0 / 53 | 0 / 44 | 0 / 36 | 0 / 87 | 0 / 80 | 9 / 157 |
    | Human seed 3 | 0 / 54 | 0 / 54 | 0 / 43 | 0 / 30 | 0 / 83 | 0 / 82 | 4 / 184 |
    | macros off, Optimal | 0 / 32 | 0 / 53 | 0 / 57 | 0 / 59 | 0 / 64 | 0 / 61 | 0 / 147 |
    | macros off, Human 1 | 0 / 32 | 0 / 52 | 0 / 57 | 1 / 73 | 1 / 70 | 0 / 58 | 4 / 143 |
    | fresh world, Optimal | 0 / 39 | 0 / 50 | 0 / 30 | 0 / 27 | 0 / 66 | 0 / 59 | 0 / 178 |
    | fresh world, Human 2 | 0 / 72 | 1 / 73 | 0 / 68 | 0 / 34 | 0 / 100 | 0 / 83 | timeout |
    | fresh world, fixed, Optimal | 0 / 37 | 0 / 57 | 0 / 30 | 0 / 25 | 0 / 67 | 0 / 79 | 0 / 162 |
    | fresh world, fixed, Human 1 | 1 / 56 | 4 / 71 | 0 / 45 | 0 / 34 | 27 / 82 | 40 / 88 | 51 / 161 |
    | fresh world, fixed, Human 2 | 0 / 52 | 1 / 72 | 0 / 54 | 0 / 39 | 22 / 83 | 39 / 80 | 38 / 152 |
    | fresh world, fixed, Human 3 | 1 / 54 | 15 / 68 | 25 / 54 | 10 / 38 | 0 / 82 | 0 / 82 | 0 / 157 |
    | BDS table + box fit, Optimal | 0 / 36 | 0 / 46 | 0 / 28 | 0 / 23 | 0 / 56 | 0 / 51 | 0 / 162 |
    | BDS table + box fit, Human 1 | 0 / 53 | 0 / 70 | 0 / 46 | 0 / 32 | 0 / 84 | 0 / 82 | 4 / 165 |
    | BDS table + box fit, Human 2 | 0 / 53 | 0 / 71 | 1 / 46 | 0 / 32 | 2 / 78 | 2 / 81 | 2 / 153 |
    | BDS table + box fit, Human 3 | 0 / 55 | 0 / 72 | 0 / 54 | 0 / 36 | 0 / 83 | 0 / 79 | 0 / 146 |

    On the fresh world Human seed 2 also drew 142 on classic legs (ladder 43, pool 64), seed 1 one in all.
    "Fixed" rows (2026-10-15, a fresh world per run): every leg ok; the Human corrections are the one-ulp bursts
    below, classic legs 0–39 per seed.

    Optimal draws no correction on any leg (classic legs included). Human draws a few on classic legs too
    (exit 14, slabs 8, pool 2 in single seeds) and with macros off, so they come from its aim, not macro moves.
    "BDS table + box fit" rows (2026-10-15, fresh world per run, test pack off): totals with classic legs Optimal
    0, Human 4 / 7 / 11 (Human 3: step 2, stair2 3, stairs 1, gap 1, ladder 4). The 500-block goto
    (`examples/goto.rs`, +500 x) drew Human 10 / 10 / 5 corrections in 1566 / 1551 / 1525 ticks (C1 run: 321).
    Before the wall guard of `correction.rs` (same code otherwise) the course drew Human 34 / 6 / 3, the goto
    11 / 7 / 15.
  - Off-axis ulp corrections on strict Linux BDS (2026-10-15):
    - Strict BDS corrects any position more than about 1e-6 off (1.07e-6 corrected, 9.5e-7 not): one f32 ulp at
      |x| ≥ 16 (7.6e-6 at 64). `ClientAcceptanceSystem` only moves its box towards differences of 0.05–0.1, so
      it plays no part.
    - The server's position every tick: a second bot sees the first's `MovePlayer` broadcasts, which carry the
      server's own f32 position (`examples/observe.rs`; a correction comes exactly when they drift 1e-6 from
      what we sent). Near the origin they differ by an ulp on a few percent of off-axis ticks without any
      correction, so the drift is found long before it shows.
    - Root cause 1, the sine table: Linux BDS fills it with glibc `sinf(i / 10430.378f)` (dumped from the live
      process, all 65536 entries; physics README). Ours was correctly rounded, two thirds of the entries an ulp
      off: every off-axis sprint jump moved by 1–8 ulps (the "jump tick" burst starts). With BDS's table the jump
      tick matches the broadcast exactly.
    - Root cause 2, the box after a correction: BDS never rebuilds its box, and rebuilding ours from the
      corrected centre is an ulp off at a face, which drew the ±1-ulp runs. `apply_small_correction` (physics
      README) keeps the faces consistent with our last 20 shared ticks, unless the box pushes against a wall.
      Replayed at tolerance 0 with both fixes, older Human runs' mismatching corrections drop to a third
      (`h-base-Hum1` 34 → 8, now `tests/traces/course-human-linux.btrc.gz`; `course-Hum2` 61 → 20) and old
      regression traces improve too (fuzz-a 1589 → 1286, fuzz-b 1701 → 1439, powder snow 29 → 19).
    - Boats too (2026-10-28): a boat's x and z are its kept box's centre. `pos += move` drew a 1-ulp x/z
      correction every few ticks, 641 on a 500-block leg, all under the 1e-5 bar below |x| 128; with the box, 47,
      all wave height (docs/research/riding-fishing-elytra.md "Boat"; live results docs/pathfind-plan.md "M3").
    - Root cause 3, walk acceleration (2026-10-28): BDS's move-relative (Windows `144832a50`, the ground move
      view's helper; Linux ELF 0x7315b8f, disassembled) scales the impulse by `speed / max(1, sqrt(len²))` first,
      then adds `x*cos - z*sin` and `x*sin + z*cos` with libm `sinf`/`cosf` (Linux imports glibc's). Ours was
      bedsim's `(sin*z)*f` with chewxy/math32 Cephes trig; diagonal keys (FRAC_1_SQRT_2) make the forward
      component inexact, so the order mattered on every diagonal tick. Each half alone helps (replayed at
      tolerance 0, fuzz-a 1286 → 1036 order only, 832 trig only); together course-human-linux 6 → 5,
      fuzz-a 1286 → 422, fuzz-b 1408 → 500, sneak-edge 221 → 175, powder snow 19 → 18, none higher. Live,
      moving ticks whose broadcast differs from our position (`observe`, each build judged on its own run):
      the 40-segment mix drill (diagonal keys, hops, mid-walk turns, near the origin) 250 of 1551 → 27 of 1552;
      8 forward sprint-hop segments at x 17.5 (fresh world each) 92 of 311 → 36 of 309, corrections 9 → 4;
      8 off-axis gotos of ~20 blocks per profile on flat ground, Optimal 19 → 13 and Human 26 → 13 corrections.
      Elytra flights (8 per profile, 800 x 600 out and back; the glide code is untouched): 0–3 a flight on both
      builds, plus one 44-correction glide spiral on the new one (HEAD 7 / 5, new 46 / 4 per profile).
    - Left open: what remains is almost all z, in forward sprint-hops: x stays exact while z drifts 1–2 ulps
      (goto legs across the pad drew corrections only on legs with a long z run). At the first miss our
      previous faces plus a z displacement one ulp lower land on the broadcast (live `observe` hop at yaw 10.66,
      the jump tick), but no single order of velocity + accel + sprint-jump boost explains it: jump boost before
      the accel, or summed with it, each fit 38 of 47 jump ticks against ours 37 and raised the live mismatches
      (35 → 41, 53). A box tracker (faces ±2 ulps, displacement ±40 of its own ulps, optionally rebuilt from the
      server's centre on jump, landing, takeoff or every tick) finds no consistent chain on most drifting
      segments, and `pos + move` from the server's previous position scores worse than our box. Replaying one
      build's live run with another build's physics gives rates that are not comparable (old run, new physics:
      391 of 1551); judge each build on its own run. course-human-linux tick 1104 (sprint jump, constant yaw)
      moves z 6.7e-7 off (3 ulps) with no trig variant explaining it. Not explained either: constant acceleration offsets,
      Java's operation order (BDS's `WaterMoveSystem` form), air speed 0.025999999, friction order,
      double-precision centres, position-derived velocity, or other centre formulas (`lo + 0.3`, `hi - 0.3`,
      `lo + span/2`: all worse than `(lo + hi) / 2`). Liquids and ladders with Human jitter draw most of what
      remains (course Human 1: ladder, pool, exit).
    - Offline bots rejoin where they left, so repeated drills creep up the pads: at y ~180 positions are
      4x coarser and Optimal drew 48 on the parkour legs; reset the world before a course run. The test pack
      (`tools/actiontest-pack`) builds its scene, with a water pool, under the player who joins 40 ticks later.
    - Tried, not kept: a Human aim holding an axis exactly once turned onto one it wants (run-to-run spread
      swamped it; seed 3 missed the gap3 leap at exactly -90°).
    - Human timeouts on a fresh world (sprint40, and diag/gap4 in later runs) were searches cut by the 2 s budget:
      the default rollout budget alone took about 2 s on the loaded testbox, and the partial path fell back on
      leapt or walked into a pit with no way out. Fixed in docs/pathfind-plan.md step 10.
    Traces: testbox `work/pf-traces` (`obs*` with broadcasts in `obs*.log`, `h-*`, `g-fit-*`).
  - Live divergence (`course --lift`: one second of levitation over the gap3 leap, Optimal): the leap
    diverged at x 51.5 in mid-air, the inline re-plan found no start node in the air (one counted re-plan),
    the next one planned 2 nodes and the leg finished (63 ticks, 0 stops). Its 2 corrections are the
    levitation's first tick, which BDS applied before the bot heard of the effect (replay: 1 mismatch).
- Dimension travel on BDS 1.26.52 (`bot/dimension.rs`; packets seen live with `examples/dimensions.rs`, the
  server's rules read from its binary; no vanilla client capture of a change exists yet):
  - Server: `ChangeDimension {dimension, position, respawn, loading_screen_id}`. From here its `PlayerAuthInput`
    handler drops every input silently: it queues one only while the player's dimension state is ready and no
    "server has movement authority" bit is set (teleport, dimension transfer, loading screen).
  - Client: `ServerboundLoadingScreen` type 1 with that id, which sets the loading screen bit. An id BDS did not
    announce is a kick (UnexpectedPacket), and the change that follows a death in another dimension carries
    none: no screen packets then.
  - Server, when its chunks are ready and without waiting for the client: its own
    `PlayerAction(DimensionChangeAck)`.
  - Client: `PlayerAction(DimensionChangeAck)`, which clears the transfer bit at once, then
    `ServerboundLoadingScreen` type 2; the screen bit clears on the server's next tick. Inputs count again.
    `SetLocalPlayerAsInitialized` is not read again.
  - `ChangeDimension.position` is the feet (`execute in nether run tp @s 30 70 30` carries y 70), unlike the
    eye positions of MovePlayer and corrections. Through a Nether portal it is only near (the portal block's
    centre, or the scaled source position while the far portal is made) and out of the End it is
    (0, 32767, 0); the real position follows in a MovePlayer or Respawn.
  - The bot stops simulating at the change and sends no input while it travels. With the server's
    acknowledgement and the sections around its feet in, it acknowledges, ends the screen, waits 10 ticks and
    simulates on from the server's position in the state BDS carries over: on the ground with one tick of
    gravity, so a player put in mid-air falls 0.0784 on the first input. An input sent in the server tick that
    ends the screen is dropped and leaves the server one tick behind (one correction): hence the wait.
    Inputs from the dimension left that BDS had not run yet are corrected to the new position after the
    acknowledgement; they reach a stopped simulation, and `replay` lists them as teleport lags.
  - Out of the End through a portal: BDS sends `ShowCredits` (0) and brings the player back
    (`ChangeDimension` with `respawn`) only after the client's `ShowCredits` (1); the bot answers at once.
  - Joining while in another dimension needs nothing more: `StartGame` names the dimension and position. A
    player who died there joins with `StartGame` in the overworld at the old position, then `Respawn` (ready).
  - Portals set by command (the drill): a Nether portal just used does nothing for some seconds (the drill
    waits 16 s outside it); an `end_portal` block takes a player standing in it but not one falling through,
    and has no collision.
  - BDS forgets an offline player between sessions (no xuid, empty `SelfSignedId`), so a join in another
    dimension needs an account, or a `SelfSignedId` in the client data (tried, not kept).
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
