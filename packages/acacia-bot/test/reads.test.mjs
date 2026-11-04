// Sync reads over a fake Transport: entities, players and what is derived from them.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { setImmediate as tick } from 'node:timers/promises'

import { EntityTable } from '@acacia-rs/bot'

import { connected, entity, vec } from './fake.mjs'

const hot = (position) => ({ tick: 1, position, velocity: vec(0, 0, 0), yaw: 0, pitch: 0, onGround: true })

test('entities and players come from the published state, recomputed only when it changes', async () => {
  const { via, bot } = await connected()
  assert.equal(via.last('createBot').params.entities, true, 'tracked by default')
  assert.deepEqual(bot.entities, {}, 'nothing published yet')
  assert.equal(bot.nearestEntity(), null)

  const at = (slot, x) => [slot, x, 64, 0, 0, 0, 0, 0, 0, null]
  const table = new EntityTable()
  table.apply({
    version: 1,
    full: true,
    added: [[0, '5', '-5', 'minecraft:player', 'Steve'], [1, '6', '-6', 'minecraft:player', 'Alex'], [2, '7', '-7', 'minecraft:zombie', null]],
    numbers: [at(0, 3), at(1, 9), at(2, 1)],
  })
  const info = (username, uniqueId) => ({ uuid: `u-${username}`, username, xuid: '', uniqueId })
  via.state['bot-1'] = {
    hot: hot(vec(0, 64, 0)),
    entities: table,
    cold: { version: 1, inventory: [], effects: [], players: [info('Steve', '-5'), info('Herobrine', '-9')] },
  }
  const [steve, zombie] = [bot.entities['5'], bot.entities['7']]
  assert.deepEqual([zombie.kind, zombie.position], ['minecraft:zombie', vec(1, 64, 0)])
  assert.equal(bot.entities, bot.entities, 'one object per bot')
  assert.equal(bot.nearestEntity(), zombie)
  assert.equal(bot.nearestEntity((e) => e.kind === 'minecraft:player'), steve)
  assert.equal(bot.players.Steve.entity, steve, 'joined by unique id')
  assert.equal(bot.players.Herobrine.entity, null, 'listed, out of view')
  assert.equal(bot.players.Alex.info, null, 'in view, not listed')
  assert.equal(bot.players, bot.players)

  const before = bot.players
  table.apply({ version: 2, numbers: [at(0, 4)] })
  assert.equal(bot.players, before, 'a move keeps the players')
  table.apply({ version: 3, removed: [at(0, 4), at(1, 9)] })
  assert.notEqual(bot.players, before)
  assert.equal(bot.players.Steve.entity, null, 'Steve left view')
})

test('inventory names stacks from the item table, heldItem follows the selected slot', async () => {
  const { via, bot } = await connected()
  assert.deepEqual(bot.inventory, [])
  assert.equal(bot.heldItem, null)
  const stack = (slot, networkId, count) => ({ window: 0, slot, networkId, count, metadata: 0 })
  const cold = { version: 1, inventory: [stack(0, 1, 64), stack(3, 2, 5)], effects: [], players: [] }
  via.state['bot-1'] = { hot: { ...hot(vec(0, 64, 0)), heldSlot: 3 }, cold }
  assert.deepEqual(bot.inventory.map((i) => i.name), [null, null], 'no item table yet')
  via.state['bot-1'].items = { items: [{ id: 1, name: 'minecraft:stone' }, { id: 2, name: 'minecraft:torch' }] }
  assert.deepEqual(bot.inventory.map((i) => [i.slot, i.name, i.count]), [[0, 'minecraft:stone', 64], [3, 'minecraft:torch', 5]])
  assert.equal(bot.inventory, bot.inventory, 'one array per change')
  assert.equal(bot.heldItem?.name, 'minecraft:torch')

  via.state['bot-1'].found = [vec(1, 63, 0)]
  assert.deepEqual(bot.findBlocks({ matching: ['stone'], maxDistance: 8 }), [vec(1, 63, 0)])
  assert.deepEqual(via.queries, [{ matching: ['stone'], maxDistance: 8 }])

  const equipping = bot.equip(bot.inventory[0], 'offHand')
  assert.deepEqual(via.answer('bot.equip').params, { bot: 'bot-1', item: 'minecraft:stone', destination: 'offHand' })
  await equipping
  const placing = bot.placeBlock(vec(1, 63, 0))
  assert.deepEqual(via.answer('bot.placeBlock').params, { bot: 'bot-1', x: 1, y: 63, z: 0 })
  await placing
})

test('entity listeners turn their groups on', async () => {
  const { via, bot } = await connected()
  bot.on('entitySpawn', () => {})
  bot.on('entityMoved', () => {})
  await tick()
  assert.deepEqual(via.last('bot.setEvents').params.events, ['entities', 'entityMoves'])
  const seen = []
  bot.on('entityGone', (e) => seen.push(e.id))
  via.push(via.event({ type: 'entityGone', ...entity('7', 'minecraft:zombie', vec(0, 0, 0)) }, 2))
  assert.deepEqual(seen, ['7'])
})

test('entityMoves carries rows; entityMoved gives each listed entity its live object', async () => {
  const { via, bot } = await connected()
  const table = new EntityTable()
  table.apply({ version: 1, full: true, added: [[0, '5', '-5', 'minecraft:pig', null]], numbers: [[0, 2, 64, 0, 0, 0, 0, 0, 0, 10]] })
  via.state['bot-1'] = { hot: hot(vec(0, 64, 0)), entities: table }
  const [batches, moved] = [[], []]
  bot.on('entityMoves', (e) => batches.push(e.rows.map((r) => r[0])))
  bot.on('entityMoved', (e) => moved.push(e))
  await tick()
  assert.deepEqual(via.last('bot.setEvents').params.events, ['entityMoves'])
  const row = (id) => [id, 2, 64, 0, 0, 0, 0, 0, 0, 10]
  via.push(via.event({ type: 'entityMoves', rows: [row('5'), row('9')] }, 2))
  assert.deepEqual(batches, [['5', '9']])
  assert.deepEqual(moved, [bot.entities['5']], 'an unlisted id is skipped')
})

test('ready waits for the first tick and for the listeners\' groups, then turns ticks off again', async () => {
  const { via, bot } = await connected()
  bot.on('chat', () => {})
  let ready = false
  const readying = bot.ready().then(() => (ready = true))
  const groups = via.last('bot.setEvents')
  assert.deepEqual(groups.params.events, ['chat', 'ticks'], 'sent at once, with the listener attached before')
  via.state['bot-1'] = { hot: hot(vec(0, 64, 0)) }
  via.push(via.event({ type: 'physicsTick' }, 2))
  await tick()
  assert.equal(ready, false, 'the groups are not on yet')
  via.push(via.done(groups.call))
  await readying
  assert.ok(bot.hot)
  await tick()
  assert.deepEqual(via.last('bot.setEvents').params.events, ['chat'])
  via.answer('bot.setEvents')

  const sent = via.sent.length
  await bot.ready()
  assert.equal(via.sent.length, sent, 'ready with nothing to change sends nothing')
})

test('ready rejects when the bot ends first', async () => {
  const { via, bot } = await connected()
  const readying = bot.ready()
  via.push(via.event({ type: 'end', reason: 'lost', message: 'gone' }, 2))
  await assert.rejects(readying, { code: 'DISCONNECTED' })
  await assert.rejects(bot.ready(), { code: 'DISCONNECTED' })
})
