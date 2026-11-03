// WsTransport's framing against a stand-in daemon (test/ws-server.mjs); the real daemon: ws-e2e.test.mjs.
import assert from 'node:assert/strict'
import { once } from 'node:events'
import { test } from 'node:test'

import { createBot, PROTOCOL_VERSION, WsTransport } from '@acacia/bot'
import { wsServer } from './ws-server.mjs'

/** A transport with a listener recording what it delivers. */
function opened(url, options) {
  const transport = new WsTransport(url, options)
  const got = { records: [], closed: 0 }
  transport.open({ records: (r) => got.records.push(...r), closed: () => got.closed++ })
  return { transport, got }
}

async function delivered(got, count) {
  while (got.records.length < count) await new Promise((r) => setTimeout(r, 5))
  return got.records
}

test('hello goes first, commands wait for its answer, responses become done records after the events before them', async (t) => {
  const server = await wsServer()
  t.after(server.close)
  const { transport, got } = opened(server.url, { token: 'k' })
  transport.send({ call: 1, method: 'createBot', params: { host: 'h', username: 'U', persist: true } })
  const peer = await server.connection()
  const hello = await peer.greet()
  assert.deepEqual(hello, { protocolVersion: PROTOCOL_VERSION, client: '@acacia/bot', mirror: true, token: 'k' })
  const create = await peer.next()
  assert.deepEqual(create, { jsonrpc: '2.0', id: 1, method: 'createBot', params: { host: 'h', username: 'U', persist: true } })

  const spawn = { type: 'event', bot: 'bot-1', seq: 1, event: { type: 'spawn' } }
  const mirror = { type: 'mirror', bot: 'bot-1', hot: { tick: 7 }, entities: { version: 1, entities: [] } }
  peer.send({ jsonrpc: '2.0', method: 'out', params: { records: [mirror, spawn] } })
  peer.send({ jsonrpc: '2.0', id: 1, result: { bot: 'bot-1' } })
  assert.deepEqual(await delivered(got, 2), [spawn, { type: 'done', bot: 'bot-1', call: 1, result: { ok: { bot: 'bot-1' } } }])
  assert.equal(transport.reads.hot('bot-1').tick, 7, 'mirror records feed the reads and are not delivered')
  assert.ok(Object.isFrozen(transport.reads.hot('bot-1')))
  assert.equal(transport.reads.cold('bot-1'), null)
  assert.throws(() => transport.reads.blockAt('bot-1', 0, 0, 0), { code: 'NOT_POSSIBLE' })

  transport.send({ call: 2, method: 'bot.chat', params: { bot: 'bot-1', message: 'x' } })
  assert.equal((await peer.next()).id, 2)
  peer.send({ jsonrpc: '2.0', id: 2, error: { code: -32021, message: 'gone', data: { code: 'UNKNOWN_BOT' } } })
  assert.deepEqual((await delivered(got, 3))[2], { type: 'done', bot: 'bot-1', call: 2, result: { err: { code: 'UNKNOWN_BOT', message: 'gone' } } })

  transport.setKeepAlive(false)
  transport.send({ call: 3, method: 'bot.waitForTicks', params: { bot: 'bot-1', ticks: 5 } })
  assert.equal((await peer.next()).id, 3, 'a live bot keeps the socket open')
  peer.drop()
  const records = await delivered(got, 5)
  assert.deepEqual(records[3], { type: 'done', bot: 'bot-1', call: 3, result: { err: { code: 'DISCONNECTED', message: `the connection to ${server.url} dropped` } } })
  assert.equal(records[4].event.type, 'end')
  assert.equal(records[4].event.reason, 'lost')
  assert.match(records[4].event.message, /keeps running on the daemon/)
  assert.equal(got.closed, 1)
  assert.ok(transport.ended)
  assert.throws(() => transport.send({ call: 4, method: 'bot.chat', params: { bot: 'bot-1', message: 'x' } }), { code: 'CLOSED' })
})

test('a refused hello fails the queued calls with its code and ends the stream', async (t) => {
  const server = await wsServer()
  t.after(server.close)
  const { transport, got } = opened(server.url)
  transport.send({ call: 1, method: 'createBot', params: { host: 'h', username: 'U' } })
  const peer = await server.connection()
  await peer.greet({ error: { code: -32040, message: 'protocol 2', data: { code: 'VERSION_MISMATCH' } } })
  await delivered(got, 1)
  assert.ok(transport.ended)
  await transport.close()
  assert.deepEqual(got.records, [{ type: 'done', bot: '', call: 1, result: { err: { code: 'VERSION_MISMATCH', message: 'protocol 2' } } }])
  assert.equal(got.closed, 1)
})

test('with nothing open the socket closes, and the next command reconnects', async (t) => {
  const server = await wsServer()
  t.after(server.close)
  const { transport, got } = opened(server.url)
  transport.send({ call: 1, method: 'bot.chat', params: { bot: 'bot-9', message: 'x' } })
  const first = await server.connection()
  await first.greet()
  const chat = await first.next()
  first.send({ jsonrpc: '2.0', id: chat.id, result: null })
  await delivered(got, 1)
  const closed = once(first, 'close')
  transport.setKeepAlive(false)
  await closed

  transport.send({ call: 2, method: 'bot.chat', params: { bot: 'bot-9', message: 'y' } })
  const second = await server.connection()
  await second.greet()
  assert.equal((await second.next()).id, 2)
  assert.equal(got.closed, 0, 'an idle close is no end')
  const done = once(second, 'close')
  await transport.close()
  await done
  assert.deepEqual(got.records.at(-1).result, { err: { code: 'CLOSED', message: 'the transport closed' } })
})

test('createBot over an unreachable daemon rejects with a ConnectError', async () => {
  await assert.rejects(createBot({ host: 'h', username: 'U', via: 'ws://127.0.0.1:1' }), { name: 'ConnectError', code: 'IO' })
})
