// From logs to a wooden pickaxe: chops 3 logs of one wood in sight, crafts planks, sticks and a crafting table in
// the 2x2 grid, places the table beside the bot and crafts the pickaxe at it. `node examples/crafter.mjs [name]`
// (build first: `npm run build`); the server: examples/connection.mjs. ACACIA_HUMAN_SCALE=0.1 speeds up the
// player-paced crafting screens (README, Pacing).
import { ActionError, createBot, goals } from '@acacia-rs/bot'

import { connection } from './connection.mjs'

const LOGS = ['oak_log', 'spruce_log', 'birch_log', 'jungle_log', 'acacia_log', 'dark_oak_log', 'mangrove_log', 'cherry_log', 'pale_oak_log']
const NEEDED = 3

const username = process.argv[2] ?? 'Crafter'
const bot = await createBot({ ...connection(), username })
bot.on('end', ({ reason, message }) => console.log(`ended: ${reason} (${message})`))
await bot.ready()
console.log(`spawned as ${username}`)

const bare = (name) => name?.replace(/^minecraft:/, '')
const countOf = (item) => bot.inventory.filter((i) => bare(i.name) === item).reduce((n, i) => n + i.count, 0)

/** Polls `check` every 5 ticks, at most `ticks` ticks; resolves with its last value. */
async function poll(check, ticks) {
  for (let waited = 0; !check() && waited < ticks; waited += 5) await bot.waitForTicks(5)
  return check()
}

async function chopLogs() {
  // Chunks keep arriving after the spawn: look again for a while before giving up.
  let logs = []
  await poll(() => (logs = bot.findBlocks({ matching: LOGS, maxDistance: 32, count: 32 })).length > 0, 200)
  const kind = logs.length > 0 ? bare(bot.blockAt(logs[0])?.name) : undefined
  for (const log of logs) {
    if (countOf(kind) >= NEEDED) break
    if (bare(bot.blockAt(log)?.name) !== kind) continue
    try {
      await bot.goto(goals.near(log.x, log.y, log.z, 2), { signal: AbortSignal.timeout(30_000) })
      console.log(`digging ${kind} at ${log.x} ${log.y} ${log.z}`)
      const before = countOf(kind)
      await bot.dig(log, { equipBestTool: true })
      // Stepping into the cell picks the drop up; a log up in the tree has no floor there, so that may fail.
      await bot.goto(goals.block(log.x, log.y, log.z), { signal: AbortSignal.timeout(10_000) }).catch(() => {})
      await poll(() => countOf(kind) > before, 40)
    } catch (e) {
      if (!(e instanceof ActionError) && e.name !== 'TimeoutError') throw e
      console.log(`skipped the log at ${log.x} ${log.y} ${log.z}: ${e.code ?? e.name}`)
    }
  }
  return kind
}

/** A cell beside the bot with air in it and a solid block under it. */
function besideBot() {
  const { x, y, z } = bot.entity.position
  const feet = { x: Math.floor(x), y: Math.floor(y), z: Math.floor(z) }
  for (const [dx, dz] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
    const cell = { x: feet.x + dx, y: feet.y, z: feet.z + dz }
    const below = { ...cell, y: cell.y - 1 }
    if (bare(bot.blockAt(cell)?.name) === 'air' && !['air', 'water', 'lava'].includes(bare(bot.blockAt(below)?.name))) return { cell, below }
  }
  return null
}

async function craftPickaxe(kind) {
  const planks = kind.replace(/_log$/, '_planks')
  console.log(`crafted ${await bot.craft(planks, 4 * countOf(kind))} ${planks}`)
  console.log(`crafted ${await bot.craft('stick')} stick`)
  await bot.craft('crafting_table')
  const spot = besideBot()
  if (!spot) throw new Error('no room beside the bot for the crafting table')
  const { x, y, z } = spot.cell
  console.log(`placing crafting_table at ${x} ${y} ${z}`)
  await bot.equip('crafting_table')
  await bot.placeBlock(spot.below, 'up', { signal: AbortSignal.timeout(20_000) })
  console.log(`crafted ${await bot.craft('wooden_pickaxe', 1, { table: spot.cell })} wooden_pickaxe`)
}

const kind = await chopLogs()
if (!kind || countOf(kind) < NEEDED) {
  console.log(`found ${kind ? countOf(kind) : 0} of the ${NEEDED} logs needed`)
  await bot.chat('not enough logs around here')
} else {
  await craftPickaxe(kind)
  await bot.chat('made a wooden pickaxe')
}
await bot.close()
