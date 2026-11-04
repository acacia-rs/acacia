// The package from TypeScript: typed options, events and block names (`BlockName` autocompletes vanilla names and
// still takes custom ones). Walks to the nearest log in sight and reports what it found.
//   npx tsc -p examples/typescript && node examples/typescript/dist/index.js [name]    (build the package first)
// A Node that strips types (official builds >= 22.18) also runs `node examples/typescript/index.ts` as it is.
// ACACIA_HOST / ACACIA_PORT pick the server (default 127.0.0.1:19132); offline auth.
import { createBot, goals, type BlockName, type BlockPos, type Bot } from '@acacia-rs/bot'

const LOGS: BlockName[] = ['oak_log', 'birch_log', 'minecraft:spruce_log']

function nearestLog(bot: Bot): BlockPos | undefined {
  return bot.findBlocks({ matching: LOGS, maxDistance: 32 })[0]
}

const username = process.argv[2] ?? 'Typed'
const bot = await createBot({ host: process.env.ACACIA_HOST || '127.0.0.1', port: Number(process.env.ACACIA_PORT || 19132), username })
bot.on('end', ({ reason }) => console.log(`ended: ${reason}`))
await bot.ready()
await bot.waitForTicks(40)

const log = nearestLog(bot)
if (log) {
  console.log(`nearest log: ${bot.blockAt(log)?.name} at ${log.x} ${log.y} ${log.z}`)
  await bot.goto(goals.near(log.x, log.y, log.z, 2), { signal: AbortSignal.timeout(30_000) })
  await bot.chat('found a tree')
} else {
  console.log('no logs in sight')
}
await bot.close()
