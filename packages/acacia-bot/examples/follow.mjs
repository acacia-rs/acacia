// Follows the player who says `!follow` until they say `!stop` (an AbortController cancels the follow); `!quit`
// leaves. `node examples/follow.mjs [name]` (build first: `npm run build`); the server: examples/connection.mjs.
import { ActionError, createBot } from '@acacia-rs/bot'

import { connection } from './connection.mjs'

const username = process.argv[2] ?? 'Follower'
const bot = await createBot({ ...connection(), username })
let following

async function follow(who) {
  const target = bot.players[who]?.entity
  if (!target) return bot.chat(`I can't see you, ${who}`)
  following?.abort()
  const run = (following = new AbortController())
  console.log(`following ${who}`)
  try {
    await bot.follow(target, { range: 2, signal: run.signal })
  } catch (e) {
    if (e.name === 'AbortError') {
      console.log(`stopped following ${who}`)
      await bot.chat('stopped')
    } else if (e instanceof ActionError) {
      console.log(`lost ${who}: ${e.code} ${e.message}`)
      await bot.chat(`lost you, ${who}`)
    } else throw e
  }
}

bot.on('entitySpawn', (e) => {
  if (e.username) console.log(`sees ${e.username}`)
})

bot.on('chat', async ({ sender, plain }) => {
  if (!sender || sender === username) return
  const word = plain.trim()
  if (word === '!follow') await follow(sender)
  else if (word === '!stop') following?.abort()
  else if (word === '!quit') {
    following?.abort()
    await bot.close()
  }
})

bot.on('end', ({ reason, message }) => console.log(`ended: ${reason} (${message})`))
await bot.ready()
console.log(`spawned as ${username}`)
