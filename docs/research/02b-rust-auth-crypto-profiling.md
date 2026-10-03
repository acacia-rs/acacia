# 02b — Rust rewrite: auth, crypto, profiling (researched 2026-09-29)

## 1. Xbox Live / MSA auth for Bedrock in Rust

### Flow (as prismarine-auth does it for Switch/Android titles)
Source: [prismarine-auth XboxTokenManager.js](https://github.com/PrismarineJS/prismarine-auth/blob/master/src/TokenManagers/XboxTokenManager.js), [Constants.js](https://github.com/PrismarineJS/prismarine-auth/blob/master/src/common/Constants.js)

1. **MSA**: device-code (`login.live.com/oauth20_connect.srf` + `oauth20_token.srf`, scope `service::user.auth.xboxlive.com::MBI_SSL`, `client_id` = the auth title's live client id, e.g. Switch `00000000441cc96b`). Refresh via `oauth20_token.srf grant_type=refresh_token`. Email/password: no supported API — only scraping `login.live.com` forms (fragile, 2FA breaks it); treat as out of scope, device-code + cached refresh tokens only.
2. **Proof key**: generate one **P-256** ECDSA key per account (persist it with the cache); JWK goes in `ProofKey`.
3. **Device token**: `POST https://device.auth.xboxlive.com/device/authenticate`
   `{RelyingParty:"http://auth.xboxlive.com", TokenType:"JWT", Properties:{AuthMethod:"ProofOfPossession", Id:"{uuid}", DeviceType:"Nintendo"|"Android", SerialNumber:"{uuid}", Version:"0.0.0", ProofKey:jwk}}`
4. **Title token** (`title.auth.xboxlive.com/title/authenticate`, `RpsTicket:"t="+msa`, `DeviceToken`) and **user token** (`user.auth.xboxlive.com/user/authenticate`, `RpsTicket:"t="+msa` for live.com client ids, `"d="` for Azure ids) — or one-shot **SISU** `sisu.xboxlive.com/authorize` `{AccessToken:"t="+msa, AppId, DeviceToken, Sandbox:"RETAIL", UseModernGamertag:true, SiteName:"user.auth.xboxlive.com", RelyingParty, ProofKey}` which returns user+title+authorization tokens at once.
5. **XSTS**: `POST https://xsts.auth.xboxlive.com/xsts/authorize` `{RelyingParty, TokenType:"JWT", Properties:{UserTokens:[..], DeviceToken, TitleToken, SandboxId:"RETAIL", ProofKey}}`. Relying parties: `https://multiplayer.minecraft.net/` (legacy chain), `http://playfab.xboxlive.com/` (PlayFab), `http://xboxlive.com` (XSAPI/social/sessions).
6. **Request signing** (every device/title/sisu/xsts call): `Signature` header = base64(`i32be(1) ‖ u64be(filetime)` ‖ sig) where sig = ECDSA-P256-SHA256 (IEEE-P1363 r‖s) over
   `i32be(1) 0x00 u64be(filetime) 0x00 "POST"0x00 path+query 0x00 authorization 0x00 body 0x00`; filetime = (unix_s + 11644473600) * 10^7. Header `x-xbl-contract-version: 1` (2 for SISU/device).

### Minecraft side — **changed in 2025-26**
- **Legacy**: `POST https://multiplayer.minecraft.net/authentication` with `Authorization: XBL3.0 x=<uhs>;<xsts>` body `{"identityPublicKey": b64(PKIX P-384 pub)}` → 3-JWT chain (Mojang-signed ES384). gophertunnel sends `User-Agent: MCPE/Android`, `Client-Version: <game ver>`.
- **New (OIDC multiplayer token)**: XSTS(`http://playfab.xboxlive.com/`) → PlayFab `Client/LoginWithXbox` (title 20CA2) → session ticket → `POST https://authorization.franchise.minecraft-services.net/api/v1.0/session/start` (device+user config, PlayFab ticket) → **MCToken** (bearer for all `*.minecraft-services.net`) → multiplayer-token endpoint with client pubkey → JWT signed by the auth service carrying the client key in the **`cpk`** claim. Servers verify via the service's OpenID config (JWKS refreshed ~30 min). Endpoints discovered via `client.discovery.minecraft-services.net`.
- gophertunnel `login.EncodeOffline`: legacy self-signed chain "for **pre-1.26.10** servers"; otherwise OIDC token + dummy chain → **assume 1.26.10+ servers require the new token** ([gophertunnel login pkg](https://pkg.go.dev/github.com/sandertv/gophertunnel/minecraft/protocol/login), [service pkg](https://pkg.go.dev/github.com/sandertv/gophertunnel/minecraft/service), v1.62.0, 2026-09-16).
- prismarine side is mid-refactor: [prismarine-auth#184](https://github.com/PrismarineJS/prismarine-auth/pull/184), [#185](https://github.com/PrismarineJS/prismarine-auth/issues/185), [bedrock-protocol#811](https://github.com/PrismarineJS/bedrock-protocol/pull/811). MCToken/session-start details also in [bedrocktool authservice](https://pkg.go.dev/github.com/bedrock-tool/bedrocktool/utils/franchise/authservice).

### Rust crates
| Crate | Scope | Verdict |
|---|---|---|
| [xal](https://lib.rs/crates/xal) 0.1.3 (Dec 2024, [openxbox/xal-rs](https://github.com/openxbox/xal-rs)) | MSA device code, SISU, device/title/user token, XSTS, P-256 request signing (reqwest+p256) | **Best base.** Supply your own title params (Switch/Android). Low activity — vendor/fork it. |
| azalea-auth | MSA→XBL→Java `login_with_xbox` only | Java-only, confirmed; no device/title token, no Bedrock. Useful as reference for MSA cache. |
| [minecraft-auth](https://docs.rs/minecraft-auth), [minecraft-msa-auth](https://docs.rs/minecraft-msa-auth), [mcproto-oauth](https://docs.rs/mcproto-oauth), mc_auth | Java Edition services | Not Bedrock. |
| (none) | multiplayer.minecraft.net chain, PlayFab, session/start, OIDC multiplayer token | **Write it** (~500 LOC) porting gophertunnel `auth`+`service`. |

Reference impls, in priority order: **gophertunnel** (`minecraft/auth`, `minecraft/service`, `protocol/login` — tracks new versions fastest, typed Go ports cleanly to Rust), prismarine-auth (Switch title flow you already use), [SanctumTerra/auth](https://github.com/SanctumTerra/auth) (TS Bedrock auth).

Ops notes: cache MSA refresh token + device token + proof key per account (gophertunnel `XBLTokenCache` exists specifically to avoid device-token rate limits). Stagger token refresh across thousands of accounts; XSTS ~16h, MCToken/multiplayer tokens shorter — refresh on demand before connect.

## 2. Crypto / compression crates

### What Bedrock needs (per connection)
- **Login JWTs**: ES384 sign (client data JWT + self-signed identity JWT with `x5u` = b64 PKIX pubkey). One-off per connect → cost irrelevant.
- **Handshake**: server sends ES384 JWT with `x5u` server pubkey + `salt`. Shared secret = ECDH P-384 (x-coordinate, 48 B); key = `SHA256(salt ‖ secret)` (32 B); IV = `key[0..12] ‖ 00 00 00 02` (GCM-style J0+1).
- **Stream cipher**: AES-256-CTR, one continuous keystream per direction for the whole session (never reset per packet).
- **Checksum**: trailing 8 B = `SHA256(u64le(send_counter) ‖ plaintext ‖ key)[0..8]`, counter per direction. This SHA-256 per packet costs more than the AES — use SHA-NI (`sha2` with `asm`/cpufeatures auto-detect, or aws-lc-rs `digest`).
- **Compression**: since 1.20.60 each batch has a 1-byte algorithm id after 0xFE: `0x00` raw deflate (no zlib header), `0x01` snappy (raw, not framed), `0xFF` none; below `compression_threshold` servers send uncompressed. Most servers use zlib.

### Crate choices
| Need | Pick | Alternative | Notes |
|---|---|---|---|
| ES384 JWT | [jsonwebtoken](https://docs.rs/jsonwebtoken) 11.1 (Sep 2026), backend `aws_lc_rs` or `rust_crypto` | hand-roll with `p384`+`ecdsa` + base64 | Supports ES384, custom header (`x5u`), disable `exp` validation. Chain verification of Mojang/OIDC tokens also fine here. |
| ECDH P-384 | `p384::ecdh` (RustCrypto) | `aws_lc_rs::agreement::ECDH_P384` | Once per connect; pick whichever matches JWT backend to avoid two key types. |
| AES-256-CTR | `aes` + `ctr::Ctr32BE<Aes256>` (AES-NI/VAES autodetected via `cpufeatures`) | `aws_lc_rs::cipher` (`EncryptingKey::ctr`, streaming via `StreamingEncryptingKey`) | Ctr32BE matches the GCM counter layout; 128-bit wrap is unreachable (would need 64 GiB). Both do ≥3-5 GB/s/core; not a bottleneck. |
| SHA-256 | `sha2` (enable `asm` feature on x86 or rely on SHA-NI autodetect) | `aws_lc_rs::digest` | Verify inbound checksums optionally — skipping inbound verify is a legit AFK-bot saving. |
| Raw deflate | **[libdeflater](https://lib.rs/crates/libdeflater)** (whole-buffer API, fastest; batches are whole buffers) | `flate2` 1.1 with `zlib-rs` feature | [flate2](https://docs.rs/flate2) default is still miniz_oxide; docs say zlib-rs "typically outperforms all the C implementations" ([default-switch issue #469](https://github.com/rust-lang/flate2-rs/issues/469)). libdeflate needs the exact output size or a bound — use a growing retry buffer capped at the protocol max. Reuse `Decompressor`/`Compressor` per connection (allocation is the hidden cost). |
| Snappy | `snap::raw::{Decoder,Encoder}` | — | Raw format. |

**aws-lc-rs vs RustCrypto vs ring**: aws-lc-rs covers everything (ECDSA P-384, ECDH P-384, AES-CTR, SHA-256) with assembly and FIPS option, but needs cmake/NASM on Windows (prebuilt NASM objects ship for x86_64 now; still slower builds). ring has P-384 ECDH/ECDSA but **no raw AES-CTR API** → not sufficient alone. RustCrypto is pure Rust, builds anywhere, and its AES/SHA are hardware-accelerated; ECDSA P-384 is ~5-10x slower than aws-lc but runs only at login. **Recommendation**: RustCrypto (`p384`, `aes`, `ctr`, `sha2`) + `jsonwebtoken` with `rust_crypto` backend; swap AES/SHA to aws-lc-rs only if a benchmark shows it matters.

Throughput reality check: an AFK bot receives mostly chunk/entity traffic (~10-100 KB/s idle, MBs during chunk load). At 1000 bots × 50 KB/s = 50 MB/s, deflate decompress (~300-1000 MB/s/core) + SHA-256 (~1.5-2 GB/s with SHA-NI) + AES-CTR is well under one core. **Biggest win is not decompressing/decoding at all**: bots that don't need chunks can request a minimal chunk radius (`RequestChunkRadius` = small) and skip decoding every packet id they don't react to (see §4).

## 3. Profiling / benchmarking toolchain

### Tools by question
| Question | Windows dev | Linux prod/droplet |
|---|---|---|
| Where does CPU go? (sampling) | **[samply](https://github.com/mstange/samply)** (ETW via xperf, Firefox Profiler UI), or VS Profiler / Superluminal | **samply** or `perf record -g` + `cargo flamegraph` / [inferno](https://github.com/jonhoo/inferno) |
| Async task stalls, busy tasks, waker storms | [tokio-console](https://github.com/tokio-rs/console) (needs `tokio_unstable` + `console-subscriber`) | same, over TCP; don't leave on in prod (overhead) |
| Frame-level timeline of per-packet work | **[Tracy](https://github.com/wolfpld/tracy)** via [tracing-tracy](https://crates.io/crates/tracing-tracy) (spans → zones; ns resolution, remote connect) | same, connect over SSH tunnel |
| Heap churn / peak | [dhat-rs](https://docs.rs/dhat) (in-process, cross-platform, also heap *tests* asserting max allocs) | + heaptrack or [bytehound](https://github.com/koute/bytehound) for full-run analysis |
| Micro-bench (wall time) | [divan](https://github.com/nvzqz/divan) (simpler, alloc counting via `AllocProfiler`) or criterion (more stats, HTML) | same |
| Deterministic CI bench | — (Valgrind is Linux-only) | [gungraun](https://github.com/gungraun/gungraun) (renamed iai-callgrind): instruction counts, noise-free on shared CI |
| Regression tracking | | [CodSpeed](https://codspeed.io) (divan/criterion compat layers, instruction-simulation, free for OSS) or [bencher.dev](https://bencher.dev) (self-hostable, ingests divan/criterion/gungraun output, thresholds + PR comments) |

Always profile `--release` with `debug = "line-tables-only"` (or `debug = 1`) and `force-frame-pointers` (`RUSTFLAGS="-C force-frame-pointers=yes"`) in a `[profile.profiling]` inheriting release, so perf/samply stacks are cheap and complete.

### Allocators
- **mimalloc** (`mimalloc` crate): works on Windows + Linux, usually best for many small short-lived buffers; lowest-friction default.
- **jemalloc** (`tikv-jemallocator`): Linux-only in practice; better fragmentation behaviour for long uptimes and has `jemalloc_ctl` stats + heap profiling. Benchmark both on the real load; switch by cfg feature.
- Better still: avoid allocation in the hot path (pooled `BytesMut`, reused decompressor buffers) so the allocator choice stops mattering; assert with dhat heap tests.

### Load generation (never against real servers)
1. **Local real server**: [Dragonfly](https://github.com/df-mc/dragonfly) (Go, gophertunnel-based, cheap per player, easy to run 1000s of connections) or **BDS** (most accurate, heavier per player) with `online-mode=false` for offline load, and a dedicated test Xbox account or two for the auth path. PocketMine-MP also works but is single-threaded PHP → it becomes the bottleneck first.
2. **Fake server (primary loop tool)**: a small Rust RakNet+Bedrock server in the same workspace that completes login (offline or with self-signed keys), then streams **recorded packet traces** (capture real sessions once via a proxy like gophertunnel's or bedrocktool) at configurable rate × N connections. Deterministic, runs in CI, isolates client cost.
3. **Replay harness (bench unit)**: feed captured decrypted/decompressed batches straight into the decode pipeline without sockets → divan + gungraun benches.
Metrics to record per run: CPU-ms per bot per minute, RSS per bot, p99 tick handling latency, packets/s, syscalls/s (`perf stat -e syscalls:*` / `strace -c`).

### Recommended iterate-in-loop setup
1. `cargo bench` (divan) for codec micro-benches: raknet frame parse, decrypt+checksum, inflate, packet header decode, full batch pipeline on recorded data.
2. Same benches under gungraun in CI (Linux runner) → push to bencher.dev (or CodSpeed) with a threshold alarm (e.g. +3% instructions fails PR).
3. `tools/load.rs` binary: spins up fake server + N bots on loopback, prints a one-line summary (CPU/bot, RSS/bot, p99). Run at N = 100/1000/5000 on a droplet-sized VM; store results as JSON in `bench-results/` and compare to previous.
4. When a number regresses: samply (Windows) or `perf` (Linux) on the load binary → flamegraph; Tracy for latency outliers; tokio-console for task scheduling issues; dhat for allocation creep.
5. Before each release: 1h soak test with jemalloc stats/heaptrack to catch leaks and fragmentation.

## 4. Architecture for thousands of UDP clients

### Sockets: one per bot is forced
Servers key a RakNet session on the client's `ip:port`, so N bots against the same server need N distinct source ports → **one `UdpSocket` per bot, `connect()`ed to the server** (kernel does demux, `send`/`recv` without addresses, ICMP errors surface). A shared socket only helps for bots against *different* servers and adds userspace demux — not worth it. Budget: ~5000 bots → 5000 fds; raise `ulimit -n` / `LimitNOFILE`, and the ephemeral range (`net.ipv4.ip_local_port_range`) caps bots per source IP per server at ~28k. Set `SO_RCVBUF` modestly (e.g. 256 KB) — chunk bursts overflow the default and drop packets, which RakNet then resends (costly on both sides).

### Batching syscalls
- **recvmmsg/GSO/GRO** batch per socket; a client socket gets small bursts, so gains are modest except during chunk loading. [quinn-udp](https://docs.rs/quinn-udp/latest/quinn_udp/struct.UdpSocketState.html) gives GRO/GSO/`recvmmsg`/`sendmmsg` on a tokio socket with Windows fallbacks — use it on the receive path, measure before adopting.
- The real syscall cost is **one epoll wake + one recv per datagram per bot**. Cut it by: coalescing RakNet ACKs (send per tick, not per datagram), sending one datagram per game tick per bot (batch game packets into one 0xFE batch), and avoiding timers per bot (one shared tick wheel).
- **io_uring** ([monoio](https://github.com/bytedance/monoio), [compio](https://docs.rs/compio/latest/compio/) — compio also does IOCP on Windows) batches across *many* sockets in one submission — the thing recvmmsg can't do. Reports show io_uring latency degrading past ~100 conns/worker ([moq#3119](https://github.com/moq-dev/moq/issues/3119)), so treat it as a later experiment behind a transport trait, not the starting point.

### Tasks vs sharded loops
- **Start**: tokio multi-thread runtime, one task per bot owning socket + RakNet + session state (no `Arc<Mutex>` shared state; control plane talks via `mpsc`). Tokio tasks are ~a few hundred bytes; 10k tasks is fine.
- **Scale step**: **sharded current-thread runtimes** — K threads (= cores), each a `current_thread` runtime owning N/K bots, bots pinned to a shard. Removes work-stealing cross-core cache traffic and lets per-shard buffer pools / decompressors be non-`Sync`. This is the standard shape for many-connection clients ([Iggy thread-per-core writeup](https://iggy.apache.org/blogs/2026/02/27/thread-per-core-io_uring/)). Keep the connection logic runtime-agnostic (sans-IO state machine: `fn on_datagram(&mut self, &[u8], now) -> Actions`) so both layouts — and the fake server and replay benches — drive the same code.

### Decode less
- Sans-IO pipeline: RakNet frame → reassemble → decrypt → checksum → inflate → split batch → **read varint packet id → dispatch only on ids the bot handles** (keepalive/NetworkStackLatency, PlayStatus, StartGame, Respawn, SetHealth/death, Disconnect, Transfer, text if logging chat, MovePlayer for self). Everything else: skip by length, never parse.
- Shrink inbound volume at source: small `RequestChunkRadius`, don't send `ClientCacheStatus` enabled unless needed, don't subscribe to things you ignore. Less data beats faster decode.
- Zero-copy with `bytes::Bytes`/`BytesMut`: slice sub-packets out of the decompressed batch without copying; per-bot reusable `BytesMut` for decrypt/inflate output; pooled send buffers.
- Decrypt/checksum/inflate must run even for skipped packets (stream cipher + whole-batch compression), so those are the irreducible per-byte costs — the §2 choices matter only there.
- Protocol definitions: generate packet structs from [PrismarineJS minecraft-data](https://github.com/PrismarineJS/minecraft-data) protocol JSON or port gophertunnel's `packet` package; only implement the handled ids initially. Existing Rust Bedrock stacks ([bedrock-rs](https://github.com/Lompandi/bedrock-rs), [rak-rs](https://docs.rs/rak-rs) 0.3.3 ~2y stale, [raknet-rs](https://lib.rs/crates/raknet-rs) client "basic handshake only") are references, not dependencies — write RakNet in-house (client side is small: open-connection req 1/2, reliability layer, split packets, ACK/NACK).

## Open items / verify during implementation
- Exact multiplayer-token endpoint path + request body (read gophertunnel `minecraft/service` source; GitHub raw fetches failed during this research).
- Switch/Android client IDs, title IDs, `x-xbl-contract-version` values: copy from prismarine-auth `Constants.js` / gophertunnel `auth.Config` rather than from this doc.
- Whether target servers (1.26.10+) still accept the legacy chain — test against current BDS.

