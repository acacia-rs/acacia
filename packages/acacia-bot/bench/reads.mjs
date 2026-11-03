// ns per sync read on one bot against FakeServer (a recorded BDS spawn, plus a floor, ENTITIES pigs and SLOTS
// stacks): a loop runner for the cached path, and a poller that times the first read after the value moved.
// Needs a release addon with TestServer: `npm run build:bench`. Method: RESULTS.md.
// usage: node bench/reads.mjs   env: ROUND_MS=200, ROUNDS=7, ENTITIES=20, SLOTS=20, SAMPLE_S=10
import * as native from '../native/index.js'
import { createBot, NativeTransport } from '@acacia/bot'

import { emit, load, log, median, num, percentiles, round, sleep } from './common.mjs'

if (!native.TestServer) {
  console.error('the addon was built without TestServer: npm run build:bench')
  process.exit(2)
}
const [roundMs, rounds, entityCount, slots, sampleS] = [num('ROUND_MS', 200), num('ROUNDS', 7), num('ENTITIES', 20), num('SLOTS', 20), num('SAMPLE_S', 10)]

const server = native.TestServer.start()
const transport = new NativeTransport()
const bot = await createBot({ host: '127.0.0.1', port: server.port, username: 'Reader', transport: 'raknet', via: transport })
server.layFloor(8)
server.serveInventory()
const [fx, fy, fz] = server.spawnFeet().map(Math.floor)
const ITEMS = ['stone', 'dirt', 'oak_planks', 'cobblestone', 'torch', 'apple', 'bread', 'iron_ingot', 'coal', 'stick']
for (let s = 0; s < slots; s++) server.give(s, `minecraft:${ITEMS[s % ITEMS.length]}`, 1 + s)
for (let i = 0; i < entityCount; i++) server.addEntity('minecraft:pig', 5000 + i, fx + (i % 5) * 2 - 4 + 0.5, fy, fz + Math.floor(i / 5) * 2 - 4 + 0.5)

const deadline = Date.now() + 20_000
while (Object.keys(bot.entities).length < entityCount || bot.inventory.length < slots || !bot.hot || !bot.blockAt({ x: fx, y: fy - 1, z: fz })) {
  if (Date.now() > deadline) throw new Error(`setup: ${Object.keys(bot.entities).length} entities, ${bot.inventory.length} stacks, hot ${!!bot.hot}`)
  await sleep(50)
}
log(`ready: ${Object.keys(bot.entities).length} entities, ${bot.inventory.length} stacks, block below ${bot.blockAt({ x: fx, y: fy - 1, z: fz })?.name}`)

let sink
/** Median ns/op over `rounds` rounds of at least `roundMs` each, after a warm-up. */
function loop(fn) {
  const batch = 256
  const until = performance.now() + 100
  while (performance.now() < until) for (let i = 0; i < batch; i++) sink = fn()
  const perRound = []
  for (let r = 0; r < rounds; r++) {
    let ops = 0
    const t0 = process.hrtime.bigint()
    let ns
    do {
      for (let i = 0; i < batch; i++) sink = fn()
      ops += batch
      ns = Number(process.hrtime.bigint() - t0)
    } while (ns < roundMs * 1e6)
    perRound.push(ns / ops)
  }
  return round(median(perRound), 1)
}

/**
 * Times each read of `fn` for `sampleS` s; reads that returned a new object (the value moved: a fetch and parse)
 * are the misses, the rest hits. `woken`: one read per 1 ms timer with records flowing (cold caches); otherwise
 * back to back on a blocked JS thread (warm; the shards still publish).
 */
async function sampled(fn, woken) {
  const hits = []
  const misses = []
  let last = fn()
  let reads = 0
  let yieldAt = performance.now()
  const until = performance.now() + sampleS * 1000
  while (performance.now() < until) {
    const t0 = process.hrtime.bigint()
    const value = fn()
    const ns = Number(process.hrtime.bigint() - t0)
    if (value !== last) misses.push(ns)
    else if (woken || (reads & 255) === 0) hits.push(ns)
    reads++
    last = value
    if (woken) await sleep(1)
    else if (performance.now() > yieldAt) {
      // Let the churn timer and the record stream run every 20 ms.
      await sleep(0)
      yieldAt = performance.now() + 20
    }
  }
  const ns = (v) => Object.fromEntries(Object.entries(percentiles(v, [50, 99])).map(([k, x]) => [k, x === null ? null : Math.round(x)]))
  return { misses: misses.length, missNs: ns(misses), hits: hits.length, hitNs: ns(hits) }
}

const floor = (i) => ({ x: fx - 8 + (i % 17), y: fy - 1, z: fz - 8 + ((i * 7) % 17) })
let k = 0
const loops = {
  'baseline: bot.username': () => bot.username,
  'bot.hot': () => bot.hot,
  'bot.entity.position': () => bot.entity.position,
  'bot.health': () => bot.health,
  'bot.entities': () => bot.entities,
  'bot.nearestEntity()': () => bot.nearestEntity(),
  'bot.players': () => bot.players,
  'bot.inventory': () => bot.inventory,
  'bot.heldItem': () => bot.heldItem,
  'bot.blockAt': () => bot.blockAt(floor(k++)),
  'findBlocks r4 count 1': () => bot.findBlocks({ matching: ['stone'], maxDistance: 4 }),
  'findBlocks r8 count 16': () => bot.findBlocks({ matching: ['stone'], maxDistance: 8, count: 16 }),
  'findBlocks r16 count 1 (missing block)': () => bot.findBlocks({ matching: ['diamond_block'], maxDistance: 16 }),
}
const nsPerOp = {}
for (const [name, fn] of Object.entries(loops)) {
  nsPerOp[name] = loop(fn)
  log(`${name}: ${nsPerOp[name]} ns/op`)
  await sleep(50)
}

// Churn so the versioned reads move: an extra pig in and out, and one stack's count, every 100 ms.
let flip = 0
const churn = setInterval(() => {
  flip++
  if (flip % 2) server.addEntity('minecraft:pig', 9000, fx + 0.5, fy, fz + 6.5)
  else server.removeEntity(9000)
  server.give(slots, 'minecraft:stone', 1 + (flip % 64))
}, 100)
const sampledReads = {}
for (const [name, fn] of Object.entries({ 'bot.hot': () => bot.hot, 'bot.entities': () => bot.entities, 'bot.inventory': () => bot.inventory, 'bot.players': () => bot.players })) {
  for (const woken of [false, true]) {
    const key = `${name} (${woken ? 'timer-woken' : 'back to back'})`
    sampledReads[key] = await sampled(fn, woken)
    log(`${key}:`, JSON.stringify(sampledReads[key]))
    await sleep(200)
  }
}
clearInterval(churn)

emit({
  bench: 'reads',
  server: 'FakeServer (Script::bds_spawn)',
  entities: Object.keys(bot.entities).length,
  stacks: bot.inventory.length,
  hotJsonBytes: JSON.stringify(bot.hot).length,
  entitiesJsonBytes: JSON.stringify(transport.reads.entities(bot.id)).length,
  coldJsonBytes: JSON.stringify(bot.cold).length,
  roundMs,
  rounds,
  nsPerOp,
  sampled: sampledReads,
  load: load(),
})
await Promise.race([transport.close(), sleep(10_000)])
server.stop()
process.exit(0)
