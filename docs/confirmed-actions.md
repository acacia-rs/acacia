# Confirmed actions (C3)

Roadmap C3: a place or break counts only once the server shows it; timeouts scale with lag and retry;
movement and work pause after a server setback; one governor every action asks. Done bar: dig and bridge
courses under injected lag with 0 ghost blocks and 0 loops (results below).

## The bot's world is the server's

The tracked world changes only on server block packets (`UpdateBlock`, `UpdateBlockSynced`,
`UpdateSubchunkBlocks`), with one exception: a placement is predicted (below). So a "ghost" can only be a
prediction the server never confirmed, or a cell whose update the bot never saw. The live check compares
the lagging bot's world with a fresh observer's, cell by cell.

## Confirmation (`interact/confirm.rs`)

`Confirm` is a tick-driven, network-free state machine; `Bot::await_confirm` feeds it one tick at a time.

- Done when the cell shows the expected state (`Gone` for a break, `Solid` for a place, a property for a
  door). A predicted cell counts only once a server update for it arrives.
- Before the deadline (governor timeout, in ticks) it waits, unless an update shows the cell unchanged:
  the server refused, so it retries at once.
- At the deadline it re-reads the cell: unchanged → retry (up to 3 tries), changed to something else →
  failed, unloaded → failed.

`break_block` re-mines on retry (an `AbortBreak` first). `place_block` clicks again. Both stay
idempotent when the first try was only late: a second break of air or a click into a filled cell does
nothing on the server. Door clicks get one try only, since a second click would toggle the door back.

## Predicted placement

Vanilla writes a placed block locally before the server answers. Without that, a bot sneaking to the edge
of its bridge is held back by the old edge while the server, which already has the block, lets it slide
on: under 150 ms of lag, the bridge course took 22 corrections without prediction and 1 with it.
`place_block` writes the held block's id into the world (`Bot::predict_block`) after the click. When a try
ends unconfirmed with no update seen, the old block goes back (`unpredict_block`), and so does the stack
change the click predicted (`unpredict`), unless the server has set that slot since. Shared worlds
(`BotConfig::shared_worlds`) show the prediction to every bot on them until it resolves.

## Governor (`governor.rs`, `Bot::governor`)

- Echo round-trip: RFC 6298 smoothing of first-try echoes (Karn: retried actions are not sampled).
  Timeout = srtt + 4·rttvar + 2 ticks, clamped to 0.5–5 s; 600 ms before the first sample.
- Admission (`Bot::admit_action`): at most `Limits::max_in_flight` actions awaiting their echo (default 4),
  spaced by a gap drawn from `min_gap_ticks..=max_gap_ticks` (1..=3), and none
  within `SETTLE_TICKS` (10) of a setback.
- Counters: `actions_sent`, `retries`, `rtt`, `lagging` (srtt > 300 ms).

Break, place and door clicks ask it. Direct-control clicks (`start_mining`, `right_click_block`) do not:
the caller aims and paces those itself.

## Setbacks and loops (`pathfind/pilot/watch.rs`)

- A setback is a correction that moves the bot more than `movement::SETBACK` (0.05 blocks) from where it
  had simulated itself on that tick. Ulp corrections (docs/DESIGN.md "Off-axis ulp corrections") are not
  setbacks. `Movement::setback_age` exposes it.
- On a setback the pilot drops its path (favoured, not counted as a re-plan), stops the controls and
  holds for `SETTLE_TICKS`, then plans from where the server put the bot. Work cannot start while it holds.
- Every path work step is counted per cell. One issued a 4th time ends the goto with `NotPossible("loop:
  …")` rather than digging forever: before this, a block the server kept restoring re-planned for free
  each time (a world change), bounded only by `give_up_ticks`. `Pilot::work_repeats` and `Pilot::settles`
  report both.

## Injected lag (`lag.rs`, `BotConfig::lag`)

Inbound only: every server packet is held for `delay` + jitter, and a "lost" datagram adds `spike` to it and
to everything behind it (RakNet resends in order, so loss is never a missing packet). The echo round trip
the bot sees includes the whole delay. Outbound packets are not delayed, since `Client::send` is
synchronous. Test and lab use only.

## Reach and line of sight

BDS judges clicks, breaks and hits by distance only; it never casts a ray through blocks. Measured on BDS
1.26.52.3 (2026-10-27, devbox), all judged by server state:

| Drill | Result |
|---|---|
| StartGame | `server_authoritative_block_breaking = true` |
| Place on the far face of a stone behind a wall | placed |
| Mine a stone behind a wall (Start, Continue, Predict, Abort) | broken |
| Place, survival, eye to the clicked block's integer corner 7.10 (centre 7.56) / 7.30 (centre 7.76) | placed / refused, with `UpdateBlock` resends |
| Same in creative, 12.40 / 12.60 | placed / refused |
| Attack a cow, eye to its feet 6.40 / 7.00 | hurt / nothing sent back |

So the limits are 7.2 (survival) and 12.5 (creative) to the block's corner, and 6.7 to an entity's position;
the bot's own `BLOCK_REACH` / `ENTITY_REACH` are stricter on purpose, for vanilla-looking play and proxies that
do raycast. Rerun: `REMOTE_HOST=devbox BDS_DIR=work/bds-raycast BDS_PORT=19230 tools/testbox-bds.sh install && …
start`, then `REMOTE_HOST=devbox tools/remote-run.sh cargo run --release -p acacia-bot --example reach_drills --
127.0.0.1:19230 RayBot [drill,...]` (test pack off: `tools/testbox-bds.sh unpack`).

## Tests

- `interact/confirm_tests.rs`: a toy server with up/down delay, lost actions, late echoes and refusals
  flips a 20-cell row. All actions are confirmed, there are no ghosts, no action takes more than 3 tries,
  refusals end at the echo, and refused predictions are reverted.
- `pathfind/tests/confirm_sim.rs`: a setback holds the bot still, then it re-plans for free and arrives
  (an ulp correction does not hold it); a dig the server keeps undoing ends in a loop error within 600 ticks.
- Live: `examples/confirmed.rs` (strict BDS, test pack off, fresh world), dig and bridge courses:

| Lag (one-way inbound) | Course | Ticks | Actions (retries) | Echo rtt | Corrections | Ghosts | Repeated work |
|---|---|---|---|---|---|---|---|
| none | dig | 106 | 16 (0) | 57 ms | 0 | 0 | 0 |
| none | bridge | 49 | 7 (0) | 98 ms | 3 | 0 | 0 |
| 150 ms ±20, 5% loss (+300 ms) | dig | 106 | 16 (0) | 392 ms | 1 | 0 | 0 |
| 150 ms ±20, 5% loss (+300 ms) | bridge | 56 | 7 (0) | 406 ms | 1 | 0 | 0 |
| 300 ms ±20, 20% loss (+300 ms) | dig | 103 | 16 (0) | 601 ms | 4 | 0 | 0 |
| 300 ms ±20, 20% loss (+300 ms) | bridge | 60 | 7 (0) | 638 ms | 6 | 0 | 0 |

The no-lag rows predate predicted placement. Each lagged run held for one setback; so did the no-lag dig
run, with no correction during the course: the setback was the correction that follows the pad teleport,
within 10 ticks of the start. Live retries never fired, because RakNet is reliable and the adaptive
timeout covers the measured echo. Retries are exercised in the simulation.
