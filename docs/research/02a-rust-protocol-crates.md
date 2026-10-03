# 02a — Rust Bedrock protocol / RakNet crates (researched 2026-09-29)

Scope: headless authenticated Bedrock CLIENT fleet (~20 packet types, one pinned protocol) in Rust.
Stats from GitHub API / crates.io API on 2026-09-29 unless marked.

## 0. Headline: the transport is changing under us (read first)
- **26.50 (protocol 2193)**: BDS `transport=` default switched `raknet` -> `nethernet`; `transport=raknet` still works.
- **26.60 (scheduled 2026-10-27)**: Aternos, gameserverkings and other secondary sources say RakNet is
  **removed from both client and BDS**; the "try NetherNet" warning becomes an error.
  UNVERIFIED against Mojang's own notes (feedback.minecraft.net and minecraft.wiki blocked this session) —
  check the 26.60.21 / 26.60.22-23 preview notes before committing to anything.
- NetherNet direct connect = HTTP signaling on TCP `<server-port>` (`GET /v1/join` status/probe,
  `POST /v1/join/{networkId}` with SDP offer), then WebRTC (ICE/DTLS/SCTP) over UDP. Two data channels
  (`ReliableDataChannel`, `UnreliableDataChannel`); packets are sent individually (no batch), >10 kB are segmented.
  LAN discovery on UDP 7551 (AES-ECB + HMAC). Xbox friends/Realms signal via `wss://signal.franchise.minecraft-services.net`.
- Ecosystem is already moving:
  - PrismarineJS bedrock-protocol PR #828 (merged 2026-09-22, v3.60.1) picks RakNet or NetherNet from pongs.
    HTTP signaling for direct servers is still open as issue #830, so **the current Node stack will hit the same wall**.
  - WaterdogPE proxy serves RakNet (old clients) and NetherNet (new clients) side by side.
  - gophertunnel ships `minecraft/nethernet.go` + `raknet.go`, backed by df-mc/go-nethernet (pushed 2026-09-28).
  - Pumpkin (Rust, GPL-3.0) has in-tree server-side NetherNet (`crates/pumpkin/src/net/bedrock/nethernet/*`,
    uses the `webrtc` crate). There's **no standalone Rust NetherNet crate**, and no Rust NetherNet client.
  - rust-raknet 0.14 README mentions a "NetherNet proxy mode" (details not checked).
- Third-party servers (featured servers, Geyser, PMMP, Nukkit) keep RakNet only while the clients they must
  accept still speak it. After 26.60, expect them to add NetherNet listeners (as Waterdog did) and eventually
  drop RakNet. **A bot that speaks only RakNet has a shelf life measured in months.**

## 1. Rust Bedrock protocol projects
- **bedrock-rs** (bedrock-crustaceans/bedrock-rs): pushed 2026-09-22, 187 stars, Apache-2.0, ~1.7k commits,
  **not on crates.io** (git dependency). Protocol crate has features `protocol-v662..v975, v1001, v2168,
  v2169, v2193` (up to 26.50), each written as a diff over the previous version, plus derive macros
  (`bedrock_macros`, `bedrock_protocol_core`). The network crate has zlib/snappy, AES-CTR encryption, p384
  ECDH (RC crypto deps), and `raknet-tokio` from bedrock-crustaceans/raknet.
  - Client mode: the README says "server-side and client-side", but the network crate only exposes
    `listener`, there's no dialer, and the only example is `server.rs`. The auth crate *validates* JWT chains;
    no sign-in flow was found (no MSA device-code, no Xbox token, no client chain signing).
  - Issue #258 (2026-09-06): the server example isn't reachable from a 26.45 client. It was closed as a
    duplicate of #257.
  - Verdict: useful for packet structs and codegen ideas, not a working client.
- crates.io search "bedrock": nothing at protocol level. There's acacia-world, bedrock-leveldb,
  bedrock-jwt 0.1.0 (verify only), bedrock-hematite-nbt, and ping tools (mcget, mcpe_motd, rsquery).
- Pumpkin: full Rust server with Bedrock (NetherNet present; no file named *raknet* in the tree, RakNet status not checked). Server-only, GPL-3.0.

## 2. Rust RakNet crates
| crate / repo | ver, date | stars / dl | license | client? | notes |
|---|---|---|---|---|---|
| [rust-raknet](https://github.com/b23r0/rust-raknet) | 0.14.2, 2026-09-29 | 225★ / 24k | MIT | yes (RaknetSocket::connect) | Revived after 2022 (all older versions yanked). Reliable, ordered and sequenced modes; fast and selective retransmit; tokio. Self-described as an "incomplete implementation by reverse engineering". Own benchmark: 14 MiB/s at 0% loss |
| [bedrock-crustaceans/raknet](https://github.com/bedrock-crustaceans/raknet) | git only, pushed 2026-09-24 | 7★ | Apache-2.0 | yes ("client, server, session") | **Sans-IO** core plus `raknet-tokio` and `bevy-raknet` wrappers. Created 2026-03. Used by bedrock-rs |
| [raknet-rust](https://github.com/mcbe-rs/raknet-rust) | 0.2.0, 2026-03-01 | 12★ / 77 | Apache-2.0 | yes (RaknetClient) | 19K LoC, tokio, tracing and Prometheus telemetry, split into high-level and `low_level::{protocol,session,transport}` APIs. No pushes since March |
| [rak-rs](https://github.com/NetrexMC/RakNet) | 0.3.3, 2024-04-28 | 55★ / 10.6k | Apache-2.0 | yes | Stale (last push 2024-07) |
| [raknet-rs](https://github.com/MemoriesOfTime/raknet-rs) | 0.1.4, 2024-10-15 | 13★ / 5.7k | Apache-2.0 | "most basic handshake, not recommended" | Nightly-only. Stream/Sink API, ACK/NACK, multiple order channels. Stale (2025-03) |
| [ruknet](https://crates.io/crates/ruknet) | 0.2.0, 2025-12-16 | – / 2.2k | MIT | yes | ~3K LoC, pluggable congestion control (UDT or sliding window) |
| [tokio-raknet](https://crates.io/crates/tokio-raknet) | 0.2.0, 2025-12-03 | – / 168 | ? | ? | Repo 404s now (jvigu) |

- Nobody publishes per-connection memory figures, or benchmarks for thousands of client sessions in one process.
  MTU discovery and split reassembly exist in all the "yes" crates per their docs, but none were audited here.
- For "many clients per process", a **sans-IO core** (bedrock-crustaceans/raknet) is the right shape: you can
  multiplex N sessions over a few UDP sockets on your own timer wheel instead of running one tokio task and
  socket per connection.

## 3. Reference implementations to port from
| ref | status | best for |
|---|---|---|
| [gophertunnel](https://github.com/Sandertv/gophertunnel) (Go) | pushed 2026-09-28, 571★, MIT. Protocol 2193 / 26.50 (PR #520 merged 2026-09-16); one version at a time | **Client semantics end to end**: Dialer, MSA/Xbox auth, login chain, encryption handshake, RakNet and NetherNet (go-nethernet), Realms. The best hand-port reference |
| [PrismarineJS/minecraft-data](https://github.com/PrismarineJS/minecraft-data) `data/bedrock/<ver>/protocol.json` | pushed 2026-09-22, 944★ | **Codegen input**: machine-readable ProtoDef for every version, including 26.x. Your current stack already runs on it, so behaviour matches 1:1 |
| [PrismarineJS/bedrock-protocol](https://github.com/PrismarineJS/bedrock-protocol) | pushed 2026-09-22, 480★, MIT | The auth and login flow you already run. NetherNet discovery done; HTTP signaling pending (#830) |
| [CloudburstMC/Protocol](https://github.com/CloudburstMC/Protocol) (Java) | pushed 2026-09-29, 417★, Apache-2.0 | Per-version codec helpers, multi-version server/proxy use (Geyser, Waterdog) |
| [Mojang/bedrock-protocol-docs](https://github.com/Mojang/bedrock-protocol-docs) | pushed 2026-09-25, 682★ | Official field docs and diffs. Good for cross-checking; less convenient than ProtoDef for codegen |
| [df-mc/go-nethernet](https://github.com/df-mc/go-nethernet) / [nethernet-spec](https://github.com/df-mc/nethernet-spec) | 2026-09-28, 26★ MIT / spec 2024-03 (stale) | NetherNet client and signaling reference |

Codegen pick: minecraft-data `protocol.json` for the one pinned version. Emit only the ~20 packets you use,
plus a skip-by-length path for everything else (Bedrock batches are length-prefixed per packet, so unknown IDs are cheap to drop).
Use gophertunnel to check tricky types (varint edge cases, item stacks, StartGame).

## 4. Recommendation (judgment)
1. **Don't start a RakNet-only Rust rewrite now.** Confirm the 26.60 removal and how target servers plan to
   handle it first. If they follow Waterdog (dual stack), RakNet buys time. If not, the client must speak NetherNet.
2. If going ahead: write your own thin client stack, and use crates for the pieces:
   - Transport: bedrock-crustaceans/raknet (sans-IO, Apache, active) behind a `Transport` trait, so NetherNet can slot in.
     Fallback: rust-raknet 0.14 (most downloads, but the revival is days old).
   - NetherNet: HTTP signaling (small), plus a WebRTC client. `webrtc` crate (Pumpkin uses it) works but is heavy per peer.
     A sans-IO stack such as str0m fits a many-sessions design better (not evaluated here).
   - Codec: generate from minecraft-data for one version. Borrow ideas, not a dependency, from bedrock-rs macros.
   - Auth and login: port from gophertunnel or bedrock-protocol (MSA device code -> XBL -> XSTS -> Minecraft
     multiplayer token, ES384 chain, ECDH/AES-CTR). No Rust crate does the client side.
3. **Gaps**: no Rust Bedrock client exists, no Rust NetherNet client, no Rust MSA/Xbox login-chain crate, no
   multi-session RakNet benchmarks, and the 26.60 transport change isn't confirmed from primary sources.

## Links
- NetherNet: [playit.gg note](https://playit.gg/support/minecraft-acacia-nethernet/) · [Aternos](https://support.aternos.org/hc/en-us/articles/39155890785053-NetherNet-protocol-Minecraft-Bedrock-Edition) · [Waterdog config](https://docs.waterdog.dev/waterdogpe-setup/nethernet-configuration) · [bedrock-protocol #828](https://github.com/PrismarineJS/bedrock-protocol/pull/828) · [Pumpkin bedrock docs](https://docs.pumpkinmc.org/config/bedrock)
- 26.60: [minecraft.wiki 26.60 dev versions](https://minecraft.wiki/w/Bedrock_Edition_26.60/Development_versions) · [Preview 26.60.21 notes](https://feedback.minecraft.net/hc/en-us/articles/48602411551629-Minecraft-Beta-Preview-26-60-21)
- gophertunnel 26.50: [PR #520](https://github.com/Sandertv/gophertunnel/pull/520) · bedrock-rs [issue #258](https://github.com/bedrock-crustaceans/bedrock-rs/issues/258)
