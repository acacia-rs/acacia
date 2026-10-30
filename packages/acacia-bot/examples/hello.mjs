// Hello world (a port of crates/acacia-bot/examples/events.rs and goto.rs): logs chat, answers `!ping` with
// `pong`, walks to the sender on `!come` (or to `!come <x> <y> <z>`, `!walk <dx> <dz>`), leaves on `!quit`.
// `node examples/hello.mjs [host:port] [name]` (build first: `npm run build`).
import { ActionError, createBot, goals } from '@acacia/bot'

const [address = '127.0.0.1:19132', username = 'Sprout'] = process.argv.slice(2)
const [host, port = '19132'] = address.split(':')

const bot = await createBot({ host, port: Number(port), username, transport: 'raknet' })
console.log(`spawned as ${username}`)

async function walk(goal) {
  try {
    await bot.goto(goal, { signal: AbortSignal.timeout(30_000) })
    await bot.chat('here')
  } catch (e) {
    if (e instanceof ActionError && e.code === 'NOT_POSSIBLE') await bot.chat('no path')
    else if (e.name === 'TimeoutError') await bot.chat('gave up')
    else throw e
  }
}

async function come(sender) {
  const target = bot.players[sender]?.entity
  if (!target) return bot.chat("I can't see you")
  const { x, y, z } = target.position
  console.log(`coming to ${sender} at ${x.toFixed(1)} ${y.toFixed(1)} ${z.toFixed(1)}`)
  await walk(goals.near(Math.floor(x), Math.floor(y), Math.floor(z), 2))
}

bot.on('entitySpawn', (e) => {
  if (e.username) console.log(`sees ${e.username}`)
})

bot.on('chat', async ({ sender, plain }) => {
  if (!sender || sender === username) return
  console.log(`<${sender}> ${plain}`)
  const [word, ...args] = plain.trim().split(/\s+/)
  const n = args.map(Number)
  if (word === '!ping') await bot.chat('pong')
  else if (word === '!come' && n.length === 3) await walk(goals.near(n[0], n[1], n[2], 2))
  else if (word === '!come') await come(sender)
  else if (word === '!walk' && n.length === 2 && bot.entity) {
    const { x, z } = bot.entity.position
    await walk(goals.xz(Math.floor(x) + n[0], Math.floor(z) + n[1]))
  } else if (word === '!quit') await bot.close()
})

bot.on('end', ({ reason, message }) => console.log(`ended: ${reason} (${message})`))
