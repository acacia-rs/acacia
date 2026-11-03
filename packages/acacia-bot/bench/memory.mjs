// Per-bot memory of @acacia-rs/bot: N offline `createBot` bots in this process against one fresh load-server world
// (a stone floor), standing for SETTLE s after the last spawn; then process.memoryUsage() and smaps_rollup.
// N = 0 loads the package and starts a host without a server. Method and the Rust baseline: RESULTS.md.
// usage: node --expose-gc bench/memory.mjs <bots>
// env: PHYSICS=1 ENTITIES=1 SNAPSHOTS=1, SHARDS (host default), JOIN_MS=100, SETTLE=25, CAP_MB=5000, PORT=19640, SAPLING / ADDRESS (common.mjs)
import { emit, env, joinBots, load, log, memoryMb, num, sleep, smapsKb, startServer, statusKb } from './common.mjs'

const n = Number(process.argv[2] ?? 1)
const physics = env('PHYSICS', '1') === '1'
const entities = env('ENTITIES', '1') === '1'
const [joinMs, settle, capMb] = [num('JOIN_MS', 100), num('SETTLE', 25), num('CAP_MB', 5000)]

// Node plus this script, before the package loads.
const bare = { memoryMb: memoryMb(), smapsKb: smapsKb() }
const { createBot, NativeTransport } = await import('@acacia-rs/bot')
const shards = process.env.SHARDS ? Number(process.env.SHARDS) : undefined
const snapshots = env('SNAPSHOTS', '1') === '1'
const transport = new NativeTransport({ shards, snapshots })
await sleep(1000)
const loaded = { memoryMb: memoryMb(), smapsKb: smapsKb() }

const base = { bench: 'memory', bots: n, physics, entities, snapshots, shards: shards ?? null, mallocArenaMax: env('MALLOC_ARENA_MAX', null), joinMs, settleS: settle }
let joined = { spawned: 0, failed: 0, errors: {}, joinS: 0 }
setInterval(() => {
  const rss = process.memoryUsage.rss() / 1048576
  if (rss > capMb) {
    emit({ ...base, capped: true, rssMb: Math.round(rss), spawned: joined.spawned })
    process.exit(3)
  }
}, 500).unref()

const server = n > 0 ? await startServer({ port: num('PORT', 19640) }) : null
let ended = 0
const ends = {}
if (server) {
  log(`memory: ${n} bots, physics ${physics}, entities ${entities}, server pid ${server.pid}`)
  joined = await joinBots(n, joinMs, async (i) => {
    const bot = await createBot({ host: server.host, port: server.port, username: `Mem${i}`, transport: 'raknet', physics, entities, via: transport })
    bot.on('end', (e) => {
      ended++
      const key = `${e.reason}: ${e.message.slice(0, 80)}`
      ends[key] = (ends[key] ?? 0) + 1
    })
    bot.on('error', () => {})
    return bot
  })
  log(`joined ${joined.spawned}/${n} in ${joined.joinS} s`, joined.errors)
  await sleep(settle * 1000)
}

globalThis.gc?.()
const row = {
  ...base,
  spawned: joined.spawned,
  failed: joined.failed,
  joinErrors: joined.errors,
  joinS: joined.joinS,
  ended,
  ends: structuredClone(ends),
  bare,
  loaded,
  memoryMb: memoryMb(),
  statusKb: statusKb(),
  smapsKb: smapsKb(),
  serverRssKb: server ? statusKb(server.pid).VmRSS : null,
  load: load(),
}
log(`rss ${row.memoryMb.rss} MB (heap ${row.memoryMb.heapUsed}, external ${row.memoryMb.external})`)

await Promise.race([transport.close(), sleep(15_000)])
await server?.stop()
emit(row)
process.exit(0)
