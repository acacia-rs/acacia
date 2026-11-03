# @acacia-rs/bot

Minecraft Bedrock bots for Node.js (≥ 22), running on acacia's Rust client in-process through a Node-API addon
(`crates/acacia-node`), or on an acacia daemon over WebSocket (`crates/acacia-daemon`) with the same API.
Pre-release: nothing is published yet.

```js
import { createBot, goals } from '@acacia-rs/bot'

const bot = await createBot({ host: '127.0.0.1', port: 19132, username: 'Sprout' })
bot.on('chat', async ({ sender, plain }) => {
  if (plain === '!ping') await bot.chat('pong')
  const target = bot.players[sender]?.entity
  if (plain === '!come' && target) {
    const { x, y, z } = target.position
    await bot.goto(goals.near(Math.floor(x), Math.floor(y), Math.floor(z), 2), { signal: AbortSignal.timeout(30_000) })
  }
})
```

`examples/hello.mjs` is the full hello world: `node examples/hello.mjs 127.0.0.1:19132 Sprout`.

## Build and test

```sh
npm install
npm run build        # release addon (napi build, cargo -j 2, this crate only) + tsc
npm run build:test   # debug addon with the `test-server` feature (TestServer for the end-to-end tests) + tsc
npm test             # node:test: test/*.test.mjs over a fake transport (test/fake.mjs), e2e.test.mjs over FakeServer,
                     # ws.test.mjs over a stand-in daemon, ws-e2e.test.mjs over target/debug/acacia-daemon
                     # (cargo build -p acacia-daemon -j 2; ACACIA_DAEMON=<path> picks another binary)
```

Benchmarks (`bench:events`, `bench:reads`, `bench:memory`) and their recorded results: see `bench/RESULTS.md`.

The end-to-end tests skip when the addon was built without `TestServer`. `src/generated/` is copied from
`crates/acacia-api/generated` by `scripts/sync-types.mjs` on every build; edit the Rust schema, never the copy.

## Contract

The one place the TS layer's contract is written down (the wire format: acacia-api's crate docs; the host:
acacia-host's; the addon: acacia-node's).

- **Layers.** A `Transport` moves commands out and record batches in. `NativeTransport` runs an acacia host in this
  process; `WsTransport` talks to an acacia daemon (below). A `Session` per transport numbers calls,
  settles each from its `done` record and routes events to bots. `createBot` without `via` shares one
  process-wide `NativeTransport`; `closeAll()` shuts it down.
- **Calls** resolve or reject from their `done` record, which comes after every event the bot raised before the call
  ended: listeners see what happened during `goto` before `await bot.goto()` returns. A refusal (`BUSY`,
  `UNKNOWN_BOT`, `CLOSED`, ...) rejects at once. `goto`, `follow`, `lookAt`, `attack`, `equip`, `placeBlock`, `dig`,
  `craft`, `recipesFor`, the container verbs, `smelt`, `consume`, `toss`, `answerForm` and `waitForTicks` queue per bot and run in call order; `chat`, `command`, `setEvents`,
  `setControlState`, `clearControlStates`, `snapshot` and `close` run at once, mid-action too.
  `bot.request(method, params, options)` calls any table method by name with the bot's id filled in, for generic
  callers such as `@acacia-rs/mcp`.
- **Movement.** `goto(goal)` takes a plain `Goal`: the classes `GoalBlock`, `GoalNear`, `GoalXZ`, `GoalY`,
  `GoalAway` and `GoalCompositeAny` (mineflayer-pathfinder's names) or the shorthands `goals.near(...)` etc. build
  one. `follow(entity, { range, signal })` keeps within `range` (default 2) of an entity until aborted; it never
  resolves on its own. Entity arguments take an `Entity` or its `id`. Control states are held keys of a physics bot:
  a running movement action (`goto`, `follow`) overrides them, and every queued call releases them when it ends.
- **Cancellation.** An aborted `signal` sends `cancel` for the call; the promise rejects with `signal.reason`
  (`AbortError`, or `TimeoutError` from `AbortSignal.timeout`) once the bot's action has ended, within a tick. Keys
  are released then, but a moving bot keeps its momentum: a cancelled `goto` or `follow` coasts about 1.5-2 blocks
  over some 8 ticks before it stands still. A signal that is already aborted rejects without sending anything.
  `createBot` cannot be cancelled yet.
- **Pacing.** Screen actions move at a player's pace: a `craft` takes 3-6 s, container verbs a few seconds
  (acacia-bot's `human.rs`). `ACACIA_HUMAN_SCALE=0.1` in the environment scales every such delay (0 removes them).
- **Joins.** Joins to one address are spaced about 0.5 s apart and BDS's `RecentlyConnected` refusal is retried
  (acacia-host's crate docs), so many `createBot` calls at once just take longer.
- **Errors** are `AcaciaError` with a `code` from acacia-api's `ErrorCode`: `ConnectError` for a failed `createBot`,
  `ActionError` for `TIMEOUT`, `DISCONNECTED`, `REJECTED`, `NOT_POSSIBLE`, `NOT_VISIBLE`, `CANCELLED`, the base class
  for host codes (`BUSY`, `UNKNOWN_BOT`, `BAD_PARAMS`, `CLOSED`, `INTERNAL`, ...). `message` is free text.
- **Events.** `Bot` is a typed `EventEmitter`; each event's payload is the record's event object (`type` included),
  except `error`, which carries an `AcaciaError` (a warning when nobody listens). A group (`chat`, `forms`, `health`,
  `players`, `windows`, `titles`, `sleep`, `ticks`, `entities`, `entityMoves`) is reported while it has a listener or
  was forced with `createBot({ events })`; changes go out once per microtask. `spawn`, `end`, `lagged` and `error`
  always come. `entitySpawn` / `entityGone` carry an `Entity`; `entityMoved` is high-rate, one event per tick batch
  listing every entity that moved (`entities`), and is lossy like `physicsTick`.
  Events a bot raises before `createBot` resolves are not delivered; records after it wait one macrotask so the
  caller can attach listeners. `bot.events(name, { signal })` iterates one event and ends with the bot. A listener
  that throws does not stop the batch; its error is rethrown as an uncaught exception afterwards.
- **Sync reads.** `bot.hot`, `bot.entity`, `bot.health`, `bot.food`, `bot.cold`, `bot.entities`, `bot.players`,
  `bot.nearestEntity(filter?)` and `bot.blockAt(pos)` read the bot's published state and terrain without a call,
  where the transport offers `reads` (null or empty otherwise, and before the first tick: `createBot` resolves at
  spawn, and `waitForTicks(n)` is a timed pause that can end before that tick, so loop on it until `bot.hot` is set). `bot.hot` is one frozen
  object per tick; `cold` and `players` are one frozen object per change (`players`: an entity or player-list
  entry coming or going), rebuilt at most once a tick except that a spawn or removal is published before its
  `entitySpawn` / `entityGone` event, so a listener finds `bot.entities` agreeing with the event.
  **Entities are kept by deltas.** `bot.entities` is one object per bot, updated in place (read it, don't write
  it; spread it to keep a copy). Each read applies the changes since the previous one: entities that came or went,
  and the numbers of those that moved, so a read costs what changed, not the list. An entity keeps its object
  while it stays in view; its `position`, `velocity`, `yaw`, `pitch` and `health` are getters on a typed array
  kept by slot, showing the numbers as of the latest read of `bot.entities`, `bot.players` or `bot.nearestEntity`.
  Once it has left, the object keeps its last numbers. `nearestEntity` reads the array without building positions.
  In-process the addon writes moved entities' numbers straight into that array; over `WsTransport` the daemon
  sends the same deltas (format: acacia-daemon's crate docs) and `EntityTable`, exported for custom transports,
  applies both. A reader that fell behind the bot's change log gets a full resync, which keeps the objects of
  entities still there.
  Each `position` read is a fresh object (caching them measured slower: `bench/RESULTS.md`), and in-process the
  numbers are the wire's f32s widened
  (10.100000381469727, where events and `WsTransport` show 10.1).
  Entities need `createBot({ entities })` (default on); their `id` is the decimal-string runtime id actions take,
  and `position` is the feet, players included. `bot.players` is keyed by name: the player list joined with player
  entities in view (`entity`, null when out of view), plus players seen only in view (`info` null). Non-physics bots
  see only the chunks around them.
- **Inventory and blocks.** `bot.inventory` is the non-empty stacks of the bot's own windows (`window` 0 is the main
  inventory, slots 0-8 the hotbar; 119 off hand, 120 armour), each with `networkId`, `count` and `name` from the
  server's item table (sent once, at login; `name` is null before it). `bot.heldItem` is the stack in the selected
  hotbar slot. `bot.findBlocks({ matching, maxDistance = 16, count = 1 })` returns the nearest loaded cells whose
  block has any of the names (any state), nearest first, synchronously; `maxDistance` is capped at 64 and `count`
  at 1024 (the `bot.findBlocks` method is the same read for transports without sync reads). `equip(item,
  destination = 'hand')` moves an item there; `placeBlock(referenceBlock, face = 'up')` resolves once the server
  shows a block in the target cell (`REJECTED` after three tries). Item arguments take a name (`stone` or
  `minecraft:stone`) or an `inventory` entry throughout.
- **Crafting.** `craft(item, count = 1, { table })` crafts through the recipe book from the inventory, in the own
  2x2 grid or at the crafting table at `table` (within reach), and resolves with how many items were made: at least
  `count` (a craft makes the recipe's whole output), fewer only when the ingredients ran out after one craft.
  `NOT_POSSIBLE` when no recipe fits the grid or ingredients are missing. `recipesFor(item)` lists the server's
  recipes for an item, ingredients summed per item or `#tag`.
- **Containers** are stateless: `containerContents(pos)`, `deposit(pos, item, count?)` and `withdraw(pos, item,
  count?)` each open the container block at `pos` (within reach), act and close it, also on failure. The moves
  resolve with how many items moved (up to `count`, all when absent): fewer once the source has no more or the
  other side is full; `NOT_POSSIBLE` when nothing could move.
- **Survival items.** `smelt(furnace, item, count = 1, { fuel, fuelCount })` runs a furnace, blast furnace or smoker
  to the end and resolves with how many came out; `consume(item?)` eats or drinks (`NOT_POSSIBLE` when not
  hungry); `toss(item, count?, { toward })` throws stacks, aimed at a point when given.
- **Process lifetime.** The addon never holds Node open by itself. The session holds it while a bot is live or a
  call is unanswered, and a closing `NativeTransport` holds it until its stream ends, so `node script.js` runs while
  it has bots and exits once they are closed. A `WsTransport`'s open socket holds Node, so it closes the socket when
  the session no longer wants it held and reconnects (a new `hello`) on the next command.
- **Over acacia-daemon** (`acacia-daemon --listen 127.0.0.1:19180 [--token T]`; protocol: its crate docs).
  `createBot({ ..., via: 'ws://127.0.0.1:19180', token })` runs the bot there; one `WsTransport` (Node's global
  `WebSocket`) is shared per URL and token, and `closeAll()` disconnects them. The API is the same, with these
  differences:
  - Refusals (`BUSY`, `UNKNOWN_BOT`, ...) reject asynchronously, and `VERSION_MISMATCH` / `UNAUTHORIZED` from the
    handshake fail every queued call. An unreachable daemon fails `createBot` with `ConnectError` `IO`.
  - The getters read a mirror the daemon pushes (`hello { mirror: true }`, `new WsTransport(url, { mirror: false })`
    turns it off): `hot`, `entity`, `health`, `food`, `cold`, `inventory`, `heldItem`, `entities`, `players` and
    `nearestEntity` work, at most a tick old and never older than the event or `await` being handled. `entities` is
    kept by deltas as in-process. `blockAt` and `findBlocks` throw
    `NOT_POSSIBLE`: terrain is not mirrored; `await bot.request('bot.findBlocks', query)` does the search.
  - A bot belongs to its connection and closes when it drops, unless created with `persist: true` (ignored
    in-process). `attachBot({ via, username })` (or `bot`) takes over a persisted bot from a new script; events raised
    while nobody was attached are lost, and the mirror starts over. When the connection drops, open calls fail
    `DISCONNECTED`, each of its bots gets a synthesized `end` (`lost`, or `closed` after `close()`), and the
    transport is spent: the next `createBot` with the same URL opens a new one. No automatic reconnect.

Not yet: `signIn`, reconnects, worker_threads (one host per env, untested), `@acacia-rs/mcp` over `WsTransport`
(it would need `findBlocks` as a call).
