// Against acacia-testserver's FakeServer (a recorded BDS spawn); needs `npm run build:test` (TestServer).
import assert from 'node:assert/strict'
import { once } from 'node:events'
import { test } from 'node:test'

import * as native from '../native/index.js'
import { createBot, NativeTransport } from '@acacia/bot'
import { runHello, until } from './hello-run.mjs'

const skip = native.TestServer ? false : 'the addon was built without TestServer (npm run build:test)'

test('hello world: spawn, answer a chat, come to a player, quit, and the process exits on its own', { skip, timeout: 60_000 }, () =>
  runHello(native),
)

test('in process: inventory, findBlocks, equip and a placement the server confirms', { skip, timeout: 60_000 }, async () => {
  const server = native.TestServer.start()
  const via = new NativeTransport({ shards: 1 })
  try {
    const bot = await createBot({ host: '127.0.0.1', port: server.port, username: 'Builder', transport: 'raknet', via })
    server.layFloor(4)
    const [fx, fy, fz] = server.spawnFeet().map(Math.floor)
    await until('the floor', () => bot.blockAt({ x: fx, y: fy - 1, z: fz })?.name === 'minecraft:stone')
    assert.deepEqual(bot.findBlocks({ matching: ['stone'], maxDistance: 2, count: 1 }), [{ x: fx, y: fy - 1, z: fz }])
    assert.throws(() => bot.findBlocks({ matching: ['no_such_block'] }), { code: 'BAD_PARAMS' })

    server.give(4, 'minecraft:stone', 10)
    await until('the stone in bot.inventory', () => bot.inventory.some((i) => i.name === 'minecraft:stone'))
    await bot.equip('stone')
    await until('the stone in hand', () => bot.heldItem?.name === 'minecraft:stone')

    const placing = bot.placeBlock({ x: fx + 1, y: fy - 1, z: fz }, 'up', { signal: AbortSignal.timeout(20_000) })
    await until('the placement packet', () => server.inventoryTransactions() > 0)
    server.setBlock(fx + 1, fy, fz, 'minecraft:stone')
    await placing
    await bot.close()
  } finally {
    await via.close()
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
    // The listeners' setEvents must reach the bot before the AddEntity does, or the spawn is filtered out.
    await bot.waitForTicks(2)
    server.addEntity('minecraft:zombie', 90, x + 2, y, z)
    await until('the zombie spawn', () => seen.some(([what, id]) => what === 'spawn' && id === '90'))
    assert.deepEqual(seen.find(([, id]) => id === '90'), ['spawn', '90', 'minecraft:zombie'])
    await until('the zombie in bot.entities', () => bot.entities['90'] !== undefined)
    assert.equal(bot.nearestEntity((e) => e.kind === 'minecraft:zombie')?.id, '90')
    const before = bot.entities
    server.moveEntity(90, x + 4, y, z)
    await until('the zombie moved', () => Math.abs(bot.entities['90'].position.x - (x + 4)) < 1e-3)
    assert.equal(bot.entities, before, 'a move keeps the list, its entities read live')
    assert.ok(Math.abs({ ...bot.entities['90'] }.position.x - (x + 4)) < 1e-3, 'a spread copies the live fields')
    assert.equal(JSON.parse(JSON.stringify(bot.entities['90'])).kind, 'minecraft:zombie')
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
