# Client blob cache (Win 1.26.52, protocol 2193)

Source: `.testserver/mitm/cache/20261002-154231.jsonl`, one session, vanilla Windows client joining local
BDS (request mode), ~1 min walking. Decoded with acacia-proto (0 parse errors, no trailing bytes). The
client had played this world before, so its on-disk blob cache was **warm** (see 3). `t` deltas in ms.

## 1. ClientCacheStatus

- Value **`enabled=true`** (body `01`). Our client sent `false` at the time.
- Order: PlayStatus(LoginSuccess) → ResourcePacksInfo (+3) → **ClientCacheStatus (+205 after LoginSuccess,
  +202 after ResourcePacksInfo)** → ResourcePackClientResponse(HaveAll) (+55). Earlier capture
  (2026-10-01 mitm session) had +31 after ResourcePacksInfo; our `delay::CACHE_STATUS` is 25-40.
  Treat 205 as an outlier (cold-start disk cache open), keep the current delay.

## 2. Wire shapes in cache mode

**LevelChunk** (351/351 identical shape): `cache_enabled=true`, `sub_chunk_count=0`,
`highest_subchunk_count=Some(n>0)` (request mode), **`blobs` = exactly 1 hash** = the biome blob,
`payload` = 1 byte `00` (border-block count 0, no block entities). The one biome blob delivered starts
`03 00` (paletted biome storage). Non-request-mode cache LevelChunks (blobs = one per section + biome,
payload = border + block entities) are **not** in this capture; that layout is from gophertunnel, unverified.

**Subchunk** entries (1622, all `cache_enabled=true`):

| result | payload | blob_id | count |
|---|---|---|---|
| Success | empty (len 0) | present | 1046 |
| Success | block-entity NBT (67, 137 B; starts `0a 00 ...`) | present | 2 |
| SuccessAllAir | absent | absent | 575 |

So: section bytes live only in the blob; the entry payload is the **non-cacheable remainder** (block
entities). Full section = `blob ++ entry.payload` (our `decode_section` reads the section and ignores the
rest). Heightmaps are inline as in non-cache mode. All-air entries carry no hash.

Delivered blobs: 216 sub-chunk sections (`09 01` = v9 one storage: 190, `09 02` two storages: 26) + 1 biome
blob. Sizes 526..4951 B, median 2128, 494 KB total.

## 3. ClientCacheBlobStatus (C>S, 51 packets)

- **Every advertised hash slot is listed exactly once** across the session: 1400 listed vs 1399 slots
  (351 LevelChunk + 1048 Subchunk). Repeats of the same hash in different chunks are listed again (dups in
  `have` up to 35 per packet); `missing` never held duplicates (0/221).
- `missing` 221 total (median 1/pkt, max 46); `have` 1179 total (median 9, max 216).
- **Warm-cache evidence:** of 1179 `have`, only 2 were delivered earlier in this session; 1177 came from
  the on-disk cache of earlier sessions. LevelChunk biome hashes: 351 have / 1 missing; the most common
  biome hash was advertised 111x and never sent. 1219 distinct hashes advertised, 217 delivered.
- Batching is not per packet: a status covers the hashes processed since the last one, often spanning
  several LevelChunk/Subchunk packets (up to 84 adverts in one status, spanning up to 1.8 s) and a single
  packet's hashes can split across two statuses. Back-to-back statuses within 1-10 ms occur.
- Latency advert → status (status that completes the advert): median 90, p90 472, min 30.
- **No status before spawn.** 26 LevelChunks arrived from 6.0 s before PlayStatus(PlayerSpawn); first
  status was 416 ms after PlayerSpawn (max latency 6396 = those). Not gated on SetLocalPlayerAsInitialized
  (sent 4.9 s later).
- Vanilla is not perfectly deduped: once it listed 4 hashes as `missing` that had arrived 697 ms earlier.

## 4. ClientCacheMissResponse (S>C, 51 packets)

- **One response per status, FIFO, even when `missing` is empty** (`blobs=0` replies seen). Up to 6
  statuses outstanding at once.
- Contents = exactly `status.missing`, **same order**, in 50/51. The exception: BDS **omitted the 4
  re-requested hashes it had already sent** (13 of 17). BDS tracks what it sent per connection and never
  re-sends: a client that drops a blob cannot get it back this session.
- Latency status → response: median 42, p90 97, max 162. Advert → usable terrain ≈ 130 ms median.

## 5. Ordering

- Status packets are sent in the same batch as SubchunkRequests (same t ±0.2 ms): emitted from the
  client tick, not on packet receipt.
- **The client does not wait for blobs before requesting more:** 28 SubchunkRequests were sent while a
  status awaited its MissResponse. Request scheduling (`subchunk-requests.md`) is unchanged by the cache.

## 6. Hash

`blob_id == xxHash64(seed 0, blob payload)` for **217/217** delivered blobs (inline xxh64, self-tested on
`""`/`"abc"` vectors). We can verify server blobs and compute ids ourselves. No xxhash crate is in
`Cargo.lock`; xxh64 is ~40 lines or `xxhash-rust` (feature `xxh64`).

## 7. Rules for a client reporting `cache=true` without a persistent store

1. Always answer: one ClientCacheBlobStatus covering every hash slot from LevelChunk.blobs and
   Subchunk entry blob_ids; never leave a slot unlisted. BDS answers every status, so it will notice gaps.
2. Hold statuses until PlayerSpawn, then flush (~400 ms after); afterwards flush on the client tick,
   ~1-2 ticks after receipt, coalescing everything processed since the last flush.
3. `missing` = hashes not received this session (deduped within the packet); `have` = hashes already
   received this session, one entry per slot. A fresh join with everything missing looks like a vanilla
   client with an empty/cleared cache; it costs bandwidth (~2.1 KB per non-air section).
4. Never claim `have` for a blob never received unless terrain is not needed: BDS will not send it later,
   and claiming hashes for freshly-modified chunks is something no real client could do.
5. Keep every received blob (payload for physics bots, hash only for idle bots) for the whole connection:
   BDS never re-sends.
6. Never block SubchunkRequests on outstanding statuses.
7. Optional cross-bot sharing of a store looks like a shared disk cache, but makes bots' `missing` sets
   correlate; per-bot `received` sets are safer (`bots-indistinguishable`).

## Implementation plan

1. **`crates/acacia-session/src/session/blob_cache.rs`** (new, session level so idle and physics bots both
   answer regardless of packet filter). `BlobStatusTracker { received: HashSet<u64>, slots: Vec<u64>,
   spawned: bool }`. `on_level_chunk/on_subchunk` push hashes (parse only the header + blob ids; a light
   reader for Subchunk avoids full heightmap decode on idle bots). `on_miss_response` adds hashes to
   `received` (verify with xxh64; drop mismatches). `flush()` → `ClientCacheBlobStatus` with
   `missing` = unreceived deduped, `have` = rest per slot; called from the session tick when
   `spawned && !slots.is_empty()`, plus a one-shot ~400 ms after PlayerSpawn.
2. **`crates/acacia-session/src/session/handlers.rs`**: send `ClientCacheStatus { enabled: self.client_cache }`
   (config flag, default true once 1-6 land; false keeps the old path). Route `LevelChunk`, `Subchunk`,
   `ClientCacheMissResponse` to the tracker before `events.push_back`; mark `spawned` in `on_spawn`. Hook
   `flush` where deferred packets are released (`session/mod.rs`).
3. **`crates/acacia-session/src/xxh64.rs`** or dependency `xxhash-rust` (shared by core and bot).
4. **`crates/acacia-bot/src/world/blobs.rs`** (new, physics bots only): `BlobStore { blobs: HashMap<u64,
   Bytes>, waiting: HashMap<u64, Vec<Pending>> }` where `Pending = Section { x, y, z, rest: Bytes }`
   (or a full LevelChunk awaiting N blobs for non-request mode). Store per bot (or per `SharedWorlds`
   entry keyed by server, content-addressed so safe; status reporting stays per session).
5. **`crates/acacia-bot/src/world/mod.rs`**: add `ClientCacheMissResponse::ID` to `PACKETS`. Subchunk
   with `cache_enabled`: for `Success` + `blob_id`, insert `blob ++ payload` via `view.insert_sub_chunk`
   if the blob is stored, else queue in `waiting`; `SuccessAllAir` unchanged. MissResponse: store blobs,
   drain `waiting`. `level_chunk`: request mode needs nothing (biome blob unused by physics); non-request
   cache mode: assemble `concat(blobs[..sub_chunk_count]) ++ payload` once all present, then
   `insert_level_chunk`. Also record MissResponse in `trace::Recorder` so replays reproduce terrain.
6. **`crates/acacia-bot/src/subchunks.rs`**: no behaviour change (vanilla does not gate requests on
   blobs). Add a test with `cache_enabled: true` responses to pin that.
7. **`crates/acacia-bot/src/bot.rs`**: none beyond PACKETS; idle bots never decode terrain and keep no
   payloads (the core tracker holds only hashes).
8. Tests: fixture of this capture's S>C LevelChunk/Subchunk/MissResponse bodies only (no login/identity
   lines) → replay through `BlobStatusTracker` with an empty `received`, assert every slot listed once,
   no duplicate `missing`, and that the world store reconstructs sections whose xxh64 matches.
9. Live check on local BDS via `tools/mitm`: compare our status/miss counts and latencies to sections 3-5.
