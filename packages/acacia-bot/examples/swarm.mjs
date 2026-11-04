// N bots in one process on one server: their joins are spaced (BDS refuses an address that joined moments ago;
// README, Joins), and bots of one host on one server share a single copy of the world's chunks. Each walks to its
// own spot on a ring around the first bot, then the script prints the process RSS and leaves.
// `node examples/swarm.mjs [count]` (default 5; build first: `npm run build`); the server: examples/connection.mjs.
import { NativeTransport, createBot, goals } from '@acacia-rs/bot'

import { connection } from './connection.mjs'

const count = Number(process.argv[2] ?? 5)
const RING = 6
const mib = (bytes) => `${(bytes / 2 ** 20).toFixed(0)} MiB`

const options = connection()
const via = options.via ?? new NativeTransport({ joinSpacingMs: 750, joinJitterMs: 250 })
const baseline = process.memoryUsage().rss
const started = Date.now()
const bots = await Promise.all(
  Array.from({ length: count }, async (_, i) => {
    const bot = await createBot({ ...options, via, username: `Swarm${i}` })
    console.log(`${bot.username} joined after ${((Date.now() - started) / 1000).toFixed(1)} s`)
    await bot.ready()
    return bot
  }),
)

const centre = bots[0].entity.position
const walks = bots.map(async (bot, i) => {
  const angle = (2 * Math.PI * i) / count
  const spot = [Math.floor(centre.x + RING * Math.cos(angle)), Math.floor(centre.z + RING * Math.sin(angle))]
  try {
    await bot.goto(goals.xz(...spot), { signal: AbortSignal.timeout(60_000) })
    return `${bot.username} reached ${spot.join(' ')}`
  } catch (e) {
    return `${bot.username} did not reach ${spot.join(' ')}: ${e.code ?? e.name}`
  }
})
for (const line of await Promise.all(walks)) console.log(line)

const rss = process.memoryUsage().rss
console.log(`rss ${mib(rss)} with ${count} bots: ${mib((rss - baseline) / count)} a bot over the ${mib(baseline)} baseline`)
await Promise.all(bots.map((bot) => bot.close()))
if (via instanceof NativeTransport) await via.close()
