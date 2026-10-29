# @acacia/bot

Minecraft Bedrock bots for Node.js (≥ 22), running on acacia's Rust client in-process through a Node-API addon
(`crates/acacia-node`). Pre-release: nothing is published yet.

```js
import { createBot, goals } from '@acacia/bot'

const bot = await createBot({ host: '127.0.0.1', port: 19132, username: 'Sprout' })
bot.on('chat', async ({ sender, plain }) => {
  if (plain === '!ping') await bot.chat('pong')
  if (plain === '!come') await bot.goto(goals.near(0, 64, 0, 2), { signal: AbortSignal.timeout(30_000) })
})
```

`examples/hello.mjs` is the full hello world: `node examples/hello.mjs 127.0.0.1:19132 Sprout`.

## Build and test

```sh
npm install
npm run build        # release addon (napi build, cargo -j 2, this crate only) + tsc
npm run build:test   # debug addon with the `test-server` feature (TestServer for the end-to-end tests) + tsc
npm test             # node:test: test/session.test.mjs (fake transport) and test/e2e.test.mjs (FakeServer)
```

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
  `UNKNOWN_BOT`, `CLOSED`, ...) rejects at once. `goto`, `dig`, `answerForm` and `waitForTicks` queue per bot and run
  in call order; `chat`, `command`, `setEvents`, `snapshot` and `close` run at once, mid-action too.
- **Cancellation.** An aborted `signal` sends `cancel` for the call; the promise rejects with `signal.reason`
  (`AbortError`, or `TimeoutError` from `AbortSignal.timeout`) once the bot has stopped. A signal that is already
  aborted rejects without sending anything. `createBot` cannot be cancelled yet.
- **Errors** are `AcaciaError` with a `code` from acacia-api's `ErrorCode`: `ConnectError` for a failed `createBot`,
  `ActionError` for `TIMEOUT`, `DISCONNECTED`, `REJECTED`, `NOT_POSSIBLE`, `NOT_VISIBLE`, `CANCELLED`, the base class
  for host codes (`BUSY`, `UNKNOWN_BOT`, `BAD_PARAMS`, `CLOSED`, `INTERNAL`, ...). `message` is free text.
- **Events.** `Bot` is a typed `EventEmitter`; each event's payload is the record's event object (`type` included),
  except `error`, which carries an `AcaciaError` (a warning when nobody listens). A group (`chat`, `forms`, `health`,
  `players`, `windows`, `titles`, `sleep`, `ticks`) is reported while it has a listener or was forced with
  `createBot({ events })`; changes go out once per microtask. `spawn`, `end`, `lagged` and `error` always come.
  Events a bot raises before `createBot` resolves are not delivered; records after it wait one macrotask so the
  caller can attach listeners. `bot.events(name, { signal })` iterates one event and ends with the bot. A listener
  that throws does not stop the batch; its error is rethrown as an uncaught exception afterwards.
- **Sync reads.** `bot.hot`, `bot.entity`, `bot.health`, `bot.food` and `bot.blockAt(pos)` read the bot's
  per-tick state and terrain without a call, where the transport offers `reads` (null otherwise, and before the
  first tick). `bot.hot` is one frozen object per tick. Non-physics bots see only the chunks around them.
- **Process lifetime.** The addon never holds Node open by itself. The session holds it while a bot is live or a
  call is unanswered, and a closing `NativeTransport` holds it until its stream ends, so `node script.js` runs while
  it has bots and exits once they are closed.

Not yet: entity and player positions, `signIn`, `findBlocks`, reconnects, worker_threads (one host per env, untested).
