# @acacia-rs/bot benchmarks

Item 9 of `reports/Acacia native Node package design.md`: the real package on the release `acacia-node` addon.
The earlier numbers in `research/js-measure/` came from a synthetic TSFN crate and from Rust-only runs. Raw rows
are in `results/*.jsonl`, one JSON line per run (`TAG=` env tags a row). `node bench/summarize.mjs` prints the
median tables.

This file holds the current numbers. Superseded runs (the first runs, the batch-limit bug, the fixes' and the
entity deltas' before/after A/Bs) are in `HISTORY.md`.

## Method

- Ryzen 7 9700X, 16 threads, shared with other sessions. Load average at the runs was 10–122 (column `load`),
  so CPU and latency figures are pessimistic, and A/Bs are interleaved on the same box. The Rust and
  bedrock-protocol baselines ran at load ~2. Node v22.22.1.
- Addon: `npm run build` (release, thin LTO) for memory and events. `npm run build:bench` (release plus the
  `test-server` feature, which adds only the `TestServer` class) for reads and for the `READ=` event runs.
  `NativeTransport` defaults: 4 shards, snapshots on. Bots are `createBot` defaults: physics on, entities on,
  `transport: 'raknet'`.
- Server: sapling `load_server` (`/srv/agents/work/acacia/sapling-load`, release), a fresh one per run on
  localhost. Memory runs use a bare stone floor (`0 0`), like memory-results.md. Event runs use the `mixed`
  world from tools/swarm-load.sh: 225 redstone clocks and 300 strolling pigs. Every player sees every other.
- One Node process per run. Joins are spaced by `JOIN_MS` (500 since acacia-host spaces joins to one address
  0.5 s apart; 300 bots take ~170 s to join). Runs go through `bench/matrix.sh`, one at a time with reps
  interleaved, detached under sshd.
- `bench:memory`: standing bots, a 25 s settle after the last spawn, then `process.memoryUsage()` and
  `/proc/self/smaps_rollup` after `gc()`.
- `bench:events`: listeners on `physicsTick`, `entitySpawn`, `entityGone` and `entityMoved` for every bot; a 10 s
  settle, then a 20 s window. JS CPU is the main thread's utime+stime from `/proc/self/task`. Two probes run in
  the window, round-robin over the bots, 10 ms apart: `clearControlStates()` (immediate) and `waitForTicks(1)`
  (queued). `READ=hot|entities|positions` makes each `physicsTick` listener read `bot.hot`, `bot.entities`, or
  `bot.entities` plus every entity's `.position`.
- `bench:reads`: one physics bot against FakeServer (acacia-testserver's recorded BDS spawn), a 17×17 stone
  floor, 20 extra pigs (28–29 entities) and 21 stacks. ns/op is the median of 7 rounds of ≥ 200 ms, warm.
  Misses are timed one read at a time with a pig and a stack churning every 100 ms.

## Per-bot memory (`bench:memory`)

Values are MB, the median of 3 reps (1 rep for phys off, 2 for the variants).

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
  (memory-results.md, 2026-10-29). As there, part of the cost is per (bot, visible player) pair. The JS heap
  stays at 4–5 MB at every N, so the per-bot cost is all native.
- **Fixed cost:** Node is 57.8 MB. Loading the package and starting the host (4 shard threads and the forwarder)
  adds 10.8 MB. The first bot (registries, world) adds another ~11 MB, about what Rust's N = 1 costs.
- **The extra ~0.8 MB/bot over Rust:** partly glibc malloc arenas across the 4 shard threads. With
  `MALLOC_ARENA_MAX=2`, 200 bots took 557 MB (2.44 MB/bot over 50..200); with `SHARDS=1`, 555 MB. Published
  snapshots are ruled out: `NativeTransport({ snapshots: false })` at N = 100 (2 reps each, interleaved) gave
  339 / 323 MB on, 360 / 320 MB off. Defaults unchanged.
- Joins at 100 ms spacing (before the host's 0.5 s spacing): 200/200 spawned in every memory run.

Against the alternatives (MB RSS):

| N | @acacia-rs/bot | acacia Rust | bedrock-protocol, no world kept |
|---|---|---|---|
| 10 | 105 | 25 | 212 |
| 100 | 334 | 176 | 1573 |
| 200 | 634 | 401 | 2311 (needs 400 ms joins) |
| per bot | ~2.8 | ~2.0 | 11–15 |

mineflayer users report 100–400 MB per bot on Java (context only, with a full world). The pitch number for
`@acacia-rs/bot` is **about 3 MB per bot**: 4–5× under bedrock-protocol and ~35–140× under mineflayer's reports.

## Event delivery and calls (`bench:events`)

Calls, `READ=none` (median of 2 reps, load 10–27; idle = 1 bot, `CLOCKS=0 MOBS=0`):

| bots | immediate RTT p50 / p99 ms | waitForTicks(1) p50 / p99 ms | batches/s | JS thread |
|---|---|---|---|---|
| 1, idle | 0.022 / 0.06 | 51.3 / 51.8 | 117 | 0.7% |
| 10 | 0.18 / 2.1 | 51.4 / 54.5 | 258 | 4.7% |
| 100 | 0.44 / 10.3 | 51.7 / 65.6 | 401 | 9.4% |
| 300 | 1.41 / 30.9 | 54.0 / 113 | 311 | 16.1% |

- Throughput (first runs, not re-run since): 300 bots deliver ~11.3k events/s (`physicsTick` at 19.4/bot/s,
  113k entity moves/s in `entityMoved`) for ~17% of the JS thread, ~1.5 µs per moved entity. Loop delay p50
  0.06 ms. `lagged` was 0 in every run.
- A per-tick `bot.hot` read costs ~7 µs in aggregate (`READ=hot`, 300 bots: JS thread 17.3% → 21.6%).

Per-tick entity reads (load 77–122, 2 reps; JS thread % with churn, spawns + removals per second across the
bots, in parentheses):

| bots, read | JS thread % (churn/s) | loop p99 ms |
|---|---|---|
| 100, entities | 12.0 (315), 13.7 (199) | 5.8, 6.4 |
| 100, positions | 15.9 (165), 16.9 (155) | 7.1, 10.4 |
| 300, entities | 14.2 (1247), 18.6 (1757) | 17, 34 |
| 300, positions | 35.5 (1314), 40.9 (725) | 139, 144 |

- A read costs what changed since the last one (entity deltas, contract in the package README). `positions`
  builds a fresh `Vec3` for each of ~600 entities per bot per tick, ~3.6 M objects/s at 300 bots.

## Sync reads (`bench:reads`)

| read | ns/op |
|---|---|
| `bot.username` (plain property, baseline) | 2 |
| cached getters (`hot`, `entities`, `inventory`, `blockAt`) | 350–840 |
| `bot.nearestEntity()` (~28 entities, load ~100) | ~1,500 |
| `findBlocks` stone, r4, count 1 | 2,600 |
| `findBlocks` stone, r8, count 16 | 11,050 |
| `findBlocks` missing block, r16 | 1,650 |
| `bot.inventory` miss p50 | 76 µs |
| `bot.entities` miss p50 (a membership change, load ~100) | ~115 µs |
| `bot.players` miss p50 (load ~100) | ~132 µs |

- A cached hit is the napi crossing. Single-read misses overstate: much of each is this box's first-read-after-
  idle cost (~80 µs for `bot.hot` at load ~80, 2–3 µs right after). Take the `READ=` event runs as the real cost.

## Open

- No heaptrack attribution of the extra ~0.8 MB/bot; arenas are the lead.
- RakNet `BadSplit` losses: 3–4 across the 300-bot runs, and 7 join `DISCONNECTED` failures at load 50–66. Not
  investigated.
- The Rust (`memory_bench`, branch js/measure) and bedrock-protocol baselines are quoted from memory-results.md.

## Reproduce

```sh
npm run build                       # events, memory (npm run build:bench for reads and READ=)
npm run bench:memory -- 100         # one run, one JSON line on stdout; env in the script header
npm run bench:events -- 300        # READ=none|hot|entities|positions; TAG=<label> tags the row
npm run bench:reads
bash bench/matrix.sh memory 3       # the matrix, rows to results/<bench>.jsonl
node bench/summarize.mjs            # median tables
```

The server is `$SAPLING/target/release/examples/load_server`
(`cargo build --release -p sapling --example load_server`). Pass `ADDRESS=host:port` to use a running one.
