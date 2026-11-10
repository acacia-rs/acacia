// The mineflayer-named events derived from raw ones (kicked, whisper, chat:<name>): over a fake transport, then
// in-process against the TestServer.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { setImmediate as tick } from 'node:timers/promises'

import * as native from '../native/index.js'
import { createBot, NativeTransport } from '@acacia-rs/bot'
import { connected } from './fake.mjs'
import { until } from './hello-run.mjs'

const line = (kind, plain, sender) => ({ type: 'chat', kind, ...(sender ? { sender } : {}), message: plain, plain, params: [], xuid: '' })

test('kicked comes before end with the kick message; a plain close raises none', async () => {
  const { via, bot } = await connected()
  const seen = []
  bot.on('kicked', (reason, end) => seen.push(['kicked', reason, end.reason]))
  bot.on('end', (end) => seen.push(['end', end.reason]))
  via.push(via.event({ type: 'end', reason: 'kicked', message: 'You were kicked' }))
  assert.deepEqual(seen, [['kicked', 'You were kicked', 'kicked'], ['end', 'kicked']])

  const { via: v, bot: closed } = await connected()
  closed.on('kicked', () => assert.fail('not a kick'))
  v.push(v.event({ type: 'end', reason: 'closed', message: 'closed' }))
  assert.ok(closed.ended)
})

test('whisper and chat:<name> listeners turn the chat group on and get their payloads', async () => {
  const { via, bot } = await connected()
  const got = []
  bot.on('whisper', (username, message) => got.push(['whisper', username, message]))
  bot.addChatPattern('trade', /^(\w+) offers (\d+) (\w+)$/)
  bot.addChatPattern('once', /hello/, { repeat: false })
  bot.on('chat:trade', (groups, chat) => got.push(['trade', ...groups, chat.kind]))
  bot.on('chat:once', (groups) => got.push(['once', groups.length]))
  await tick()
  assert.deepEqual(via.last('bot.setEvents').params.events, ['chat'])

  via.push(
    via.event(line('whisper', 'psst', 'Steve')),
    via.event(line('chat', 'Alex offers 3 emerald', 'Alex'), 2),
    via.event(line('chat', 'hello hello', 'Alex'), 3),
    via.event(line('chat', 'hello again', 'Alex'), 4),
    via.event(line('whisper', 'no sender'), 5),
  )
  assert.deepEqual(got, [['whisper', 'Steve', 'psst'], ['trade', 'Alex', '3', 'emerald', 'chat'], ['once', 0]])
  assert.equal(bot.removeChatPattern('trade'), true)
  assert.equal(bot.removeChatPattern('once'), false, 'a one-shot pattern is gone once it matched')
})

const skip = native.TestServer ? false : 'the addon was built without TestServer (npm run build:test)'

test('in process: a whisper and a kick from the server', { skip, timeout: 60_000 }, async (t) => {
  const via = new NativeTransport({ shards: 1 })
  const server = native.TestServer.start()
  t.after(async () => {
    await via.close()
    server.stop()
  })
  const bot = await createBot({ host: '127.0.0.1', port: server.port, username: 'Listener', transport: 'raknet', physics: false, via })
  const whispers = []
  bot.on('whisper', (username, message) => whispers.push(`${username}: ${message}`))
  await bot.ready()
  server.whisper('Steve', 'meet at spawn')
  await until('the whisper', () => whispers.length > 0)
  assert.deepEqual(whispers, ['Steve: meet at spawn'])

  const kicked = new Promise((resolve) => bot.once('kicked', resolve))
  server.kick('Server closed')
  assert.match(await kicked, /Server closed/)
})
