# @acacia/bot benchmarks (2026-11-03)

Item 9 of `reports/Acacia native Node package design.md`. These runs use the real package and the release
`acacia-node` addon. The earlier numbers in `research/js-measure/` came from a synthetic TSFN crate and from
Rust-only runs. Raw rows are in `results/*.jsonl`, one JSON line per run. `node bench/summarize.mjs` prints the
median tables quoted below.

## Headlines

- **Memory: ~2.8 MB per standing bot on ~69 MB fixed.** 200 crowded bots took 634 MB RSS. The JS heap stays at
  4–5 MB at every N, so the per-bot cost is all native. Rust without Node, from memory-results.md: ~2.0 MB/bot
  on ~14 MB. bedrock-protocol: 11–15 MB per client on ~80 MB, before it keeps any world.
- **Events at 300 bots: 11.3k events/s to JS listeners for 17% of the JS thread.** The listeners get
  `physicsTick` at 19.4/bot/s, `entityMoved` carrying 113k entity moves/s, and spawn/gone. Event-loop delay is
  p50 0.06 ms, p99 3.9 ms. Nothing was `lagged`.
- **Call round trip under that load: p50 10.7 ms for an immediate call.** At 10 bots it is already 4 ms. That
  floor comes from the shards' 10 ms flush (see Bottlenecks).
- **Getters, cached: 0.5–1.4 µs.** Every one crosses napi. A per-tick `bot.hot` costs ~5–7 µs per read in
  aggregate. Reading `bot.entities` per tick in a busy world does not scale: about 350 µs per read with ~400
  entities.
- **Bug found and fixed:** past ~100 bots in a crowd, bots dropped with `decompressed batch exceeds limit`.
  This invalidated the first 200/300-bot runs. Fix: `acacia-host` sets a 128 MiB limit (Bugs, 1).

## Setup

- Ryzen 7 9700X, 16 threads, shared with other sessions. Load average at the runs was 26–95 (column `load`;
  usually 40–60). The Rust and bedrock-protocol baselines ran at load ~2, so CPU and latency figures here are
  pessimistic. Node v22.22.1.
- Addon: `npm run build` (release, thin LTO) for memory and events. `npm run build:bench` (release plus the
  `test-server` feature, which adds only the `TestServer` class) for reads and for the `READ=` event runs.
  `NativeTransport` defaults: 4 shards, 10 ms flush, snapshots on. Bots are `createBot` defaults: physics on,
  entities on, `transport: 'raknet'`.
- Server: sapling `load_server` (`/srv/agents/work/acacia/sapling-load`, release), a fresh one per run on
  localhost. Memory runs use a bare stone floor (`0 0`), like memory-results.md. Event runs use the `mixed`
  world from tools/swarm-load.sh: 225 redstone clocks and 300 strolling pigs. Every player sees every other.
- One Node process per run. Joins are spaced 100 ms apart. Runs go through `bench/matrix.sh`, one at a time
  with reps interleaved, detached under sshd.

## Per-bot memory (`bench:memory`)

Standing bots, a 25 s settle after the last spawn, then `process.memoryUsage()` and `/proc/self/smaps_rollup`
(after `gc()`). Values are MB, the median of 3 reps (1 rep for phys off, 2 for the variants).

| N | rss | Anonymous | heapUsed | VmHWM (peak) | per bot above loaded | Rust phys on, shared (memory-results.md) |
|---|---|---|---|---|---|---|
| Node only (script, before import) | 57.8 | 14.1 | 4.3 | – | – | – |
| 0 (package loaded, host started) | 69.1 | 16.2 | 4.2 | 69.7 | – | – |
| 1 | 79.6 | 20.0 | 4.4 | 82.1 | 10.5 | 15.4 |
| 10 | 105.2 | 44.2 | 4.5 | 115.7 | 3.66 | 25.1 |
| 50 | 226.1 | 164.0 | 4.6 | 249.0 | 3.15 | 91.3 |
| 100 | 333.5 | 270.8 | 4.6 | 439.5 | 2.65 | 175.8 |
| 200 | 634.2 | 571.5 | 4.8 | 902.5 | 2.83 | 400.7 |

- **Slope over N = 10..200: 2.78 MB/bot** (physics off: 2.63). Rust's slope on the same scenario was 1.98
  (memory-results.md, 2026-10-29). As there, part of the cost is per (bot, visible player) pair.
- **Fixed cost:** Node is 57.8 MB. Loading the package and starting the host (4 shard threads and the forwarder)
  adds 10.8 MB. The first bot (registries, world) adds another ~11 MB, about what Rust's N = 1 costs.
- **What the extra ~0.8 MB/bot over Rust is:** partly glibc malloc arenas across the 4 shard threads. With
  `MALLOC_ARENA_MAX=2`, 200 bots took 557 MB (2.44 MB/bot over 50..200). With `SHARDS=1` they took 555 MB. The
  rest is untested. Candidates: the host's per-tick published snapshots (each bot's entity list holds every
  visible player, which is an N² term), the per-bot hot JSON cache in `NativeHost`, and baseline drift (the
  Rust numbers are from another day and moved ±20% run to run).
- **Joins at 100 ms spacing:** 200/200 spawned in every memory run (8 runs at N = 200), in ~28 s. In the
  300-bot event runs, 2093 of 2100 joins succeeded. The 7 failures were `DISCONNECTED` during login, at load
  50–66. bedrock-protocol got 112/200 at the same spacing and needed 400 ms.

Against the alternatives (MB RSS):

| N | @acacia/bot | acacia Rust | bedrock-protocol, no world kept |
|---|---|---|---|
| 10 | 105 | 25 | 212 |
| 100 | 334 | 176 | 1573 |
| 200 | 634 | 401 | 2311 (needs 400 ms joins) |
| per bot | ~2.8 | ~2.0 | 11–15 |

mineflayer users report 100–400 MB per bot on Java (context only, with a full world). The pitch number for
`@acacia/bot` is **about 3 MB per bot**: 4–5× under bedrock-protocol and ~35–140× under mineflayer's reports.

## Event delivery end to end (`bench:events`)

Listeners on `physicsTick`, `entitySpawn`, `entityGone` and `entityMoved` for every bot. A 10 s settle after
the joins, then a 20 s window. JS CPU is the main thread's utime+stime from `/proc/self/task` (100 Hz ticks).
Two probes run during the window, sequential and round-robin over the bots, 10 ms apart:
`clearControlStates()` (immediate lane) and `waitForTicks(1)` (queued). Values are medians over reps
(5 at 300 bots, 4 at 100, 3 at 10).

| bots | events/s | ticks/bot/s | entity moves/s | batches/s | records/batch | JS thread | process | loop delay p50 / p99 ms | immediate RTT p50 / p99 ms | waitForTicks(1) p50 / p99 ms |
|---|---|---|---|---|---|---|---|---|---|---|
| 10 | 389 | 19.46 | 4.7k | 224 | 2.1 | 2.2% | 7.8% | 0.05 / 1.0 | 4.1 / 12.3 | 55.9 / 62.4 |
| 100 | 3.9k | 19.48 | 44.6k | 328 | 12.3 | 8.7% | 64% | 0.06 / 2.6 | 8.4 / 20.9 | 57.9 / 73.5 |
| 300 | 11.3k | 19.41 | 113k | 246 | 45 | 17.3% | 245% | 0.06 / 3.9 | 10.7 / 65.9 | 62.6 / 137 |

- At 300 bots the shard threads use ~2.3 cores and the forwarder 0.3%. The JS thread is far from the
  bottleneck. `lagged` was 0 in every run.
- That is ~15 µs of JS thread per event: events are fat here. An `entityMoved` lists ~24 moving pigs, so it
  works out to ~1.5 µs per moved entity, decode plus dispatch. The synthetic bench measured ~0.45 µs for a
  169-byte event. Per-tick events are as affordable as tsfn-results.md predicted. The cost tracks entity churn.
- **One snapshot per tick** (`READ=hot`, each `physicsTick` listener reads `bot.hot`): at 300 bots the JS
  thread went from 17.3% to 21.6% (2 reps, about 5.8k reads/s, so ~7 µs per read). At 100 bots it was within
  noise. **Per-tick `bot.entities`** (`READ=entities`) at 100 bots: the JS thread went from 8% to 77% and loop
  p99 to 76 ms, which is ~350 µs per read with ~400 tracked entities. At 300 bots the JS thread saturated and
  the run never finished (killed at 600 s).

## Sync reads (`bench:reads`)

One physics bot against FakeServer (acacia-testserver's recorded BDS spawn) with a 17×17 stone floor,
20 extra pigs (28–29 entities, 4.2 KB entity-list JSON) and 21 stacks (1.8 KB cold JSON). The loop runner
reports ns/op as the median of 7 rounds of ≥ 200 ms, warm, mostly cache hits (4 ticks per round). The table
shows the median over 3 runs.

| read | ns/op |
|---|---|
| `bot.username` (plain property, baseline) | 2 |
| `bot.hot` / `bot.entity.position` / `bot.health` | 690 / 680 / 690 |
| `bot.entities` (28 entities) | 535 |
| `bot.nearestEntity()` | 1360 |
| `bot.players` | 1085 |
| `bot.inventory` (21 stacks) / `bot.heldItem` | 545 / 1335 |
| `bot.blockAt` | 1280 |
| `findBlocks` stone, r4, count 1 | 38,600 |
| `findBlocks` stone, r8, count 16 | 51,700 |
| `findBlocks` missing block, r16 | 2,600 |

- A cached hit is ~0.5–0.7 µs, and every bit of it is the napi crossing. `bot.hot` copies the cached JSON
  string into a fresh JS string on every call and compares it. The versioned reads make one `*Version` call.
- **Misses**, timed one read at a time (`sampled` in the rows) with an extra pig and one stack churning every
  100 ms, p50: `bot.hot` ~40 µs, `bot.entities` ~140 µs, `bot.players` ~110 µs, `bot.inventory` ~220 µs. For
  `bot.hot` that overstates: the native part measured alone is ~9 µs and `JSON.parse` of the 346 B string is
  1.7 µs warm, and the aggregate per-tick figure above is ~7 µs. Take the single-read figures as upper bounds
  and the `READ=` event runs as the real cost.

## Bugs and bottlenecks

1. **Fixed: bots dropped in crowds.** The client refuses a decompressed batch over 32 MiB
   (`crates/acacia-session/src/batch.rs:10`). A newcomer on a crowded server gets every player's skin at once,
   which is more than that. The Rust baselines set 128 MiB (`crates/acacia-bot/examples/memory_bench.rs:17`),
   but acacia-host left the default and offered no knob. Before the fix, 179 of 300 event-bench bots and ~89
   of 200 memory-bench bots ended with `lost: Protocol("decompressed batch exceeds limit")` from ~20 s in. Their
   calls then got `UNKNOWN_BOT` (rows in `results/before-batch-limit/`). The fix is
   `HostConfig::max_batch_bytes`, default 128 MiB (`crates/acacia-host/src/host.rs:33`), passed to the client
   builder (`crates/acacia-host/src/bot/mod.rs:46`). It is not exposed to JS.
2. **Call latency floor = shard flush.** A `done` record waits for its shard's next flush
   (`HostConfig::flush_every`, 10 ms, `crates/acacia-host/src/host.rs:41`; `outbound.rs:76`), so even an
   immediate call costs ~4 ms at idle. A possible fix: flush at once when a batch holds a `done` record.
3. **`bot.entities` per tick is O(entities) per read.** Any entity moving bumps the version, and then the
   whole list is serialized in Rust, parsed in JS and re-indexed (`src/native.ts` `#entities`, `src/reads.ts`
   `#byId`). With moving mobs that happens every tick. Deltas, or a per-entity read, would be needed before
   advertising per-tick entity reads.
4. **`findBlocks` scans whole sections.** `crates/acacia-world/src/find.rs:35` visits all 4096 cells of every
   section that overlaps the radius and holds a match, instead of the radius's bounding box, so a 4-block
   search costs ~39 µs. Clamping the loop to the box would cut r4 to ~730 cells.
5. **Inventory miss ~220 µs**, likely `src/reads.ts:134`: `inventory()` rebuilds a `Map` of the whole item
   table on every cold change. Caching the map per table would remove it (not verified separately).
6. Not investigated: 3 RakNet losses (`lost: Transport(Protocol("BadSplit"))`) across ~2100 bot-runs at
   300 bots, and the 7 join `DISCONNECTED` failures, all at load 50–66.

## Not done

- No feature-gated synthetic emitter: `research/js-measure/tsfn-results.md` already isolates the TSFN path,
  and these runs show the transport isn't the bottleneck.
- The Rust (`memory_bench`, branch js/measure) and bedrock-protocol baselines were not re-run today. Their
  figures are quoted from memory-results.md.
- No heaptrack attribution of the extra ~0.8 MB/bot, and no snapshots-off A/B (`HostConfig::snapshots` is not
  reachable from JS).

## Reproduce

```sh
npm run build                       # events, memory (npm run build:bench for reads and READ=)
npm run bench:memory -- 100         # one run, one JSON line on stdout; env in the script header
npm run bench:events -- 300
npm run bench:reads
bash bench/matrix.sh memory 3       # the matrix, rows to results/<bench>.jsonl
node bench/summarize.mjs            # median tables
```

The server is `$SAPLING/target/release/examples/load_server`
(`cargo build --release -p sapling --example load_server`). Pass `ADDRESS=host:port` to use a running one.
