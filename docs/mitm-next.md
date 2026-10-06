# acacia-mitm: what is left

State on 2026-10-05. How the proxy works is in [DESIGN.md](DESIGN.md) and the crate's module docs;
this is the work list. On master: session-tagged captures, transfer following over RakNet,
`sent`/`dropped` capture events, `on_login`/`on_close` hooks, `Session::proxy`, sign-in off the
RakNet loop, and `capcheck`.

## 1. Check what was only tested against the fake server

Everything above passed the crate's suites (acacia client through the proxy to `FakeServer`).
None of it has met the vanilla client, a real server or an online account. Each needs the test BDS
or a signed-in game, so one at a time:

- **Transfer with the vanilla client.** Does the game rejoin the proxy from a new port inside the
  30 s window (`transfer.rs`), and does it accept an IP where the server sent a hostname?
- **Transfer on a real network** (`--online`): hub to game server and back, credentials fetched
  per hop.
- **`--pack-cdn` from another device**: the URL now uses `Session::proxy`, not 127.0.0.1.
- **`capcheck` on real captures.** It was only run on a hand-made file; no `.jsonl` captures were
  on the machine. Expect re-encode differences that are not bugs (non-canonical varints, trailing
  bytes) and decide per packet whether to fix the codec or list an exception.

## 2. Realms and friend worlds (decision open)

The proxy reaches only direct-connect servers. `acacia-client` can dial a Realm or a friend's
world (`realm_builder`, `friend_builder`), but only as a whole `Client` that logs in and runs the
session itself; signaling, trickle ICE, TURN and keepalive live in its driver.

Proposed, not agreed:

- `acacia-client` gains a raw link: dial a `SignalingTarget`, hand back undecoded batches. This
  adds public API there and touches the driver, so it is that area's change.
- `Relay` takes a wire per side. Today `Relay::new` builds the game side and the upstream codec
  from one `Wire` (`relay.rs`); RakNet game side with a NetherNet upstream needs them apart.
- The game keeps joining the proxy by IP over RakNet; `--realm <id>` or `--friend <gamertag>`
  picks the upstream.

The alternative is to leave the proxy at direct-connect servers.

## 3. Transfer following: known limits

- **NetherNet is not followed.** A Transfer passes through and the game leaves the proxy.
- **A door per game.** Each game is sent back to a port of its own (`transfer.rs`), so the proxy
  must be reachable on ports beyond the one it listens on: fine on a LAN, not behind a single
  forwarded port. A fixed port range to forward is not offered yet.
- **Untried with the vanilla client**: whether it rejoins through the door inside the 30 s window.
- **Every hop signs in afresh.** A new key per connection never matches the credential cache
  (`acacia-auth/src/online/client.rs`), so each join and transfer is a full fetch from
  rate-limited endpoints.
- **Only a server's Transfer is followed.** One sent through the `Injector`, or made by an
  interceptor from another packet, reaches the game unchanged.
- The proxy's own rewrite of a Transfer shows in the capture as the `transfer` event, not as
  `dropped` + `sent`.

## Decided against

- A packet ignore list while recording: the capture keeps everything; filter when reading
  (`capdump --only/--exclude`).
- A client limit.
