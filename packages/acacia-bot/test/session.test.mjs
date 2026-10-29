// The TS layer over a fake Transport: no host, no server (the addon only loads).
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { setImmediate as tick } from 'node:timers/promises'

import { AcaciaError, ActionError, ConnectError, createBot } from '@acacia/bot'

class FakeTransport {
  sent = []
  keepAlive = []
  refuse = null
  open(listener) {
    this.listener = listener
  }
  send(command) {
    if (this.refuse) throw new AcaciaError(this.refuse, 'refused')
    this.sent.push(command)
  }
  setKeepAlive(on) {
    this.keepAlive.push(on)
  }
  close() {
    this.listener.closed()
    return Promise.resolve()
  }
  push(...records) {
    this.listener.records(records)
  }
  last(method) {
    return this.sent.findLast((c) => c.method === method)
  }
  done(call, result = { ok: null }, bot = 'bot-1') {
    return { type: 'done', bot, call, result }
  }
  event(event, seq = 1, bot = 'bot-1') {
    return { type: 'event', bot, seq, event }
  }
}

async function connected(options = {}) {
  const via = new FakeTransport()
  const creating = createBot({ host: 'h', username: 'U', via, ...options })
  via.push(via.event({ type: 'spawn' }), via.done(via.last('createBot').call, { ok: { bot: 'bot-1' } }))
  const bot = await creating
  await tick()
  return { via, bot }
}

const chat = (plain) => ({ type: 'chat', kind: 'chat', sender: 'S', message: plain, plain, params: [], xuid: '' })

test('events raised during an action come before it settles', async () => {
  const { via, bot } = await connected()
  const log = []
  bot.on('chat', (line) => log.push(`chat ${line.plain}`))
  const walking = bot.goto({ type: 'xz', x: 1, z: 2 }).then(() => log.push('goto done'))
  const { call, params } = via.last('bot.goto')
  assert.deepEqual(params, { bot: 'bot-1', goal: { type: 'xz', x: 1, z: 2 } })
  via.push(via.event(chat('one')), via.event(chat('two'), 2), via.done(call))
  await walking
  assert.deepEqual(log, ['chat one', 'chat two', 'goto done'])
})

test('records after createBot wait until the caller can listen', async () => {
  const via = new FakeTransport()
  const creating = createBot({ host: 'h', username: 'U', via, events: ['chat'] })
  assert.deepEqual(via.last('createBot').params.events, ['chat'])
  via.push(via.done(via.last('createBot').call, { ok: { bot: 'bot-1' } }), via.event(chat('early'), 2))
  const bot = await creating
  const seen = []
  bot.on('chat', (line) => seen.push(line.plain))
  await tick()
  assert.deepEqual(seen, ['early'])
})

test('aborting sends cancel and rejects with the signal reason once the bot stopped', async () => {
  const { via, bot } = await connected()
  const aborter = new AbortController()
  const waiting = bot.waitForTicks(400, { signal: aborter.signal })
  const { call } = via.last('bot.waitForTicks')
  aborter.abort()
  assert.deepEqual(via.last('cancel').params, { bot: 'bot-1', call })
  via.push(via.done(via.last('cancel').call), via.done(call, { err: { code: 'CANCELLED', message: 'cancelled' } }))
  await assert.rejects(waiting, { name: 'AbortError' })

  const before = via.sent.length
  await assert.rejects(bot.dig({ x: 1, y: 2, z: 3 }, { signal: AbortSignal.abort() }), { name: 'AbortError' })
  assert.equal(via.sent.length, before, 'an aborted signal sends nothing')
})

test('failures map to AcaciaError subclasses by code', async () => {
  const via = new FakeTransport()
  const creating = createBot({ host: 'h', username: 'U', via })
  via.push(via.done(via.last('createBot').call, { err: { code: 'IO', message: 'refused' } }))
  await assert.rejects(creating, (e) => e instanceof ConnectError && e.code === 'IO')

  const { via: v, bot } = await connected()
  const walking = bot.goto({ type: 'y', y: 3 })
  v.push(v.done(v.last('bot.goto').call, { err: { code: 'NOT_POSSIBLE', message: 'no path' } }))
  await assert.rejects(walking, (e) => e instanceof ActionError && e.code === 'NOT_POSSIBLE' && e.message === 'no path')

  v.refuse = 'BUSY'
  await assert.rejects(bot.chat('hi'), (e) => e instanceof AcaciaError && !(e instanceof ActionError) && e.code === 'BUSY')
})

test('listeners turn their event groups on and off', async () => {
  const { via, bot } = await connected({ events: ['health'] })
  const groups = () => via.sent.filter((c) => c.method === 'bot.setEvents').map((c) => c.params.events)
  const onChat = () => {}
  bot.on('chat', onChat)
  bot.on('chat', () => {})
  bot.once('form', () => {})
  await tick()
  assert.deepEqual(groups(), [['chat', 'forms', 'health']], 'one update per microtask')
  bot.off('chat', onChat)
  await tick()
  assert.equal(groups().length, 1, 'chat still has a listener')
  bot.removeAllListeners('chat')
  bot.on('spawn', () => {})
  await tick()
  assert.deepEqual(groups().at(-1), ['forms', 'health'], 'forced groups stay on')
})

test('keep-alive follows open calls and live bots', async () => {
  const { via, bot } = await connected()
  assert.deepEqual(via.keepAlive, [true], 'held from createBot on')
  const closing = bot.close()
  const iterated = (async () => {
    const ends = []
    for await (const end of bot.events('end')) ends.push(end.reason)
    return ends
  })()
  via.push(via.event({ type: 'end', reason: 'closed', message: 'closed' }, 2), via.done(via.last('bot.close').call))
  await closing
  assert.deepEqual(await iterated, ['closed'])
  assert.equal(bot.ended, true)
  assert.deepEqual(via.keepAlive, [true, false], 'released once nothing is owed')
})

test('a throwing listener does not cost the rest of the batch', async () => {
  const { via, bot } = await connected()
  const seen = []
  bot.on('chat', (line) => {
    seen.push(line.plain)
    if (line.plain === 'boom') throw new Error('listener bug')
  })
  // The rethrow is an uncaught exception, which node:test would pin on the test: catch it at the source.
  const nextTick = process.nextTick
  let rethrown
  process.nextTick = (fn) => {
    try {
      fn()
    } catch (e) {
      rethrown = e
    }
  }
  try {
    via.push(via.event(chat('boom')), via.event(chat('after'), 2))
  } finally {
    process.nextTick = nextTick
  }
  assert.deepEqual(seen, ['boom', 'after'])
  assert.equal(rethrown?.message, 'listener bug')
})

test('the stream ending rejects what is still open', async () => {
  const { via, bot } = await connected()
  const waiting = bot.waitForTicks(5)
  await via.close()
  await assert.rejects(waiting, { code: 'CLOSED' })
  await assert.rejects(bot.chat('late'), { code: 'CLOSED' })
})
