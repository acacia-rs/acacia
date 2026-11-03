// End-to-end event delivery through @acacia-rs/bot: N bots against a fresh load-server world with `physicsTick`,
// `entitySpawn`/`entityGone` and `entityMoved` listeners, measured over a window after the joins (RESULTS.md).
// usage: node bench/events.mjs <bots>
// env: CLOCKS=225 MOBS=300 (the world), JOIN_MS=100, SETTLE=10, WINDOW=20 (s), PORT=19620, SHARDS (host default),
//      PROBE_GAP=10 (ms between round-trip probes), READ=none|hot|entities|positions, SAPLING / ADDRESS (common.mjs)
import { monitorEventLoopDelay } from 'node:perf_hooks'

import { createBot, NativeTransport } from '@acacia-rs/bot'

import { emit, env, joinBots, load, log, memoryMb, num, percentiles, pidTicks, round, sleep, startServer, threadCpu, threadTicks } from './common.mjs'

const n = Number(process.argv[2] ?? 10)
const world = { clocks: num('CLOCKS', 225), mobs: num('MOBS', 300) }
const [joinMs, settle, window, probeGap] = [num('JOIN_MS', 100), num('SETTLE', 10), num('WINDOW', 20), num('PROBE_GAP', 10)]
const shards = process.env.SHARDS ? Number(process.env.SHARDS) : undefined

const server = await startServer({ port: num('PORT', 19620), ...world })
const transport = new NativeTransport({ shards })

// Batches as the session receives them (one per onRecords call, after the addon's JSON.parse).
const wire = { batches: 0, records: 0 }
const open = transport.open.bind(transport)
transport.open = (listener) =>
  open({
    records: (records) => {
      wire.batches++
      wire.records += records.length
      listener.records(records)
    },
    closed: () => listener.closed(),
  })

const counts = { physicsTick: 0, entitySpawn: 0, entityGone: 0, entityMoved: 0, movedEntities: 0, lagged: 0, laggedDropped: 0, end: 0 }
/** Every `end` since the first join, by reason (not reset with the window counts). */
const ends = {}
const started = performance.now()
const reset = () => {
  for (const k of Object.keys(counts)) counts[k] = 0
  wire.batches = wire.records = 0
}

// READ=hot: every physicsTick listener also reads bot.hot (a one-snapshot-per-tick consumer); READ=entities adds
// bot.entities too, and READ=positions every entity's position.
const read = env('READ', 'none')
let readSink = 0
function listen(bot, first) {
  bot.on('physicsTick', () => {
    counts.physicsTick++
    if (read !== 'none') readSink += bot.hot?.tick ?? 0
    if (read === 'entities') readSink += Object.keys(bot.entities).length
    if (read === 'positions') for (const e of Object.values(bot.entities)) readSink += e.position.x
  })
  bot.on('entitySpawn', () => counts.entitySpawn++)
  bot.on('entityGone', () => counts.entityGone++)
  bot.on('entityMoved', (e) => {
    counts.entityMoved++
    counts.movedEntities += e.entities.length
  })
  bot.on('end', (e) => {
    counts.end++
    const key = `${e.reason}: ${e.message.slice(0, 80)}`
    ends[key] ??= { count: 0, firstS: round((performance.now() - started) / 1000, 1) }
    ends[key].count++
  })
  // A `lagged` record goes to every bot: count it once.
  if (first) {
    bot.on('lagged', (e) => {
      counts.lagged++
      counts.laggedDropped += e.dropped
    })
  }
  bot.on('error', () => {})
  return bot
}

log(`events: ${n} bots, world ${JSON.stringify(world)}, server pid ${server.pid}`)
const joined = await joinBots(n, joinMs, async (i) =>
  listen(await createBot({ host: server.host, port: server.port, username: `Ev${i}`, transport: 'raknet', via: transport }), i === 0),
)
log(`joined ${joined.spawned}/${n} in ${joined.joinS} s`, joined.errors)
await sleep(settle * 1000)

/** Sequential calls round-robin over the bots until `until`, `probeGap` ms apart; ms per call. */
async function probe(call, until) {
  const ms = []
  const errors = {}
  const failedBots = new Set()
  for (let i = 0; performance.now() < until; i++) {
    const bot = joined.bots[i % joined.bots.length]
    const t0 = performance.now()
    try {
      await call(bot)
      ms.push(performance.now() - t0)
    } catch (e) {
      const key = e.code ?? e.name
      errors[key] = (errors[key] ?? 0) + 1
      failedBots.add(bot.username)
    }
    await sleep(probeGap)
  }
  return { calls: ms.length, errors, failedBots: failedBots.size, ...Object.fromEntries(Object.entries(percentiles(ms, [50, 99])).map(([k, v]) => [k, v === null ? null : round(v, 3)])) }
}

reset()
const delay = monitorEventLoopDelay({ resolution: 1 })
delay.enable()
const threads0 = threadTicks()
const cpu0 = process.cpuUsage()
const server0 = pidTicks(server.pid)
const t0 = performance.now()
const until = t0 + window * 1000
const [immediate, queued] = joined.spawned
  ? await Promise.all([probe((b) => b.clearControlStates(), until), probe((b) => b.waitForTicks(1), until)])
  : [null, null]
const seconds = (performance.now() - t0) / 1000
const cpu = process.cpuUsage(cpu0)
const threads = threadCpu(threads0, threadTicks(), seconds)
const serverCpu = server.pid ? round((pidTicks(server.pid) - server0) / seconds) : null
delay.disable()
const window_ = { ...counts, ...wire }

const perS = (v) => round(v / seconds)
const events = window_.physicsTick + window_.entitySpawn + window_.entityGone + window_.entityMoved
const row = {
  bench: 'events',
  bots: n,
  ...world,
  shards: shards ?? null,
  read,
  spawned: joined.spawned,
  failed: joined.failed,
  joinErrors: joined.errors,
  joinS: joined.joinS,
  windowS: round(seconds, 2),
  eventsPerS: perS(events),
  perS: {
    physicsTick: perS(window_.physicsTick),
    entitySpawn: perS(window_.entitySpawn),
    entityGone: perS(window_.entityGone),
    entityMoved: perS(window_.entityMoved),
    movedEntities: perS(window_.movedEntities),
    records: perS(window_.records),
    batches: perS(window_.batches),
  },
  ticksPerBotS: joined.spawned ? round(window_.physicsTick / seconds / joined.spawned, 2) : null,
  recordsPerBatch: window_.batches ? round(window_.records / window_.batches) : null,
  lagged: window_.lagged,
  laggedDropped: window_.laggedDropped,
  ended: window_.end,
  ends: structuredClone(ends),
  cpuPct: { js: threads.js, process: round(((cpu.user + cpu.system) / 1e6 / seconds) * 100), server: serverCpu, threads },
  jsUsPerEvent: events ? round(((threads.js / 100) * seconds * 1e6) / events, 2) : null,
  // The histogram holds the 1 ms sampling timer's interval: the delay is what exceeds 1 ms.
  loopDelayMs: Object.fromEntries([['p50', delay.percentile(50)], ['p99', delay.percentile(99)], ['max', delay.max]].map(([k, v]) => [k, round(Math.max(0, v / 1e6 - 1), 2)])),
  rttMs: { immediate, waitForTicks1: queued },
  memoryMb: memoryMb(),
  load: load(),
  env: Object.fromEntries(['CLOCKS', 'MOBS', 'JOIN_MS', 'SETTLE', 'WINDOW', 'SHARDS', 'PROBE_GAP'].filter((k) => env(k)).map((k) => [k, env(k)])),
}
log(`window ${row.windowS} s: ${row.eventsPerS} events/s, js ${threads.js}% cpu, loop delay p99 ${row.loopDelayMs.p99} ms`)

await Promise.race([transport.close(), sleep(15_000)])
await server.stop()
emit(row)
process.exit(0)
