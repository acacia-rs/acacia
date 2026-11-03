// Shared by the tests: an MCP client wired in memory to a server over a BotRegistry on @acacia-rs/bot's FakeTransport.
import { setImmediate as tick } from 'node:timers/promises'

import { Client, InMemoryTransport } from '@modelcontextprotocol/client'

import { BotRegistry, createServer } from '@acacia-rs/mcp'
import { FakeTransport } from '../../acacia-bot/test/fake.mjs'

/** A client connected in memory to a server over `bots`. */
export async function clientFor(bots, options = {}) {
  const [clientSide, serverSide] = InMemoryTransport.createLinkedPair()
  const server = createServer(bots, options)
  await server.connect(serverSide)
  const client = new Client({ name: 'test', version: '0' })
  await client.connect(clientSide)
  return { client, close: () => Promise.all([client.close(), server.close()]) }
}

/** A registry on a fake transport with bot `U` (id `bot-1`) connected. */
export async function fakeBots(username = 'U') {
  const via = new FakeTransport()
  const bots = new BotRegistry({ host: 'h' }, via)
  const connecting = bots.connect({ username })
  await tick()
  await spawn(via)
  const handle = await connecting
  await tick()
  return { via, bots, handle }
}

/** Answers the last `createBot` with bot `id` spawned, then the first tick `connect` waits for (`bot.ready()`). */
export async function spawn(via, id = 'bot-1') {
  const before = via.sent.length
  via.push(via.event({ type: 'spawn' }, 1, id), via.done(via.last('createBot').call, { ok: { bot: id } }, id))
  const groups = await sent(via, 'bot.setEvents', before)
  via.state[id] = { ...via.state[id], hot: via.state[id]?.hot ?? { tick: 1 } }
  via.push(via.done(groups.call, { ok: null }, id), via.event({ type: 'physicsTick' }, 2, id))
}

/** Calls tool `name` and returns `{ text, structured, isError }`. */
export async function call(client, name, args = {}, options) {
  const result = await client.callTool({ name, arguments: args }, options)
  return { text: result.content.map((c) => c.text).join('\n'), structured: result.structuredContent, isError: !!result.isError }
}

/** Waits until the fake transport has a command for `method` past `after` sent ones. */
export async function sent(via, method, after = 0) {
  for (let i = 0; i < 200; i++) {
    const command = via.sent.slice(after).findLast((c) => c.method === method)
    if (command) return command
    await tick()
  }
  throw new Error(`no ${method} sent; got ${via.sent.map((c) => c.method).join(', ')}`)
}
