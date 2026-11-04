# @acacia-rs/bot benchmark history (2026-11-03, appended 2026-11-04)

Superseded runs, kept as they were recorded: the first runs (before the batch-limit fix and the latency and read
fixes), the fixes' before/after, and the entity-delta A/B. Current numbers and the method: `RESULTS.md`. Rows are
in `results/*.jsonl`: untagged rows are the first runs, `after` / `after-v1` the fixes, `delta-before` /
`delta-after` the delta A/B, `prof-*` the first profile round; `results/before-batch-limit/` holds the runs the batch-limit bug invalidated.

## First runs

### Headlines

- **Memory: ~2.8 MB per standing bot on ~69 MB fixed.** (Still current: `RESULTS.md`, Per-bot memory.)
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

Load average at these runs was 26–95 (usually 40–60); joins were spaced 100 ms apart.

### Event delivery end to end (`bench:events`)

Values are medians over reps (5 at 300 bots, 4 at 100, 3 at 10).

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

### Sync reads (`bench:reads`)

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

### Bugs and bottlenecks

1. **Fixed: bots dropped in crowds.** The client refuses a decompressed batch over 32 MiB
   (`crates/acacia-session/src/batch.rs:10`). A newcomer on a crowded server gets every player's skin at once,
   which is more than that. Before the fix, 179 of 300 event-bench bots and ~89 of 200 memory-bench bots ended
   with `lost: Protocol("decompressed batch exceeds limit")` from ~20 s in. Their calls then got `UNKNOWN_BOT`
   (rows in `results/before-batch-limit/`). The fix is `HostConfig::max_batch_bytes`, default 128 MiB.
2. **Call latency floor = shard flush.** A `done` record waited for its shard's next flush
   (`HostConfig::flush_every`, 10 ms), so even an immediate call cost ~4 ms at idle.
3. **`bot.entities` per tick was O(entities) per read.** Any entity moving bumped the version, and then the
   whole list was serialized in Rust, parsed in JS and re-indexed.
4. **`findBlocks` scanned whole sections**: all 4096 cells of every section that overlaps the radius and holds a
   match, so a 4-block search cost ~39 µs.
5. **Inventory miss ~220 µs**: `inventory()` rebuilt a `Map` of the whole item table on every cold change.
6. Not investigated: 3 RakNet losses (`lost: Transport(Protocol("BadSplit"))`) across ~2100 bot-runs at
   300 bots, and the 7 join `DISCONNECTED` failures, all at load 50–66.

## After the fixes

Fixes 2–5 above, one commit each on js/cancel (2026-11-03, later): `done` flushes at once (acacia-host
`outbound.rs`), entity reads split into members and a numbers array, `find_blocks` clipped to the search box,
item names mapped once per table. Rows are tagged `"tag": "after"`. Load was 10–27 against 26–95, so the CPU and
latency gains are partly load; the RTT and entity-read gains are not (see the idle row and the cost model).
`after-v1` rows are a first cut of the entity split, superseded.

Calls (`bench:events`, `READ=none`; median of 2 reps after, of 3–5 before; idle = 1 bot, `CLOCKS=0 MOBS=0`):

| bots | immediate RTT p50 / p99 ms | waitForTicks(1) p50 / p99 ms | batches/s | JS thread |
|---|---|---|---|---|
| 1, idle | 2.19 / 12.7 → **0.022 / 0.06** | 56.3 / 59.6 → 51.3 / 51.8 | 78 → 117 | 2.4% → 0.7% |
| 10 | 4.1 / 12.3 → **0.18 / 2.1** | 55.9 / 62.4 → 51.4 / 54.5 | 224 → 258 | 2.2% → 4.7% |
| 100 | 8.4 / 20.9 → **0.44 / 10.3** | 57.9 / 73.5 → 51.7 / 65.6 | 328 → 401 | 8.7% → 9.4% |
| 300 | 10.7 / 65.9 → **1.41 / 30.9** | 62.6 / 137 → 54.0 / 113 | 246 → 311 | 17.3% → 16.1% |

- The flush floor is gone: a `done` now leaves with the events queued before it. The probes run back to back
  10 ms apart, so faster calls mean more of them, which is most of the 10-bot JS-thread rise.

Per-tick entity reads (each `physicsTick` listener reads `bot.entities`; `positions` also reads every entity's
`.position`):

| bots, read | JS thread | loop delay p50 / p99 ms | immediate RTT p50 ms | finished |
|---|---|---|---|---|
| 100, entities | 77.2% → **19.8%** | 0.16 / 76 → 0.07 / 5.9 | 11.0 → 1.15 | yes |
| 100, positions | – → **18.2%** | – → 0.06 / 2.0 | – → 0.48 | yes |
| 300, entities | saturated → **45.4%** | – → 0.06 / 5.8 | – → 1.58 | no (killed at 600 s) → yes |
| 300, positions | – → **68%** | – → 0.06 / 8.8 | – → 1.64 | yes (2 of 2) |

- Cost model (`node` microbench, 600 entities): a move is one `entitiesVersion` call plus one copy into the
  reader's `Float64Array`; reading all 600 positions is 7 µs. What remained at 300 bots was membership churn:
  ~1200 spawns/removals a second across the bots, each a members fetch. Sending full `Entity` JSON and building
  fresh getter objects cost ~1.1–1.5 ms per change at 600 entities (the `after-v1` cut, whose 300-bot runs went
  past 5.7 GB RSS behind a saturated JS thread and were killed). Members as `[id, uniqueId, kind, username]`
  rows (24 KB, parse 83 µs) with objects carried over by id cost ~90 µs.

Sync reads (`bench:reads`, median of 3 runs; ns/op unless noted):

| read | before | after |
|---|---|---|
| `findBlocks` stone, r4, count 1 | 38,600 | **2,600** |
| `findBlocks` stone, r8, count 16 | 51,700 | **11,050** |
| `findBlocks` missing block, r16 | 2,600 | 1,650 |
| `bot.inventory` miss p50 | 218 µs | **76 µs** |
| `bot.entities` miss p50 (a membership change) | 141 µs | 134 µs |
| `bot.players` miss p50 | 110 µs | 88 µs |
| `bot.nearestEntity()` | 1,360 | 2,160 |
| cached getters (`hot`, `entities`, `inventory`, `blockAt`) | 530–1,280 | 350–840 |

- The item-table `Map` rebuild was 97 µs of the inventory miss, timed alone (2076 items).
- `nearestEntity` got slower: each `.position` became a getter that builds a `Vec3`.

## Entity deltas A/B

Commit 9da8cbc (2026-11-03, late): entity reads apply deltas instead of refetching the members. An interleaved
A/B on one box: `delta-before` is the parent commit ad782b8 and `delta-after` is 9da8cbc, both `npm run
build:bench` addons. Each rep runs reads, then events 100/300 × `READ=entities|positions`, for one side and then
the other, and rep 2 swaps the order; 2 reps a side. **Load was 77–122**, so compare only within this section.

Per-tick entity reads (`bench:events`). JS thread %, each rep, with churn (spawns + removals per second across
the bots):

| bots, read | before: JS % (churn/s) | after: JS % (churn/s) | loop p99 ms before → after |
|---|---|---|---|
| 100, entities | 14.2 (155), 16.0 (160) | **12.0** (315), **13.7** (199) | 6.3, 9.7 → 5.8, 6.4 |
| 100, positions | 20.1 (185), 19.9 (160) | **15.9** (165), **16.9** (155) | 10.6, 11.8 → 7.1, 10.4 |
| 300, entities | 28.2 (1150), 34.7 (427) | **14.2** (1247), **18.6** (1757) | 44, 27 → 17, 34 |
| 300, positions | 59.9 (324), 41.5 (1459) | **35.5** (1314), **40.9** (725) | 1805, 8.4 → 139, 144 |

- `READ=entities` at 300 bots halves the JS thread even though the after runs had 1.1–4× the churn.
- `READ=positions` gains less: its listener builds a fresh `Vec3` for each of ~600 entities per bot per tick,
  ~3.6 M objects/s at 300 bots.
- At this load 2–4 of 300 bots hit the join timeout in 3 of 4 after runs, and 3 in 1 of 4 before runs. Call RTTs
  were dominated by load (p50 9–94 ms at 300 bots on both sides) and are not compared.

Sync reads (`bench:reads`, 2 runs a side, ns/op):

| read | before | after |
|---|---|---|
| `bot.nearestEntity()` (~28 entities) | 2,550 / 2,541 | **1,540 / 1,474** |
| `bot.entities`, cached | 586 / 566 | 499 / 533 |
| `bot.entities` miss p50 (a membership change) | 175 / 166 µs | 116 / 114 µs |
| `bot.players` miss p50 | 148 / 138 µs | 133 / 131 µs |

The miss cost no longer grows with the entity count. A scratch probe made one membership change, waited 150 ms
without reading, read `bot.hot` once, then timed one `bot.entities`, 60 changes per run:

| entities | before | after | first `bot.hot` after the same idle |
|---|---|---|---|
| 38 | 185 µs | **87 µs** | 78–81 µs |
| 610 | 643 µs | **87 µs** | 78–79 µs |

The after-side ~85 µs is no larger than the first `bot.hot` read after the same 150 ms idle, so it is this box's
cost of a first read after idling (at load ~80), not delta work. A read right after it costs 2–3 µs.

## Event throughput and per-tick reads before the 2026-11-04 profile

Moved from `RESULTS.md` when the profile and its fixes replaced them (their rows: untagged and `delta-after`).

- Throughput (first runs): 300 bots delivered ~11.3k events/s (`physicsTick` at 19.4/bot/s, 113k entity moves/s
  in `entityMoved`) for ~17% of the JS thread, ~1.5 µs per moved entity. Loop delay p50 0.06 ms. `lagged` was 0
  in every run.
- A per-tick `bot.hot` read cost ~7 µs in aggregate (`READ=hot`, 300 bots: JS thread 17.3% → 21.6%), when it
  still crossed as JSON.

Per-tick entity reads after the entity deltas (load 77–122, 2 reps; JS thread % with churn, spawns + removals per
second across the bots, in parentheses):

| bots, read | JS thread % (churn/s) | loop p99 ms |
|---|---|---|
| 100, entities | 12.0 (315), 13.7 (199) | 5.8, 6.4 |
| 100, positions | 15.9 (165), 16.9 (155) | 7.1, 10.4 |
| 300, entities | 14.2 (1247), 18.6 (1757) | 17, 34 |
| 300, positions | 35.5 (1314), 40.9 (725) | 139, 144 |

`positions` builds a fresh `Vec3` for each of ~600 entities per bot per tick, ~3.6 M objects/s at 300 bots.

## Profile and fixes, first round (2026-11-04)

Moved from `RESULTS.md` when the second round's fixes replaced them (rows tagged `prof-base` / `prof-fix1` /
`prof-fix2`). Steady state after this round, 300 bots, `READ=entities`, load 43–49: ~10.9k events/s, 94–105k
entity moves/s, JS thread 26–28%, shard threads 173–180%, immediate RTT p50 2.6–3.0 ms.

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

JS thread, 300 bots (% of busy samples; busy = 25% of the window before, 21% after):

| frame | before | after fix 2 |
|---|---|---|
| `NativeTransport #receive`: `JSON.parse` of the record batches (self) | 44.3 | 47.3 |
| addon calls (`NativeHost`, self) | 16.6 | 14.3 |
| bench listener (`Object.keys(bot.entities)`) | 13.7 | 14.9 |
| `bot.hot` (inclusive) | 11.8 | ~2 (`NativeHot.read` 1.5 self) |
| `bot.entities` delta read (inclusive) | 13.0 | 13.8 |
| GC | 3.9 | 3.9 |

Natively the JS thread was 78% libnode (JSON parser, string-to-double) and 11% addon; `entityMoved` was ~95% of
the batch bytes, each moved entity a ~200-byte `Entity` object.

| fix | metric | before | after |
|---|---|---|---|
| 1. snapshot lists reuse the entity strings at the same index (acacia-bot `snapshot/slots.rs`) | shard instructions, median | 107.9 G (107.6–108.5) | **98.7 G** (97.4–100.5) |
| | shard cycles, median | 145.3 G | **124.9 G** |
| | shard CPU | 199–202% | **170–180%** |
| 2. `bot.hot` through a `Float64Array` instead of JSON (acacia-node `hot.rs`, `native-hot.ts`) | JS instructions, corrected for entity moves | – | **−1.4 / −1.8 / −2.3 G** (~5.6%) |
| | `bot.hot` share of the busy JS thread | 11.8% | ~2% |

Fix 2's JS instructions scale with entity moves (0.196 G per 1k moves/s, fitted on the six fix-1 runs), so each
after run was compared to that fit at its own move rate; raw medians were 32.6 G before and 31.9 G after. Load
averages 41–62.
