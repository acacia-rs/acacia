// A bot that outlives its script, on acacia-daemon. The first run creates it with `persist: true`; a later run
// attaches to it by username and carries on. `!detach` in chat (or Ctrl-C) ends the script and leaves the bot on
// the daemon; `!quit` closes the bot for good.
//   cargo run -p acacia-daemon --release -- --listen 127.0.0.1:19180
//   ACACIA_VIA=ws://127.0.0.1:19180 node examples/ws-persist.mjs [name]    (run it, detach, run it again)
// ACACIA_VIA defaults to that URL; the server the bot joins: examples/connection.mjs.
import { AcaciaError, attachBot, closeAll, createBot } from '@acacia-rs/bot'

import { connection } from './connection.mjs'

const via = process.env.ACACIA_VIA || 'ws://127.0.0.1:19180'
const username = process.argv[2] ?? 'Keeper'

async function attachOrCreate() {
  try {
    const bot = await attachBot({ via, username })
    console.log(`attached to ${username}`)
    return bot
  } catch (e) {
    if (!(e instanceof AcaciaError && e.code === 'UNKNOWN_BOT')) throw e
  }
  const bot = await createBot({ ...connection(), via, username, persist: true })
  console.log(`created ${username}, persisted`)
  return bot
}

const bot = await attachOrCreate()
bot.on('end', ({ reason, message }) => console.log(`ended: ${reason} (${message})`))
bot.on('chat', async ({ sender, plain }) => {
  if (!sender || sender === username) return
  console.log(`<${sender}> ${plain}`)
  if (plain === '!detach') await closeAll()
  else if (plain === '!quit') await bot.close()
})
// Disconnecting only drops this script's connection: the daemon keeps a persisted bot running.
process.once('SIGINT', () => closeAll())

await bot.ready()
const { x, y, z } = bot.entity.position
console.log(`${username} at ${x.toFixed(1)} ${y.toFixed(1)} ${z.toFixed(1)}`)
