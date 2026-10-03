# Vanilla SubChunkRequest scheduling (Win 1.26.52)

Source: MITM captures `.testserver/mitm/skins-capture/20261002-100800.jsonl` (session **A**, idle, ~3 min)
and `.testserver/mitm/20261001-211901.jsonl` (sessions **B0**, **B1**, walking/falling). Local BDS,
client asked radius 12, server granted 10. Spawn is on a tall structure: player eye at y=307.6
(sub-chunk 19); most terrain columns top out at sub-chunk 3-5. All numbers below are from these 3 sessions
(227 requests, ~3.9k requested sub-chunks).

## Wire facts (decoding)

- LevelChunk in request mode: `SubChunkCount=0`, `SubChunkLimit` optional **present**. Limit = number of
  sub-chunks counted from the bottom (index -4). Seen: 8, 9, 10, 11, 19, 24 (24 = full -4..19).
- SubChunk header: `CacheEnabled bool, Dimension varint, Position 3x int32 LE`, then varuint entry count.
  Position echoes the request. Entries come back in request order.
- Result codes seen: only `1 Success` and `6 SuccessAllAir`. Never ChunkNotFound. HeightMapType seen:
  1 HasData, 2 TooHigh, 3 TooLow (40 entries, B0 only).
- RTT request->response: median 53-67 ms, p90 96-122 ms. Every request answered except 27 at the end of
  B0 (after respawn/death); the client never retried those.

## 1. Position and offsets

| Fact | Evidence |
|---|---|
| `Position.x/z` = player's chunk (`floor(x)>>4`, `floor(z)>>4`) at send time | 226/227 requests; the 1 miss was sent the same tick the player crossed a border |
| `Position.y` is always 0 | 227/227, incl. requests sent while the player was at the y=32769 pre-spawn placeholder |
| Offsets dx/dz are relative to Position; dy is therefore the **absolute** sub-chunk index | dy range -4..19 in all sessions |
| max abs(dx), abs(dz) | 10-11 (= server radius 10) |

## 2. Two request streams

Each (column, y-list) group in a request is one of two shapes; they can share a packet.

| Stream | Shape | Share of requested sub-chunks (A / B0 / B1) |
|---|---|---|
| **Bulk** (column loader) | ascending `-4 .. k`, one column | 64% / 49% / 51% |
| **Flood** (render/visibility) | single sub-chunks or short descending runs | 36% / 51% / 49% |

### Bulk stream: fits 100% (208/208 bulk groups)
- Covers exactly the columns with `dx^2+dz^2 <= 17` around the player chunk (radius ~4.1, 57 columns).
  At startup every circle column was bulk-requested except the ones the flood had already fully fetched;
  no bulk column had d^2 >= 18 while Position was current.
- Range = `[-4, -4+limit)` from that column's LevelChunk, minus any top sub-chunks already requested
  by the flood. 187 groups were exact `n == limit`; the other 21 all stopped right below already-requested
  sub-chunks. Heightmaps / result codes play no visible role.
- Order inside the circle: **not** by distance (Spearman(order, d^2) = 0.06-0.23) and not LevelChunk arrival
  order. Looks like hash-set iteration; treat as random.
- Batching: startup <= 3 bulk columns per packet (one outlier of 13); after startup 1, rarely 2.

### Flood stream (partially explained)
- In-circle flood only touched tall columns (limit 19/24) that reach the camera's sub-chunk; terrain columns
  (limit 8-11) inside the circle were **never** flooded, always bulk.
- Outside the circle it requests only sub-chunks below `limit` and **at most 12 sub-chunks below the camera**
  (rel dy in [-12, 0] for horizontal distance 5-10, all sessions). Never above the camera here (no data
  above y=19).
- Progresses one layer per response round-trip: e.g. structure columns (6..10, -1..0) scanned 19 -> 7
  top-down over ~1.4 s. Above-limit space counts as known air without a request, so the fill crosses it
  instantly (hence runs like `(8,0): 18..11` in one packet).
- While falling (B0) the far terrain tops (y=3, 90+ columns per packet) were requested only once the camera
  came within ~12 sub-chunks; then each further fall step requested one more layer down in cave columns.
- Does not stop at the first non-air result: 344 flood requests sat directly below a `Success+HasData`
  sub-chunk vs 172 below an all-air one. Consistent with a face-connectivity (occlusion) BFS from the camera,
  which this data cannot pin down further.

## 3. Timing

| Phase | Behaviour |
|---|---|
| Startup | Server bursts ~300 LevelChunks in <15 ms; first request 0.56-1.3 s later (after load). First packet is bulk (3 cols), flood starts ~15-50 ms later at the camera sub-chunk |
| Startup burst | 39-52 requests; bulk circle done 0.9-1.3 s after first request; up to 8 (A) / 6 (B1) requests and ~240 sub-chunks in flight |
| Frame batching | 8-11% of requests share a millisecond with the previous one (same frame); gaps quantised at ~16 ms (60 fps) |
| Per packet | median 9 sub-chunks, mean 17, max 193 (flood, falling) |
| Steady state | Bulk requests for new columns are serialized: sent with 0 outstanding in 47/54 cases, typically 1-4 frames (12-70 ms) after the previous response |

## 4. Re-requests and chunk changes

- Duplicates: ~4-5% of requested sub-chunks (52 / 44 / 48 per session). Pairs: flood->flood 76, bulk->flood
  36, flood->bulk 32. 66 were re-sent while the first request was still unanswered (often same frame).
  The two streams evidently keep separate "requested" sets.
- Moving into a new chunk: Position switches immediately; the new leading-edge columns of the d^2<=17 circle
  (e.g. 9 columns for a straight step) are bulk-requested **one column per packet**, first one median ~140 ms
  (0-2400) after the crossing, from already-cached LevelChunks (no new LevelChunk needed). Old columns are not re-requested.
- New LevelChunks at the radius-10 edge (moving) get no bulk request; only flood touches them.

## 5. Bot algorithm (radius 2-4)

With radius <= 4 every column the server sends lies inside the d^2<=17 circle, so the bulk stream is the
whole story except the flood near the camera.

```
state: lim[col] from LevelChunk; asked = set(); queue = []; inflight = 0
on LevelChunk(col, limit): lim[col] = limit; if near(col): enqueue(col)
on chunk change or startup:  for col in circle(player_chunk, d2<=17) with lim known and not fully asked:
                                 enqueue(col)
                             shuffle(queue)                       # vanilla order ~ random
enqueue(col): queue.push(col)
each frame (~16 ms):
    if startup: cols = pop up to 3 from queue, allow up to ~6 packets in flight
    else:       if inflight == 0 and queue: cols = [pop 1]  (10% chance: 2)
    for col in cols: ys = [-4 .. -4+lim[col]-1] minus asked[col]; mark asked
    send SubChunkRequest(dim, Position=(pcx, 0, pcz), offsets=(col-pc, y) for each y)
    optional flood: before the first bulk packet, request the camera sub-chunk and its 4 horizontal
       neighbours that are < their column limit, singly; then one layer down per response (max 12 below
       the camera), even if those columns get bulk-requested later (yields vanilla-like ~5% dups)
on SubChunk: inflight -= 1
```

Unsure: (a) the flood's exact rule (occlusion BFS) and how it behaves when the player stands on normal
terrain - every capture spawned on a limit-24 structure; (b) whether the 17 circle tracks the
simulation-distance setting (default 4) or is fixed; (c) startup per-frame caps (3 columns/packet,
~6-8 in flight) may depend on frame rate; (d) behaviour with ChunkRadius <= 4 granted by the server is
untested. Capture a vanilla session spawning on flat ground with radius 4 to settle (a), (b), (d).
