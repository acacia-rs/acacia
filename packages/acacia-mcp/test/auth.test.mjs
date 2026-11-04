// connect with microsoft auth over @acacia-rs/bot's fake transport: the device code comes back as the tool result.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { BotRegistry } from '@acacia-rs/mcp'
import { FakeTransport } from '../../acacia-bot/test/fake.mjs'
import { call, clientFor, sent, spawn } from './helpers.mjs'

test('connect returns the device code at once, and a second connect waits for the bot', async (t) => {
  const written = []
  t.mock.method(process.stderr, 'write', (text) => written.push(String(text)))
  const via = new FakeTransport()
  const bots = new BotRegistry({ host: 'h', auth: 'microsoft' }, via)
  const { client, close } = await clientFor(bots)
  t.after(close)

  const first = call(client, 'connect', { username: 'Online', profilesFolder: '/p' })
  const create = await sent(via, 'createBot')
  assert.equal(create.params.auth, 'microsoft')
  assert.equal(create.params.profilesFolder, '/p')
  const message = 'To sign in, use a web browser to open the page https://www.microsoft.com/link and enter the code AB12 to authenticate.'
  via.push({ type: 'msaCode', call: create.call, userCode: 'AB12', verificationUri: 'https://www.microsoft.com/link', message, expiresIn: 900 })
  const prompted = await first
  assert.ok(!prompted.isError, prompted.text)
  assert.ok(prompted.text.startsWith(message), prompted.text)
  assert.match(prompted.text, /lapses 15 min after .*call connect again/)
  assert.equal(prompted.structured.signIn.userCode, 'AB12')
  assert.ok(written.some((w) => w.includes('Online') && w.includes('AB12')), 'the operator sees it on stderr too')

  bots.signInWaitMs = 20
  const again = await call(client, 'connect', { username: 'online' })
  assert.equal(again.structured.signIn.userCode, 'AB12', 'still waiting on the person: the same code, after a wait')
  bots.signInWaitMs = 30_000
  const waiting = call(client, 'connect', { username: 'Online' })
  await spawn(via)
  const joined = await waiting
  assert.match(joined.text, /^Online is connected/)
  t.mock.restoreAll()
})
