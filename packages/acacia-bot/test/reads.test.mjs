// Sync reads over a fake Transport: entities, players and what is derived from them.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import { setImmediate as tick } from 'node:timers/promises'

import { connected, entity, vec } from './fake.mjs'

const hot = (position) => ({ tick: 1, position, velocity: vec(0, 0, 0), yaw: 0, pitch: 0, onGround: true })

test('entities and players come from the published state, recomputed only when it changes', async () => {
  const { via, bot } = await connected()
  assert.equal(via.last('createBot').params.entities, true, 'tracked by default')
  assert.deepEqual(bot.entities, {}, 'nothing published yet')
  assert.equal(bot.nearestEntity(), null)

  const steve = entity('5', 'minecraft:player', vec(3, 64, 0), { uniqueId: '-5', username: 'Steve' })
  const alex = entity('6', 'minecraft:player', vec(9, 64, 0), { uniqueId: '-6', username: 'Alex' })
  const zombie = entity('7', 'minecraft:zombie', vec(1, 64, 0))
  const info = (username, uniqueId) => ({ uuid: `u-${username}`, username, xuid: '', uniqueId })
  via.state['bot-1'] = {
    hot: hot(vec(0, 64, 0)),
    entities: { version: 1, entities: [steve, alex, zombie] },
    cold: { version: 1, inventory: [], effects: [], players: [info('Steve', '-5'), info('Herobrine', '-9')] },
  }
  assert.equal(bot.entities['7'], zombie)
  assert.equal(bot.entities, bot.entities, 'one object per published list')
  assert.equal(bot.nearestEntity(), zombie)
  assert.equal(bot.nearestEntity((e) => e.kind === 'minecraft:player'), steve)
  assert.equal(bot.players.Steve.entity, steve, 'joined by unique id')
  assert.equal(bot.players.Herobrine.entity, null, 'listed, out of view')
  assert.equal(bot.players.Alex.info, null, 'in view, not listed')
  assert.equal(bot.players, bot.players)

  const before = bot.players
  via.state['bot-1'].entities = { version: 2, entities: [zombie] }
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
