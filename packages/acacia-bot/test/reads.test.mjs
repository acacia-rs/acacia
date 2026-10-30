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
