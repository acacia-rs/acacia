// Microsoft sign-in: msaCode routing over a fake transport and a stand-in daemon, then end to end in-process with
// the addon's fake sign-in (ACACIA_FAKE_SIGN_IN, test builds only; nothing reaches Microsoft).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import * as native from '../native/index.js'
import { createBot, NativeTransport, signIn, WsTransport } from '@acacia-rs/bot'
import { FakeTransport } from './fake.mjs'
import { wsServer } from './ws-server.mjs'

const code = { userCode: 'ABCD', verificationUri: 'https://www.microsoft.com/link', message: 'enter ABCD', expiresIn: 900 }

test('an msaCode record reaches the onMsaCode of its own call before the call settles', async () => {
  const via = new FakeTransport()
  const prompts = []
  const signing = signIn({ username: 'Alex', via, onMsaCode: (p) => prompts.push(p) })
  const other = createBot({ host: 'h', username: 'U', via, onMsaCode: () => assert.fail('not this call') })
  const { call, params } = via.last('signIn')
  assert.deepEqual(params, { username: 'Alex' })
  via.push({ type: 'msaCode', call, ...code }, { type: 'done', call, result: { ok: { displayName: 'Alex', xuid: '1' } } })
  assert.deepEqual(await signing, { displayName: 'Alex', xuid: '1' })
  assert.deepEqual(prompts, [code])
  via.push(via.done(via.last('createBot').call, { ok: { bot: 'bot-1' } }))
  await other
})

test('the default onMsaCode prints the message on stderr', async (t) => {
  const via = new FakeTransport()
  const written = []
  t.mock.method(process.stderr, 'write', (text) => written.push(String(text)))
  const creating = createBot({ host: 'h', username: 'U', auth: 'microsoft', via })
  const { call, params } = via.last('createBot')
  assert.equal(params.auth, 'microsoft')
  via.push({ type: 'msaCode', call, ...code }, via.done(call, { ok: { bot: 'bot-1' } }))
  await creating
  t.mock.restoreAll()
  assert.deepEqual(written, ['enter ABCD\n'])
})

test('over WebSocket an msaCode in an out notification reaches the call it names', async (t) => {
  const server = await wsServer()
  t.after(server.close)
  const transport = new WsTransport(server.url)
  const prompts = []
  const signing = signIn({ username: 'Alex', via: transport, onMsaCode: (p) => prompts.push(p) })
  const peer = await server.connection()
  await peer.greet()
  const request = await peer.next()
  assert.equal(request.method, 'signIn')
  peer.send({ jsonrpc: '2.0', method: 'out', params: { records: [{ type: 'msaCode', call: request.id, ...code }] } })
  peer.send({ jsonrpc: '2.0', id: request.id, result: { displayName: 'Alex', xuid: '1' } })
  assert.equal((await signing).displayName, 'Alex')
  assert.deepEqual(prompts, [code])
  await transport.close()
})

const skip = native.TestServer ? false : 'the addon was built without TestServer (npm run build:test)'

test('in process: signIn caches, createBot prompts for an account that is not signed in, then joins', { skip, timeout: 60_000 }, async () => {
  process.env.ACACIA_FAKE_SIGN_IN = '1'
  const via = new NativeTransport({ shards: 1 })
  delete process.env.ACACIA_FAKE_SIGN_IN
  const server = native.TestServer.start()
  try {
    const prompts = []
    const onMsaCode = (p) => prompts.push(p)
    assert.equal((await signIn({ username: 'Cached', via, onMsaCode })).displayName, 'Cached')
    assert.equal(prompts.length, 1)
    assert.equal(prompts[0].userCode, 'FAKE1234')
    await signIn({ username: 'Cached', via, onMsaCode })
    assert.equal(prompts.length, 1, 'signed in already')

    const bot = await createBot({ host: '127.0.0.1', port: server.port, username: 'Fresh', transport: 'raknet', auth: 'microsoft', physics: false, via, onMsaCode })
    assert.equal(prompts.length, 2, 'the prompt came before createBot resolved')
    await bot.ready()
    await bot.close()
  } finally {
    await via.close()
    server.stop()
  }
})
