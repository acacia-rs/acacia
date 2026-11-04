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

Steady state with a per-tick reader (`READ=entities`: each `physicsTick` reads `bot.hot` and `bot.entities`),
300 bots, after the two fixes below (2026-11-04, 3 reps, load 43–49): ~10.9k events/s (`physicsTick` 19.4/bot/s,
94–105k entity moves/s in `entityMoved`), JS thread 26–28%, shard threads 173–180%, forwarder 0.3%, immediate
RTT p50 2.6–3.0 ms, loop delay p99 ~6 ms, `lagged` 0. Per bot that is ~0.59% of a core on the shards and
~0.09% on the JS thread. A read costs what changed since the last one (entity deltas, contract in the package
README). Earlier per-tick read tables (2026-11-03, before later commits; today's base gave a JS thread of
27–29% where they show 14–19%) are in `HISTORY.md`.

## Profile (2026-11-04)

`bench:events` with ticks, entities and `READ=entities`, the release code with line tables (`--profile
profiling`, `CARGO_PROFILE_PROFILING_DEBUG=line-tables-only`). Native: `perf record -F 499 -g -p <node>` over 15 s
of the window; JS: `CPU_PROF=<file>` (the inspector's profiler over the window). Self time; the release build
has no frame pointers, so perf's call chains (fp and DWARF alike) don't unwind and inclusive figures are JS-only.

CPU split (% of one core, base = js/cancel before the fixes, load 41–58):

| bots | shard threads (4) | JS thread | forwarder | sapling server |
|---|---|---|---|---|
| 100 | 60.7 | 14.4 | 0.5 | 39 |
| 300 | 199–202 | 27–29 | 0.4 | 70 |

Shard threads, 300 bots, top self frames (% of shard samples), before → after fix 1:

| frame | before | after |
|---|---|---|
| `RandomState::hash_one<&str>` + `sip::Hasher::write` (snapshot interner) | 27.2 | 2.3 |
| `ListBuilder::build` closure + `EntityInfo::of` + list `Vec` build/drop | ~10 | ~28 |
| `acacia_nbt` decode and drop (`drop_glue<Value>`, `read_payload`, `read_str`) | 7.9 | 8.9 |
| malloc / free / memmove | 6.0 | 7.7 |
| `acacia_bot::state` (`Entity::solid_box` 1.7, `Bot::apply` 2.3, ...) | 6.3 | ~7 |
| session decode + inflate / proto reads / raknet | 3.1 / 2.2 / 0.7 | similar |
| acacia-host (`play` select poll 1.4–1.7) + serde_json batch encoding | 1.9 + 2.0 | similar |

The interner's share moved into the per-tick list rebuild itself, which is now the top shard frame: ~600
`EntityInfo`s per bot per tick, rebuilt whenever any entity packet arrived (Not done, below). At 100 bots the
interner was 18.4% of shard samples.

JS thread, 300 bots (% of busy samples; busy = 25% of the window before, 21% after):

| frame | before | after fix 2 |
|---|---|---|
| `NativeTransport #receive`: `JSON.parse` of the record batches (self) | 44.3 | 47.3 |
| addon calls (`NativeHost`, self) | 16.6 | 14.3 |
| bench listener (`Object.keys(bot.entities)`) | 13.7 | 14.9 |
| `bot.hot` (inclusive) | 11.8 | ~2 (`NativeHot.read` 1.5 self) |
| `bot.entities` delta read (inclusive) | 13.0 | 13.8 |
| GC | 3.9 | 3.9 |

Natively the JS thread is 78% libnode (JSON parser, string-to-double) and 11% addon. `entityMoved` is ~95% of the
batch bytes: each moved entity is a ~200-byte `Entity` object.

## Fixes (2026-11-04)

Interleaved A/B on the same box, 300 bots, `READ=entities`, `perf stat -e instructions,cycles --per-thread` over a
15 s window, 3 reps a side (6 for fix 1's after side, which is also fix 2's before side). Each side ran from its own copy of
the package; rows in `results/events.jsonl` tagged `prof-base` / `prof-fix1` / `prof-fix2` (the profile runs
included; the stat counts themselves are not in the rows).

| fix | metric | before | after |
|---|---|---|---|
| 1. snapshot lists reuse the entity strings at the same index (acacia-bot `snapshot/slots.rs`) | shard instructions, median | 107.9 G (107.6–108.5) | **98.7 G** (97.4–100.5) |
| | shard cycles, median | 145.3 G | **124.9 G** |
| | shard CPU | 199–202% | **170–180%** |
| 2. `bot.hot` through a `Float64Array` instead of JSON (acacia-node `hot.rs`, `native-hot.ts`) | JS instructions, corrected for entity moves | – | **−1.4 / −1.8 / −2.3 G** (~5.6%) |
| | `bot.hot` share of the busy JS thread | 11.8% | ~2% |

- Fix 2's JS instructions scale with entity moves (0.196 G per 1k moves/s, fitted on the six fix-1 runs), so each
  after run is compared to that fit at its own move rate; raw medians were 32.6 G before and 31.9 G after.
- Load averages: 41–62 across the A/B runs, both sides alike.

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

## Tried and dropped: caching entity vectors

Caching `entity.position` / `velocity` per entity (2026-11-03, not committed) made reads slower, not faster. V8
allocates a short-lived `{x, y, z}` for about what a cache hit costs (~10 ns), and keeping young vectors on
long-lived entity state adds write barriers and promotions on every miss.

- `bench:events` 100 bots `READ=positions` (frozen vectors, kept while the numbers matched; 3 reps a side,
  interleaved, load 62–75; rows tagged `cache-before` / `cache-after`): JS thread 15.6 / 18.1 / 17.6% without, 21.6 / 22.2 / 23.8% with.
- `bench:reads` `every entity .position` (27 entities): ~3,010 ns without, ~3,100 with (2 runs a side).
- Microbench, 600 entities a tick (`node`, `EntityTable` alone), ns per entity per tick, without → with:
  10% moving, one read 23 → 36, three reads 44 → 56; 90% moving, one read 57 → 77. That is the cheapest variant
  (unfrozen, one count per apply); value-checked frozen vectors were worse (37–141). `Object.freeze` alone costs
  ~47 ns per vector.

## Open

- **Per-tick entity list rebuild** (the top shard frame, ~28% of shard samples at 300 bots): `Publisher` rebuilds
  every `EntityInfo` whenever `Entities::version` moved, and it moves on every entity packet, in-view or not. A
  per-entity change stamp set where the tracker applies a packet (`state/entities/apply.rs`, `moves.rs`) would let
  `ListBuilder` copy unchanged entries and visit only the touched ones. Not done here: it reaches into the
  tracker, outside this branch's snapshot code.
- **`entityMoved` as JSON objects** (~95% of batch bytes; `JSON.parse` is ~45% of the busy JS thread at 300
  bots). A `node` microbench (45 records of 24 entities): parsing `Entity` objects costs 1.33–1.46 µs per
  entity, rows (`[id, uniqueId, kind, username, x, y, z, vx, vy, vz, yaw, pitch, health]`) expanded to the same
  objects in JS 0.63–0.70 µs, with 38% fewer bytes. That changes the wire schema (acacia-api `Event::EntityMoved`,
  the daemon, TS types), so it waits for a decision.
- No heaptrack attribution of the extra ~0.8 MB/bot; arenas are the lead.
- RakNet `BadSplit` losses (3–4 across the earlier 300-bot runs; none in the 18 runs of 2026-11-04):
  diagnosis and fix for the raknet owner in `research/js-measure/badsplit-diagnosis.md`. Out-of-scope hot
  spots for other owners: `research/js-measure/hotspots-for-owners.md`.
- The Rust (`memory_bench`, branch js/measure) and bedrock-protocol baselines are quoted from memory-results.md.

## Reproduce

```sh
npm run build                       # events, memory (npm run build:bench for reads and READ=)
npm run bench:memory -- 100         # one run, one JSON line on stdout; env in the script header
npm run bench:events -- 300        # READ=none|hot|entities|positions; TAG=<label> tags the row;
                                    # CPU_PROF=x.cpuprofile profiles the JS thread over the window
npm run bench:reads
bash bench/matrix.sh memory 3       # the matrix, rows to results/<bench>.jsonl
node bench/summarize.mjs            # median tables
```

The server is `$SAPLING/target/release/examples/load_server`
(`cargo build --release -p sapling --example load_server`). Pass `ADDRESS=host:port` to use a running one.
