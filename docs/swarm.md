# Swarm (`acacia_bot::swarm`)

Many bots, many servers, one process; the interfaces a coordinator needs to spread bots over nodes.
Example: `crates/acacia-bot/examples/swarm_specs.rs`.

## Model
- `Swarm::builder()…start(async |bot: &mut Bot, state: &mut S| { … })` starts K shard threads (default: one
  per core), each a single-threaded tokio runtime. A bot lives on one shard for its whole life, so the task
  future need not be `Send` (Rust cannot yet bound an async closure's future `Send`).
- The task drives one connection. It returning after a disconnect means "reconnect per the policy"; returning
  while still connected (or after `bot.disconnect()`) ends the bot with `Removed`. `state` survives reconnects.
- One bot does one thing at a time (`&mut Bot`), unlike azalea's task per event, so no re-entrancy guards.
- Handle: `add(spec)`, `remove(id).await` (→ the spec with its current state), `snapshot()`, `events()`
  (broadcast), `drain()`, `shutdown()`.
- Hooks: `bot_config(|spec| …)`, `client(|spec, builder| …)` pick per-bot settings from the spec (e.g. its role
  in `state`). Every bot shares the swarm's `SharedWorlds`, so bots on one server store each chunk once.

## Shards
A new bot goes on the first shard running fewer than `shard_fill` bots (default 100), and on the least-loaded
one only when every shard is that full. Packing is what keeps idle bots cheap: a thread that is asleep costs a
wake-up for every packet and timer, and a busy thread shares one wake-up between many bots. 50 idle bots
against `load_server` (acacia-testserver) took 12.5 s of CPU per 55 s spread over 23 shards and 3.3 s on one,
which was then about 6% busy. Bots whose task does long synchronous work delay their shard's neighbours; give
those a lower `shard_fill`.

## Specs are data
`BotSpec<S> { id, login, target, proxy, state }` is serde, as are `SwarmEvent` and `Snapshot`:
- `id` is chosen by the caller, so it stays unique across nodes.
- `Login::Online { account }` names an account; the node signs it in from its `token_cache`. Share one
  `TokenCache` implementation (e.g. a database) across nodes and any node can run any account (see Accounts).
- `Target::Server { address }` or `Target::Realm { id }` (online only, via `acacia_client::realm_builder`).
- `proxy` (`host:port[:user:pass]`) carries both the game connection and sign-in (one auth client per proxy).

## Reconnect policy (`Policy`, policy.rs)
| Ended by | Default |
|---|---|
| Transport, I/O, timeout, NetherNet failure | Reconnect, backoff 1 s doubling to 60 s, ±25 %; a 60 s session resets it |
| Kick | Reconnect; 3 kicks in a row each within 30 s of spawning → `Failed` (ban / whitelist) |
| `Transfer` | Join the new address next; later reconnects use the spec's target |
| Login refused (outdated, edu, editor) | `Failed`; server full reconnects |
| Auth needing the account holder (`requires_user_action`, OAuth, no tokens) | `Failed` |
| Invalid spec (bad proxy, realm with offline login, no token cache) | `Failed` |
| Account lease busy or lost (see Accounts) | `AccountBusy`, retried with the backoff; not passed to `on_disconnect` |

`Policy::on_disconnect` gets the default `Decision` and may replace it. Failed bots stay listed until removed.

## Accounts
Two live sessions of one account kick each other, and two nodes refreshing its tokens at once can strand a rotated
refresh token. So:
- **Lease** (`AccountLease`, lease.rs): an online bot joins only while it holds its account's lease, holder
  `<node_id>/<bot id>`. It keeps the lease across reconnects, renews it every `lease_ttl / 3` (default TTL 30 s) and
  releases it when removed or failed. Busy → `BotStatus::AccountBusy`, retried with the normal backoff (a move races
  the old node's release). Lost mid-session (renewal refused, or failing until near expiry) → disconnect at once, then
  the same retry path. The default `LocalLeases` covers one process; nodes sharing accounts plug in a shared store
  (Redis `SET NX PX` + a fence counter, or a Postgres row) and the same `node_id` naming. A shard that stalls past the
  TTL can briefly overlap with the new holder; `Lease::fence` lets external writes reject the stale one.
- **Token writes** are compare-and-swap (see docs/auth.md, Cache), so even outside the lease a stale writer loses.

## Joins
Joins to one target are spaced `500 ± 250 ms` across all shards (reconnects too), so a swarm start or a server
restart does not arrive as a burst. Different targets do not wait for each other. Spacing is per node, which is
enough under the placement rule below; per-IP limits are left to one proxy per account plus each bot's backoff.

## Horizontal scaling
The library is the node; placement is the coordinator's job (the AFK service). It gets:
- `snapshot().shard_load` and per-bot status for placement and health.
- Moving a bot: `node_b.add(node_a.remove(&id).await?)`. `remove` returns once the bot is offline and its lease
  released, with `state` as the task left it (failed bots keep theirs too), so the new node joins without an
  `AccountBusy` wait. `S` must be serde to cross the wire. `None` means the bot ended on its own first.
- `drain()` before taking a node away: no new joins; waiting bots park (`BotStatus::Parked`), online ones play on
  until removed (or park after a disconnect). Then `remove` each and `add` it elsewhere.
- Placement rule: keep bots of one server on one node; chunk sharing is per process.
