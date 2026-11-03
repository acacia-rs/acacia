# Builder iteration speed (2026-11-03)

Where the acacia builder loop (edit `crates/acacia-bot/src/build/**`, unit tests on testbox, live trials on the
BDS pool, read logs) spends its time, what changed, and what was measured and left alone. Branches:
`tools/iteration` (on master) and `tools/iteration-trial` (`build/schematic-trial` + `tools/iteration` + the
trial script).

## Sources

- Transcripts of the schematic-trial agent (`54c86c90…/subagents/agent-a37a79ce08227b834.jsonl`, 10-28 19:28 to
  10-29 23:50, ~203 min active) and the four builder-tier agents of 10-28/29 (a067aba0, a0e38c06, a5fa578e,
  a9659565). Mined with the speed-up-iteration skill's `mine-transcripts.py` plus one-off timelines of every
  background wait (launch, completion notice, first action after it).
- `~/work/acacia-build-trial`: the last full trial's logs and journals (10-29 23:49, code e89f932).
- 164 builder journals under `~/work` (house16 A/B, scenes, trials) for stall gaps.
- Live re-measurement on 2026-11-03 22:30-23:30. The box was loaded throughout: load average 100-115 on 16
  cores, CPU pressure "some" ~80%. Timings below are from that box; compare them with each other, not with
  quiet-box numbers.

## Cost table (trial agent, ~203 min active)

| Step | Wall | Share | What it was |
|---|---|---|---|
| Live trial waits | 102 min | 50% | full 9-structure trials of 16-46 min; 1 smoke run |
| Model time between calls | 47 min | 23% | not tool cost |
| Unit-test waits | 44 min | 22% | one 31-min lock hang + relaunches; the other 18 cycles median 25 s |
| Foreground tools | 9 min | 4% | greps, reads, git |
| Results nobody read | 25 h | (idle) | 10-28 20:15 its waiter was killed with the session; the finished trial sat until a coordinator ping |

90% of its background wall-clock was spent blocked: it held unit tests back while a trial ran, because both
used one remote dir (remote-test.sh wipes the dir it syncs into). The tier agents: 362 min of merged waits over
four agents, 49% blocked; their house16 A/B runs passed 90 of 94 times.

### Inside the steps

- **Trial runs are 60% dead tail.** 8 of 9 structures ended by the stall cut: 3000 ticks without a cell done.
  That tail is 1399 of the run's 2324 game seconds. The longest gap a one-bot trial run recovered from was 258
  ticks; across all 164 journals only two-bot house16 runs recovered from more than 1000 (max 2206).
- **Trial schedule.** `PAR=3` on a 6-instance pool, in argument order, so the 507 s outpost started 7.5 min in.
  A greedy schedule model over that run's per-structure times (+10 s a run; it reproduces the real 16.0 min as
  15.8) gives 8.6 min for largest-first with 6 slots and 6.7 min with a 1200-tick stall cut.
- **Lock hang (10-29 21:41-22:13).** `flock target/.busy` without `-o`: the build started the shared sccache
  server, which inherited the lock fd and kept it. The job never started, so no `test.log`, and remote-test's
  "already running" check (it looks for test.log) let a second launch queue behind it. Known since 10-29
  (sapling switched to `flock -o` in 6c92748); acacia master still had the plain flock.
- **Unit-test cycle.** Launch to the agent acting on the result: median 25 s over 18 cycles, 2 tool calls (a
  DETACH launch and a hand-written `until [ -f test.done ]; do sleep 5|10|15` waiter, 17 of them), often a
  third call to read the failure. Compile is most of it (below).
- **Waits that missed.** Monitor expires after 30 min (re-armed by hand at 23:30); a killed waiter (session
  stop) leaves a finished job unread; a killed guard leaves no `.done`, so every waiter hangs.
- **`tools/bds-pool.sh status` always said free.** It checked `<i>/.lock`, `run` leases `<i>.lock`.

## Measured on 2026-11-03

Launch components (one call each): ssh round trip 0.34 s, `testbox-target.sh` 0.43 s, tar of the worktree 0.63 s
(12 MB).

| Build | Result |
|---|---|
| cold `cargo test -p acacia-bot --lib`, sccache (default) | no progress in 4 min, killed (rustc at 7-10% CPU) |
| same, `REMOTE_ENV="RUSTC_WRAPPER="` | 77 s compile, 86 s total |
| cold release `examples/build`, no sccache | 133 s |
| edit (touch `build/plan.rs`) to verdict, `-- build::`, no sccache, `CARGO_INCREMENTAL=0` (the profile's) | compile 13.6 s, total 19-20 s (medians of 4 and 3) |
| same, `CARGO_INCREMENTAL=1` | compile 4.2 s, total 10.2 s (median of 4) |
| same, remote-test.sh as committed (sets `CARGO_INCREMENTAL=1`) | compile 2.9 s, total 9.5 s (median of 3) |

The first run after switching `CARGO_INCREMENTAL` or the wrapper rebuilds the workspace crates (26-42 s); the
medians leave it out. `~/.profile.d/agents.sh` exports `CARGO_INCREMENTAL=0` for every shell, so sccache can
cache everything; cargo only builds workspace members incrementally, so turning it on for test jobs costs only
sccache hits on workspace crates in a cold build.

### sccache

The shared sccache server (`pgrep -x sccache`, cmdline `/usr/bin/sccache`) ran in `agent-bellprint-*.scope`,
whose 8 GB cap was thrashing: `memory.pressure` full avg10 84-92%, 28 M file refaults. Every compile any session
sends through sccache runs as that server's child, inside that scope. The server restarts after its idle timeout
from whichever session compiles next, so which scope it lands in is luck. Workaround per job:
`REMOTE_ENV="RUSTC_WRAPPER="` (switching the wrapper rebuilds a target's deps once). The fix is the user's call
(it changes a server every session shares; no root needed, but stopping it kills in-flight compiles):

```sh
sccache --stop-server   # when no build is running
systemd-run --user --unit=sccache -p MemoryMax=16G -E SCCACHE_NO_DAEMON=1 -E SCCACHE_IDLE_TIMEOUT=0 \
  -E SCCACHE_CACHE_SIZE=60G /usr/bin/sccache --start-server
```

The server then lives in its own unit with its own cap and never idles out, so clients find it there. A user unit
file in `~/.config/systemd/user/` would make it survive reboots.

### Trial, before and after

Both runs on 2026-11-03, same code (e89f932 + the knob), same loaded box, release binary prebuilt (133 s cold,
no sccache), `python3 tools/trial-times.py ~/work/acacia-iter-live`:

| Run | Whole trial | All but the outpost done | Stalled tails |
|---|---|---|---|
| before: `PAR=3`, argument order, 3000-tick cut | 21.8 min | 14.0 min | ~3000 ticks on 8 of 9 |
| after: `PAR=6`, largest first, `STALL_TICKS=1200` | 13.0 min | 5.2 min | ~1200 ticks on 8 of 9 |

Per structure, game seconds before → after: taiga 196 → 102, desert 204 → 112, savanna 213 → 129, snowy 243 →
146, plains_house 346 → 201, mansion 259 → 170, plains_smith 314 → 299 (it kept placing), ruined_portal 51 →
49 (passes, never stalls). Verdicts (placed and mismatch counts) are the same apart from run-to-run spread, so the
shorter cut lost no result.

The outpost now sets the length: 762 s (after) and 860 s (before) of slow, real progress (1393 and 1417
placed; 970 on 10-29), not a stall. A full trial is outpost + ~1 min; everything else is in by 5 min.

## What changed

`tools/iteration`:
- `tools/testbox-job.sh` (new, testbox side of both remote scripts): `flock -o`; the log says "queued" until the
  lock is free; a lock held past 600 s ends the job with 126 and the holder's pids; a guard that dies without a
  verdict gets a `.done` written, so waiters end.
- `tools/testbox-wait.sh` (new): the one waiter. Prints errors, the failing tests' output and the result lines
  (test) or the log tail (run); exits with the job's code; exits 99 when the dir has no job script. Both remote
  scripts end with it, so a filtered test is one foreground call with the failure text in it.
- `remote-test.sh` / `remote-run.sh`: refuse while the other kind of job is still going in the dir.
  remote-test sets `CARGO_INCREMENTAL=1` (`REMOTE_ENV` can override it).
- `tools/bds-pool.sh status`: checks the lease file `run` takes.
- `examples/support/house.rs`: `STALL_TICKS` (default 3000).
- `tools/iter-bench.sh`: times the unit-test loop (launch, compile, rest; medians). Rerun:
  `RUNS=5 REMOTE_ENV="RUSTC_WRAPPER=" tools/iter-bench.sh`.
- `docs/testing.md`: how to wait (foreground for a filtered test; `DETACH=1` + `testbox-wait.sh` in the
  background otherwise; not Monitor, no hand loops) and a `-live` dir for live runs.

`tools/iteration-trial` (for `build/schematic-trial`):
- `tools/schematic-trial.sh`: `PAR` defaults to the pool size, largest file first, `STALL_TICKS=1200`, bare
  names (`tools/schematic-trial.sh taiga_house`) and no argument for all of `~/work/schematics/vanilla`.
- `tools/trial-times.py ~/work/<dir>`: build time, each run's game seconds and stalled tail, the whole run.

## Edit to result

| Loop | Before | After |
|---|---|---|
| unit test after one edit (filtered) | median 25 s and 2-3 tool calls (transcripts); 19-20 s measured today with the old compile settings | 9.5 s, one foreground call with the failure text |
| unit test while the sccache server's scope thrashes | unbounded (4 min with no progress) | same unless `REMOTE_ENV="RUSTC_WRAPPER="` or the sccache unit |
| test queued behind a leaked lock | 31 min, silent, two relaunches | cannot leak (`-o`); otherwise 126 after 10 min naming the holder |
| one structure (taiga_house) | ~196 s game + ~20 s build/start | ~102 s + ~20 s |
| full 9-structure trial | 21.8 min (16.0 on 10-29) | 13.0 min; 5.2 min without the outpost |
| unit tests during a trial | held back (one shared dir) | run alongside (`-live` dir) |

Rerun the measurements: `RUNS=5 REMOTE_ENV="RUSTC_WRAPPER=" tools/iter-bench.sh` and
`python3 tools/trial-times.py ~/work/<live dir>` after a trial.

## Measured, left alone

- Sync (tar + ssh, 1.4 s together) and BDS reset/start per run (~10 s): small next to builds and runs.
- house16 A/B (`build-ab.sh`): 90 of 94 runs pass, so the stall cut rarely fires; two bots need the 3000-tick
  cut anyway (recovered from 2206).
- Scenes (`scenes.sh`): already parallel on the pool since 10-28 (full check 114 s then).
- An incremental release profile: measured equal to `--release` on 10-28.
- testbox-target eviction of queued jobs (sapling ea24001 added a 3 h grace after it hung pollers): acacia's
  done files live outside `target`, so an eviction costs a rebuild, not a hung waiter.
- Debug round trips on sim tests (DBG `eprintln!` reruns): each is one ~25 s cycle now; a journal dump in the sim
  harness would cut their number, but that is builder work, not tooling.
