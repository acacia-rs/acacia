# netem

A UDP proxy that makes a link worse: delay with jitter, random loss and a bandwidth cap, each per
direction. It speaks no protocol, so it works for RakNet, NetherNet's UDP and a real client alike.

```
cargo run --release -p acacia-netem -- --listen 127.0.0.1:19196 --to 127.0.0.1:19132 \
    --delay 75ms --jitter 10ms --loss 2 --down-rate 2mbit --down-queue 256k
```

- `up` is client to server, `down` server to client. `--delay`, `--jitter`, `--loss` (percent),
  `--rate` (bits a second: `kbit`, `mbit`) and `--queue` (bytes waiting for the rate: `k`, `m`) set
  both directions; `--up-<option>` / `--down-<option>` set one.
- Each client gets its own socket to the server, so the server sees one peer per client.
- A datagram is lost first, then waits its turn on the capped link (dropped when the queue is full),
  then is delayed; arrivals never overtake each other.
- `--stats N` prints a line every N seconds: per direction the datagrams in, lost, dropped for a full
  queue, sent, kB sent and the largest queue seen. `--for N` stops after N seconds.

## Reading RakNet's recovery

`--capture FILE` writes every datagram with its time, direction and fate. Then run:

```
py -3 tools/netem/raknet_trace.py FILE...
```

For each direction it reports:
- **Datagrams** sent, their size, and how many never reached netem. Those are gaps in the sequence
  numbers, mostly a burst overflowing netem's socket buffer (212 kB on testbox). A real client's
  buffer would drop them the same way.
- **Recoveries:** resends of messages whose last copy netem dropped, and how long they took.
- **Spurious resends:** resends of messages whose last copy got through.
- **ACK and NACK records** the other side sent.

## Timing sapling joins

`tools/netem-pacing.sh <local sapling checkout> [radius]` runs `pacing.sh` on testbox:
- a sapling `drill_server` on generated terrain;
- a bot that holds the view open, since generated columns nobody sees are dropped and made again;
- a warm-up join;
- one `join_timing` bot through netem per profile (clean, lossy, narrow, bad), with
  `raknet_trace.py`'s reading of its capture.

Each prints the time to spawn, to every column of the view and to the last section answer.
`BDS_PORT=19250` also joins a BDS started with `BDS_DIR=work/bds-netem BDS_PORT=19250
tools/testbox-bds.sh start` at each profile. `RAKNET=local` builds sapling against this tree's
acacia-raknet instead of the commit sapling pins, to judge a raknet change before it is pushed.
Touch the changed files if a build ran since you edited them: synced files keep their mtimes.
