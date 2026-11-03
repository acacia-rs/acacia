# @acacia/bot

Minecraft Bedrock bots for Node.js (≥ 22), running on acacia's Rust client in-process through a Node-API addon
(`crates/acacia-node`). Pre-release: nothing is published yet.

```js
import { createBot, goals } from '@acacia/bot'

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
npm test             # node:test: test/*.test.mjs over a fake transport (test/fake.mjs), e2e.test.mjs over FakeServer
```

Benchmarks (`bench:events`, `bench:reads`, `bench:memory`) and their recorded results: see `bench/RESULTS.md`.

The end-to-end tests skip when the addon was built without `TestServer`. `src/generated/` is copied from
`crates/acacia-api/generated` by `scripts/sync-types.mjs` on every build; edit the Rust schema, never the copy.

## Contract

The one place the TS layer's contract is written down (the wire format: acacia-api's crate docs; the host:
acacia-host's; the addon: acacia-node's).

- **Layers.** A `Transport` moves commands out and record batches in. `NativeTransport` runs an acacia host in this
  process; a WebSocket transport will implement the same interface. A `Session` per transport numbers calls,
  settles each from its `done` record and routes events to bots. `createBot` without `via` shares one
  process-wide `NativeTransport`; `closeAll()` shuts it down.
- **Calls** resolve or reject from their `done` record, which comes after every event the bot raised before the call
  ended: listeners see what happened during `goto` before `await bot.goto()` returns. A refusal (`BUSY`,
  `UNKNOWN_BOT`, `CLOSED`, ...) rejects at once. `goto`, `follow`, `lookAt`, `attack`, `equip`, `placeBlock`, `dig`,
  `craft`, `recipesFor`, the container verbs, `smelt`, `consume`, `toss`, `answerForm` and `waitForTicks` queue per bot and run in call order; `chat`, `command`, `setEvents`,
  `setControlState`, `clearControlStates`, `snapshot` and `close` run at once, mid-action too.
  `bot.request(method, params, options)` calls any table method by name with the bot's id filled in, for generic
  callers such as `@acacia/mcp`.
- **Movement.** `goto(goal)` takes a plain `Goal`: the classes `GoalBlock`, `GoalNear`, `GoalXZ`, `GoalY`,
  `GoalAway` and `GoalCompositeAny` (mineflayer-pathfinder's names) or the shorthands `goals.near(...)` etc. build
  one. `follow(entity, { range, signal })` keeps within `range` (default 2) of an entity until aborted; it never
  resolves on its own. Entity arguments take an `Entity` or its `id`. Control states are held keys of a physics bot:
  a running movement action (`goto`, `follow`) overrides them, and every queued call releases them when it ends.
- **Cancellation.** An aborted `signal` sends `cancel` for the call; the promise rejects with `signal.reason`
  (`AbortError`, or `TimeoutError` from `AbortSignal.timeout`) once the bot has stopped. A signal that is already
  aborted rejects without sending anything. `createBot` cannot be cancelled yet.
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
  where the transport offers `reads` (null or empty otherwise, and before the first tick). `bot.hot` is one frozen
  object per tick; `cold`, `entities` and `players` are one frozen object per change, rebuilt at most once a tick
  except that a spawn or removal is published before its `entitySpawn` / `entityGone` event, so a listener finds
  `bot.entities` agreeing with the event.
  With `NativeTransport`, a "change" for `entities` and `players` is an entity coming or going (or its kind or name
  changing). Moves don't rebuild them: their entities' `position`, `velocity`, `yaw`, `pitch` and `health` are
  getters on a typed array that the addon overwrites in place when anything moved, so reading `bot.entities` and
  every `.position` each tick costs one copy and no JSON or allocation. The getters show the numbers as of the latest read of
  `bot.entities`, `bot.players` or `bot.nearestEntity`. An entity keeps its object while it stays in view; once it
  has left, the object keeps its last numbers.
  Each `position` read is a fresh object, and the numbers are the wire's f32s widened (10.100000381469727, where
  events show 10.1).
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
  it has bots and exits once they are closed.

Not yet: `signIn`, reconnects, worker_threads (one host per env, untested).
