// Movement and combat verbs over a fake Transport: what each sends, and goal classes on the wire.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { GoalCompositeAny, GoalNear, GoalXZ, goals } from '@acacia-rs/bot'
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

test('request calls a method by name with the bot filled in', async () => {
  const { via, bot } = await connected()
  const finding = bot.request('bot.findBlocks', { matching: ['stone'] })
  assert.deepEqual(via.answer('bot.findBlocks', { ok: { blocks: [] } }).params, { bot: 'bot-1', matching: ['stone'] })
  assert.deepEqual(await finding, { blocks: [] })
})

test('activation, hand and respawn verbs send flat params; entities go by id', async () => {
  const { via, bot } = await connected({ autoRespawn: false })
  assert.equal(via.last('createBot').params.autoRespawn, false)
  const cases = [
    [() => bot.activateBlock({ x: 1, y: 2, z: 3 }), 'bot.activateBlock', { x: 1, y: 2, z: 3 }],
    [() => bot.activateBlock({ x: 1, y: 2, z: 3 }, 'north'), 'bot.activateBlock', { x: 1, y: 2, z: 3, face: 'north' }],
    [() => bot.activateItem(), 'bot.activateItem', {}],
    [() => bot.deactivateItem(), 'bot.deactivateItem', {}],
    [() => bot.activateEntity(entity('9', 'minecraft:villager_v2', vec(0, 0, 0))), 'bot.activateEntity', { entity: '9' }],
    [() => bot.swingArm(), 'bot.swingArm', {}],
    [() => bot.setQuickBarSlot(4), 'bot.setQuickBarSlot', { slot: 4 }],
    [() => bot.respawn(), 'bot.respawn', {}],
  ]
  for (const [run, method, params] of cases) {
    const running = run()
    assert.deepEqual(via.answer(method).params, { bot: 'bot-1', ...params }, method)
    await running
  }
})

test('survival verbs: tools, auto-eat, beds, signs and the craftable list', async () => {
  const { via, bot } = await connected({ autoEat: true })
  assert.deepEqual(via.last('createBot').params.autoEat, {}, 'true takes the defaults')
  const cell = { x: 1, y: 2, z: 3 }
  const sticks = [{ item: 'minecraft:stick', count: 4 }]
  const cases = [
    [() => bot.equipBestTool(cell), 'bot.equipBestTool', cell, { slot: 2 }, 2],
    [() => bot.equipBestTool(cell), 'bot.equipBestTool', cell, { slot: null }, null],
    [() => bot.dig(cell, { equipBestTool: true }), 'bot.dig', { ...cell, equipBestTool: true }, null, null],
    [() => bot.setAutoEat(false), 'bot.setAutoEat', { enabled: false }, null, null],
    [() => bot.setAutoEat({ below: 10, prefer: 'noWaste' }), 'bot.setAutoEat', { enabled: true, below: 10, prefer: 'noWaste' }, null, null],
    [() => bot.sleep(cell), 'bot.sleep', cell, null, null],
    [() => bot.wake(), 'bot.wake', {}, null, null],
    [() => bot.writeSign(cell, 'hello\nworld'), 'bot.writeSign', { ...cell, lines: ['hello', 'world'] }, null, null],
    [() => bot.writeSign(cell, ['x'], { back: true }), 'bot.writeSign', { ...cell, lines: ['x'], back: true }, null, null],
    [() => bot.craftableItems(), 'bot.craftableItems', {}, { items: sticks }, sticks],
    [() => bot.craftableItems({ table: true }), 'bot.craftableItems', { table: true }, { items: [] }, []],
  ]
  for (const [run, method, params, result, want] of cases) {
    const running = run()
    assert.deepEqual(via.answer(method, { ok: result }).params, { bot: 'bot-1', ...params }, method)
    assert.deepEqual(await running, want, method)
  }
})
