// Movement and combat verbs over a fake Transport: what each sends, and goal classes on the wire.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { GoalCompositeAny, GoalNear, GoalXZ, goals } from '@acacia/bot'
import { connected, entity, vec } from './fake.mjs'

test('goal classes and shorthands are plain goals on the wire', async () => {
  const { via, bot } = await connected()
  const walking = bot.goto(new GoalCompositeAny(new GoalNear(1, 2, 3, 2), goals.xz(4, 5)))
  const sent = JSON.parse(JSON.stringify(via.last('bot.goto').params.goal))
  assert.deepEqual(sent, { type: 'any', goals: [{ type: 'near', x: 1, y: 2, z: 3, range: 2 }, { type: 'xz', x: 4, z: 5 }] })
  assert.ok(goals.xz(4, 5) instanceof GoalXZ)
  via.answer('bot.goto')
  await walking
})

test('follow runs until aborted, entities go by id', async () => {
  const { via, bot } = await connected()
  const steve = entity('5', 'minecraft:player', vec(1, 2, 3), { username: 'Steve' })
  const aborter = new AbortController()
  const following = bot.follow(steve, { range: 3, signal: aborter.signal })
  const { call, params } = via.last('bot.follow')
  assert.deepEqual(params, { bot: 'bot-1', entity: '5', range: 3 })
  aborter.abort()
  assert.deepEqual(via.last('cancel').params, { bot: 'bot-1', call })
  via.push(via.done(via.last('cancel').call), via.done(call, { err: { code: 'CANCELLED', message: 'cancelled' } }))
  await assert.rejects(following, { name: 'AbortError' })

  const hitting = bot.attack('7')
  assert.deepEqual(via.answer('bot.attack').params, { bot: 'bot-1', entity: '7' })
  await hitting
  const looking = bot.lookAt(vec(1.5, 2, 3.5))
  assert.deepEqual(via.answer('bot.lookAt').params, { bot: 'bot-1', x: 1.5, y: 2, z: 3.5 })
  await looking
})

test('control states go out on the immediate lane', async () => {
  const { via, bot } = await connected()
  const holding = bot.setControlState('jump', true)
  assert.deepEqual(via.answer('bot.setControlState').params, { bot: 'bot-1', control: 'jump', state: true })
  await holding
  const clearing = bot.clearControlStates()
  assert.deepEqual(via.answer('bot.clearControlStates').params, { bot: 'bot-1' })
  await clearing
})
