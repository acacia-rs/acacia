# @acacia-rs/bot benchmarks

Item 9 of `reports/Acacia native Node package design.md`: the real package on the release `acacia-node` addon.
The earlier numbers in `research/js-measure/` came from a synthetic TSFN crate and from Rust-only runs. Raw rows
are in `results/*.jsonl`, one JSON line per run (`TAG=` env tags a row). `node bench/summarize.mjs` prints the
median tables.

This file holds the current numbers. Superseded runs (the first runs, the batch-limit bug, the fixes' and the
entity deltas' before/after A/Bs, the first profile round) are in `HISTORY.md`.

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
- `bench:events`: listeners on `physicsTick`, `entitySpawn`, `entityGone` and `entityMoves` for every bot; a 10 s
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
300 bots, after both fix rounds below (2026-11-04, 3 reps, load 34–44): ~10.9k events/s (`physicsTick`
19.4/bot/s, 86–102k entity moves/s in `entityMoves`), JS thread 18–19%, shard threads 106–108%, forwarder 0.3%,
immediate RTT p50 1.1–1.2 ms, `lagged` 0. Per bot that is ~0.36% of a core on the shards and ~0.06% on the JS
thread (the profiling build; the plain release one is the same code). A read costs what changed since the last one (entity deltas, contract in the package
README). Earlier per-tick read tables (2026-11-03, before later commits; today's base gave a JS thread of
27–29% where they show 14–19%) are in `HISTORY.md`.

## Profile (2026-11-04)

`bench:events` with ticks, entities and `READ=entities`, the release code with line tables (`--profile
profiling`, `CARGO_PROFILE_PROFILING_DEBUG=line-tables-only`). Native: `perf record -F 499 -g -p <node>` over 15 s
of the window; JS: `CPU_PROF=<file>` (the inspector's profiler over the window). Self time: without frame
pointers perf's call chains (fp and DWARF alike) don't unwind. For callers, build the addon with `RUSTFLAGS="-C
force-frame-pointers=yes"` into its own target dir, record with `-g`, and read `perf script --no-inline` (plain
`perf report` resolving inlines took minutes).

CPU split (% of one core; 300 bots, load 34–58):

| code | shard threads (4) | JS thread | forwarder | sapling server |
|---|---|---|---|---|
| js/cancel before the fixes (100 bots: 60.7 / 14.4) | 199–202 | 27–29 | 0.4 | 70 |
| after the first round | 170–180 | 26–28 | 0.3 | – |
| after the second round | 106–108 | 18–19 | 0.3 | – |

Shard threads, 300 bots, top self frames after the second round (% of shard samples, `lazy-300-perf-1`):

| frame | % |
|---|---|
| kernel (unresolved) | 13.0 |
| `Entity::solid_box` (physics collision boxes over every entity) | 8.7 |
| `bytes` `shared_clone` + `shared_drop` (packet buffers' refcounts) | 8.9 |
| entity list rebuild on a spawn or removal (`ListBuilder::build`, `EntityInfo::of`, `Vec` collect) | 6.2 |
| `ListBuilder::patch` (the per-tick entity numbers) | 2.7 |
| SipHash `hash_one` + `Hasher::write` (`Entity::health`, `block_entities`' position maps) | 3.5 |
| `Bot::apply` / host `play` poll / `World::get` | 3.3 / 3.2 / 2.8 |
| malloc / free / memmove | ~7 |
| NBT (`acacia_nbt`) | < 0.5 |

First round's frame tables (the interner, `bot.hot` as JSON, the JS-thread split): `HISTORY.md`. No JS profile
was taken after `entityMoves` rows; its instructions are below.

## Fixes (2026-11-04)

Interleaved A/B on the same box, 300 bots, `READ=entities`, `perf stat -e instructions,cycles --per-thread` over a
15 s window, 3 reps a side, each side from its own copy of the package and its own profiling addon. Fixes 3–5 are
the second round, each against the one before it (rows in `results/events.jsonl` tagged `r2base` / `rows` /
`stamp` / `lazy`, `fp` the frame-pointer profile; the stat counts themselves are not in the rows). The first round (fixes 1–2) is in `HISTORY.md`:
shard instructions 107.9 → 98.7 G from reusing entity strings, ~5.6% of the JS thread from `bot.hot` as a
`Float64Array`.

| fix | metric | before | after |
|---|---|---|---|
| 3. `entityMoves` as `[id, x, y, z, vx, vy, vz, yaw, pitch, health]` rows instead of `Entity` objects (acacia-api `EntityRow`, `reads.ts`) | JS instructions, median | 31.4 G (31.2–33.4) | **17.9 G** (16.8–18.1) |
| | JS thread CPU | 27–28% | **18–19%** |
| | shard instructions (serde writes less) | 101.6 G | **96.8 G** |
| 4. entity change stamps: the published list patches only the entities a packet changed (acacia-bot `state/entities/stamps.rs`, `snapshot/publish.rs`) | shard instructions, median | 96.2 G (95.5–96.9) | **88.6 G** (88.0–90.1) |
| | shard cycles, median | 121.4 G | **95.3 G** |
| | shard CPU | 166–173% | **135–137%** |
| 5. block entity NBT kept undecoded until read (acacia-bot `state/block_entities.rs`) | shard instructions, median | 89.5 G (89.1–89.6) | **61.4 G** (61.3–62.5) |
| | shard cycles, median | 96.4 G | **75.5 G** |
| | shard CPU | 134–137% | **106–108%** |

- The after sides of fixes 3 and 5 ran at equal or lower move rates (fix 4's at a higher one, 102–110k vs
  89–98k/s), so none of the gains come from fewer moves. Immediate RTT p50 fell 2.1–2.5 → 1.1–1.2 ms over the round.
- Fix 5's callers came from a frame-pointer build (`fp-300-perf-1`): `acacia_nbt` was under 21% of shard samples,
  all in acacia-bot's own state code, none in proto or session. `BlockEntities::block_entity` decoding every
  `BlockActorData` 11.3%, its `insert` dropping the replaced tree 5.2%, `on_block_update` dropping on removal 3.3%,
  `Containers::block_entity` walking for `CustomName` 1.2%. The `mixed` world's 225 clocks each drive a sticky
  piston, which sends its block entity (piston arm, moving block) on every move, and physics bots keep all block
  entities (`BlockEntityTracking::All`). Kept entries now hold the bytes and decode on the first `get`/`iter`;
  containers skip the walk without `CustomName` in the bytes.
- Load averages: 34–44 across the second round's runs, both sides alike.

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

- **Full entity list rebuilds on spawns and removals** (~6% of shard samples): mobs strolling in and out of view
  rebuild all ~600 entries; slot reassignment could patch in place too, appending and swap-removing entries.
- **`Entity::solid_box`** (8.7%) and the packet buffers' refcounts (8.9%): owners and leads in
  `research/js-measure/hotspots-for-owners.md`.
- **`entityMoves` still crosses as JSON** (a `node` microbench, 45 records of 24 entities: rows parse in
  ~0.6 µs per entity, `Entity` objects ~1.5, flat `ids` + `numbers` arrays ~0.55). Writing the rows into a
  `Float64Array` in-process, as `bot.hot` does, would take most of the rest off the JS thread.
- No heaptrack attribution of the extra ~0.8 MB/bot; arenas are the lead.
- RakNet `BadSplit` losses (3–4 across the earlier 300-bot runs; none in the 38 runs of 2026-11-04):
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
