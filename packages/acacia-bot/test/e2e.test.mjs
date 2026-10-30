// Against acacia-testserver's FakeServer (a recorded BDS spawn); needs `npm run build:test` (TestServer).
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { dirname, join } from 'node:path'
import { test } from 'node:test'
import { setTimeout as sleep } from 'node:timers/promises'
import { fileURLToPath } from 'node:url'

import * as native from '../native/index.js'
import { createBot, NativeTransport } from '@acacia/bot'

const pkg = join(dirname(fileURLToPath(import.meta.url)), '..')
const skip = native.TestServer ? false : 'the addon was built without TestServer (npm run build:test)'

async function until(what, check, ms = 10_000) {
  const deadline = Date.now() + ms
  while (!check()) {
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`)
    await sleep(20)
  }
}

test('hello world: spawn, answer a chat, come to a player, quit, and the process exits on its own', { skip, timeout: 60_000 }, async () => {
  const server = native.TestServer.start()
  const child = spawn(process.execPath, ['examples/hello.mjs', `127.0.0.1:${server.port}`, 'Hello'], { cwd: pkg })
  let out = ''
  child.stdout.on('data', (d) => (out += d))
  child.stderr.on('data', (d) => (out += d))
  const exited = once(child, 'exit')
  try {
    await until('spawn', () => out.includes('spawned as Hello'), 20_000)
    server.chat('Steve', '!ping')
    await until('pong', () => server.receivedChat().includes('pong'))

    server.layFloor(8)
    const [x, y, z] = server.spawnFeet()
    server.addPlayer('Steve', 77, Math.floor(x) + 5.5, Math.floor(y), Math.floor(z) + 0.5)
    await until('Steve in view', () => out.includes('sees Steve'))
    server.chat('Steve', '!come')
    await until('the walk to Steve', () => /here|no path|gave up/.test(server.receivedChat().join(' ')), 35_000).catch((e) => {
      throw new Error(`${e.message}\n${out}`)
    })
    assert.match(out, new RegExp(`coming to Steve at ${(Math.floor(x) + 5.5).toFixed(1)} `))
    assert.ok(server.receivedChat().includes('here'), `${out}\n${server.receivedChat()}`)

    server.chat('Steve', '!quit')
    const quit = Date.now()
    const [code] = await Promise.race([exited, sleep(15_000, ['still running'], { ref: false })])
    assert.equal(code, 0, out)
    assert.match(out, /<Steve> !ping/)
    assert.match(out, /ended: closed/)
    console.log(`${out.trimEnd()}\n[exit ${code} ${Date.now() - quit} ms after !quit; server got ${JSON.stringify(server.receivedChat())}]`)
  } finally {
    child.kill()
    server.stop()
  }
})

test('in process: events, sync reads, cancel and close', { skip, timeout: 60_000 }, async () => {
  const server = native.TestServer.start()
  const via = new NativeTransport({ shards: 1 })
  try {
    const bot = await createBot({ host: '127.0.0.1', port: server.port, username: 'Inproc', transport: 'raknet', physics: false, via })
    const lines = []
    bot.on('chat', (line) => lines.push(line.plain))
    await bot.waitForTicks(10)
    server.chat('Steve', 'mid-action')
    await bot.waitForTicks(10)
    assert.deepEqual(lines, ['mid-action'], 'a listener turned the chat group on')

    const hot = bot.hot
    assert.ok(hot && hot.alive && hot.tick > 0, JSON.stringify(hot))
    assert.equal(bot.hot, bot.hot, 'one parse per tick')
    const { x, y, z } = hot.position
    await until('the chunk under the bot', () => bot.blockAt({ x, y: y - 1, z }) !== null, 5_000)
    assert.match(bot.blockAt({ x, y: y - 1, z }).name, /^minecraft:/)

    const seen = []
    bot.on('entitySpawn', (e) => seen.push(['spawn', e.id, e.kind]))
    bot.on('entityGone', (e) => seen.push(['gone', e.id]))
    server.addEntity('minecraft:zombie', 90, x + 2, y, z)
    await until('the zombie spawn', () => seen.some(([what, id]) => what === 'spawn' && id === '90'))
    assert.deepEqual(seen.find(([, id]) => id === '90'), ['spawn', '90', 'minecraft:zombie'])
    await until('the zombie in bot.entities', () => bot.entities['90'] !== undefined)
    assert.equal(bot.nearestEntity((e) => e.kind === 'minecraft:zombie')?.id, '90')
    server.removeEntity(90)
    await until('the zombie gone', () => seen.some(([what, id]) => what === 'gone' && id === '90'))
    await until('the zombie out of bot.entities', () => bot.entities['90'] === undefined)

    const aborted = new AbortController()
    const long = bot.waitForTicks(400, { signal: aborted.signal })
    setTimeout(() => aborted.abort(), 200)
    await assert.rejects(long, { name: 'AbortError' })
    await bot.waitForTicks(1)

    const ended = once(bot, 'end')
    await bot.close()
    assert.equal((await ended)[0].reason, 'closed')
    await assert.rejects(bot.chat('late'), { code: 'UNKNOWN_BOT' })
  } finally {
    await via.close()
    server.stop()
  }
})
