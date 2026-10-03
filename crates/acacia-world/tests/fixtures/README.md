# Captured packets

One packet per file: varint header (`id | sender << 10 | target << 12`) followed by the body, i.e. what
`acacia_proto::RawPacket::parse` takes. Used by `tests/captures.rs`.

- `geyser/` — local Paper + Geyser lab (Paper 26.2, Geyser Standalone 2.11.3 + Boar, superflat world with a stone test arena at spawn),
  2026-10-03, protocol 2193. `start_game.bin` (7 `geyser_custom:*` blocks) plus `LevelChunk`s for the
  empty chunk at 0,0 and the 3×3 chunks around spawn (x=60, z=6). Full chunks: `sub_chunk_count` > 0,
  cache disabled.
- `bds/` — local BDS 1.26.52. `LevelChunk` is in sub-chunk request mode (`sub_chunk_count` 0,
  `highest_subchunk_count` set, payload = biomes only); block data needs `SubchunkRequest`/`Subchunk`.

Capture more with
`cargo run -p acacia-client --example capture -- <server> <name|@account> <secs> <outdir> [ids...]`
(ids: 11 StartGame, 58 LevelChunk, 174 SubChunk, 21 UpdateBlock, 172 UpdateSubChunkBlocks).
Rename to `start_game.bin` / `level_chunk_*.bin`; keep this directory under ~1 MB.
