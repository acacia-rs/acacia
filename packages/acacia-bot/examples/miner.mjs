// Mines a few ores in sight and stores what they drop in the nearest chest: findBlocks for the ores, equipBestTool
// for each, dig, then deposit. `node examples/miner.mjs [name] [ores]` (default 5 ores; build first: `npm run
// build`); the server: examples/connection.mjs. Give the bot a pickaxe first (iron ore needs stone or better).
import { ActionError, createBot, goals } from '@acacia-rs/bot'

import { connection } from './connection.mjs'

const ORES = ['coal', 'iron', 'copper', 'gold', 'redstone', 'lapis', 'diamond', 'emerald'].flatMap((o) => [`${o}_ore`, `deepslate_${o}_ore`])
const DROPS = new Set(['coal', 'raw_iron', 'raw_copper', 'raw_gold', 'redstone', 'lapis_lazuli', 'diamond', 'emerald'])

const [username = 'Miner', wanted = '5'] = process.argv.slice(2)
const bot = await createBot({ ...connection(), username })
bot.on('end', ({ reason, message }) => console.log(`ended: ${reason} (${message})`))
await bot.ready()
await bot.waitForTicks(40) // let the chunks around the spawn arrive

const bare = (name) => name?.replace(/^minecraft:/, '')
const within = (ms) => ({ signal: AbortSignal.timeout(ms) })

const [chest] = bot.findBlocks({ matching: ['chest', 'barrel'], maxDistance: 32 })
const ores = bot.findBlocks({ matching: ORES, maxDistance: 32, count: 64 })
console.log(`${ores.length} ores in sight; ${chest ? `a chest at ${chest.x} ${chest.y} ${chest.z}` : 'no chest'}`)

let mined = 0
for (const ore of ores) {
  if (mined >= Number(wanted)) break
  const name = bare(bot.blockAt(ore)?.name)
  try {
    // Buried ores have no standing cell this close: NOT_POSSIBLE, and the next one is tried.
    await bot.goto(goals.near(ore.x, ore.y, ore.z, 3), within(30_000))
    const slot = await bot.equipBestTool(ore)
    console.log(`mining ${name} at ${ore.x} ${ore.y} ${ore.z} with ${slot === null ? 'bare hands' : bare(bot.heldItem?.name)}`)
    await bot.dig(ore, within(30_000))
    await bot.goto(goals.block(ore.x, ore.y, ore.z), within(10_000)).catch(() => {}) // walk over the drop
    mined++
  } catch (e) {
    if (!(e instanceof ActionError) && e.name !== 'TimeoutError') throw e
    console.log(`skipped ${name} at ${ore.x} ${ore.y} ${ore.z}: ${e.code ?? e.name}`)
  }
}

if (chest) {
  await bot.goto(goals.near(chest.x, chest.y, chest.z, 2), within(60_000))
  const carried = new Set(bot.inventory.map((i) => bare(i.name)).filter((n) => DROPS.has(n)))
  for (const item of carried) console.log(`stored ${await bot.deposit(chest, item)} ${item}`)
}
console.log(`mined ${mined} ores`)
await bot.close()
