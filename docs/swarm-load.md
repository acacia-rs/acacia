# Swarm load profiles

Busy-world profiles of the headless client: a 50-bot swarm (`examples/swarm_load.rs`) against sapling's
`load_server` example on testbox, with perf over the swarm once every bot has joined.

| Scenario | Bots | Wandering | Redstone clocks | Pigs |
|---|---|---|---|---|
| crowd | 50 | 0 | 0 | 0 |
| wander | 50 | 50 | 0 | 0 |
| redstone | 50 | 0 | 225 | 0 |
| mobs | 50 | 0 | 0 | 300 |
| mixed | 50 | 25 | 225 | 300 |
| mixed150 | 150 | 75 | 225 | 300 |

Every bot tracks entities; wanderers run physics and walk to random goals near spawn.

## Loop

1. Tests: `DETACH=1 tools/remote-test.sh -p <crate> --lib --test <name>`. Keep `--lib`/`--test`: a bare
   `cargo test -p` also builds every example (GBs of the shared tmpfs).
2. A/B: `DETACH=1 tools/swarm-load-revs.sh crowd,redstone,mobs,wander <rev A> <rev B>` (`.` = working tree).
   Each commit's binary is built once and kept in testbox `~/work/swarm-load-bins`; four scenarios take ~7 min.
3. Profiles: `tools/swarm-load-suite.sh <local dir> [scenario...]`, or `tools/swarm-load.sh <scenario>` on testbox
   (`PERF=record` is the default there: perf.data plus report.txt with self and inclusive time).

With `DETACH=1` the result is the tail of `<remote dir>/run.log` (or `test.log`) once `run.done` appears; watch it
with a Monitor. Local polls get reaped on this machine when memory runs short.

## Measuring

- Judge by instructions. The two sides run at once, each with its own server, port and cores (swapped each round),
  so both see the same box. One binary against itself (2026-10-24): crowd 1.001, redstone 1.003, mobs 1.000,
  wander 0.995. Cycles move 1-3% on a quiet box and much more on a busy one; the bench lock isn't needed.
- `mixed` doesn't repeat: the same binary gave 353-482 G instructions run to run. Its wanderers path among 300
  mobs, and A* is cut by wall-clock budgets. Lifting the budget (465-727 G) showed the work itself is chaotic.
  Pathfinding changes go to its own benches, not to this.
- `mixed150` disconnects bots: sapling batches ~290 KB per PlayerList entry per tick (44 MB batches), past
  acacia's 16 MiB decompressed limit.

## Results

7c3ff59 (before) against 8f4d01b, instructions / cycles: crowd -4.3% / -12%, redstone -34% / -26%,
mobs -8.9% / -11%, wander -15% / -10%. From:
- physics: one tick's block lookups memoised in a dense box; integer floor/ceil instead of libm; one liquid scan.
- session: batches up to 256 bytes deflated by a fixed-Huffman encoder (zlib reset a 128 KiB table per batch).
- block entities: the root `id`/`CustomName` read from raw NBT, decoded only when kept; a key whose bytes are
  absent skips the walk (pistons and observers send BlockEntityData every move).
- client driver: one timer reset instead of a new sleep per loop pass; entity maps Fx-hashed.

Tried, no gain: an exact float floor in `probe_area` and an inlined `block_pos` (wander -0.3%, in the noise).

## Left, by owner

- Pathfinding (wander ~55% of the swarm): `CellMemo::get` ~10% self, `CellMemo::default` 1.7% (a memo built per
  search), `cell::state_flags` matches block names with `str` searches per cell (1.4% in `StrSearcher::new`).
- RakNet: kernel UDP sends are 11% of mobs (one `sendto` per datagram; `sendmmsg` would batch them).
- Inherent: sha256 per packet (testbox lacks SHA-NI), tokio wakers and channels.
