// Cancelling createBot and signIn, which have no bot to name: over a fake transport, then in-process against a
// server that never answers and the addon's fake sign-in (ACACIA_FAKE_SIGN_IN, test builds only).
import assert from 'node:assert/strict'
import { createSocket } from 'node:dgram'
import { test } from 'node:test'

import * as native from '../native/index.js'
import { createBot, NativeTransport, signIn } from '@acacia-rs/bot'
import { FakeTransport } from './fake.mjs'

const cancelled = { err: { code: 'CANCELLED', message: 'cancelled' } }

test('aborting createBot or signIn sends cancelCall with its call id', async () => {
  const via = new FakeTransport()
  const aborter = new AbortController()
  const creating = createBot({ host: 'h', username: 'U', via, signal: aborter.signal })
  const { call } = via.last('createBot')
  aborter.abort()
  assert.deepEqual(via.last('cancelCall').params, { call })
  via.push(via.done(via.last('cancelCall').call), via.done(call, cancelled, ''))
  await assert.rejects(creating, { name: 'AbortError' })

  const signing = signIn({ username: 'A', via, signal: AbortSignal.timeout(1) })
  const { call: signInCall } = via.last('signIn')
  await new Promise((resolve) => setTimeout(resolve, 20))
  assert.deepEqual(via.last('cancelCall').params, { call: signInCall })
  via.push(via.done(signInCall, cancelled, ''))
  await assert.rejects(signing, { name: 'TimeoutError' })
})

test('an abort that loses the race to the spawn closes the new bot and rejects', async () => {
  const via = new FakeTransport()
  const aborter = new AbortController()
  const creating = createBot({ host: 'h', username: 'U', via, signal: aborter.signal })
  aborter.abort()
  via.push(via.done(via.last('createBot').call, { ok: { bot: 'bot-1' } }))
  await new Promise((resolve) => setImmediate(resolve))
  const close = via.last('bot.close')
  assert.deepEqual(close.params, { bot: 'bot-1' })
  via.push(via.event({ type: 'end', reason: 'closed', message: 'closed' }), via.done(close.call))
  await assert.rejects(creating, { name: 'AbortError' })
})

const skip = native.TestServer ? false : 'the addon was built without TestServer (npm run build:test)'

test('in process: abort ends a join to a silent server and a waiting sign-in at once', { skip, timeout: 30_000 }, async (t) => {
  const silent = createSocket('udp4')
  await new Promise((resolve) => silent.bind(0, '127.0.0.1', resolve))
  process.env.ACACIA_FAKE_SIGN_IN = '60000'
  const via = new NativeTransport({ shards: 1 })
  delete process.env.ACACIA_FAKE_SIGN_IN
  t.after(async () => {
    silent.close()
    await via.close()
  })
  const port = silent.address().port
  const joining = createBot({ host: '127.0.0.1', port, username: 'Joiner', transport: 'raknet', via, signal: AbortSignal.timeout(300) })
  const started = Date.now()
  await assert.rejects(joining, { name: 'TimeoutError' })
  assert.ok(Date.now() - started < 3_000, `the join stopped ${Date.now() - started} ms in`)

  const aborter = new AbortController()
  const params = { username: 'Waiting', profilesFolder: '/nonexistent/acacia-cancel', via, signal: aborter.signal }
  const signing = signIn({ ...params, onMsaCode: () => aborter.abort() })
  await assert.rejects(signing, { name: 'AbortError' })
  const online = { host: '127.0.0.1', port, username: 'Online', auth: 'microsoft', transport: 'raknet', profilesFolder: '/nonexistent/acacia-cancel2', via }
  const late = new AbortController()
  const creating = createBot({ ...online, signal: late.signal, onMsaCode: () => late.abort() })
  await assert.rejects(creating, { name: 'AbortError' })
})
