// Tool calls over an in-memory MCP client against @acacia/bot's fake transport (test/helpers.mjs).
import assert from 'node:assert/strict'
import { afterEach, beforeEach, test } from 'node:test'
import { setImmediate as tick } from 'node:timers/promises'

import { entity, vec } from '../../acacia-bot/test/fake.mjs'
import { call, clientFor, fakeBots, sent } from './helpers.mjs'

const hot = (position) => ({
  tick: 5, position, velocity: vec(0, 0, 0), yaw: 0, pitch: 0, onGround: true, health: 18, maxHealth: 20, food: 17,
  saturation: 5, xpLevel: 0, xpProgress: 0, air: 300, alive: true, gameMode: 0, dimension: 0, heldSlot: 0,
  flying: false, sleeping: false, riding: false, gliding: false,
})

let via, bots, client, close
beforeEach(async () => {
  ;({ via, bots } = await fakeBots())
  via.state['bot-1'] = {
    hot: hot(vec(0.5, 64, 0.5)),
    cold: {
      version: 1,
      inventory: [{ window: 0, slot: 0, networkId: 5, count: 10, metadata: 0 }],
      effects: [],
      players: [{ uuid: 'u', username: 'Steve', xuid: '', uniqueId: '-77' }, { uuid: 'v', username: 'Alex', xuid: '', uniqueId: '-78' }],
    },
    entities: {
      version: 1,
      entities: [
        entity('77', 'minecraft:player', vec(5.5, 64, 0.5), { username: 'Steve' }),
        entity('90', 'minecraft:zombie', vec(3.5, 64, 4.5)),
      ],
    },
    items: { items: [{ id: 5, name: 'minecraft:stone' }] },
  }
  ;({ client, close } = await clientFor(bots, { allowCommands: false }))
})
afterEach(() => close())

test('status sums up the bot: position, health, held item, players, entities, unread chat', async () => {
  const { text, structured, isError } = await call(client, 'status')
  assert.ok(!isError, text)
  assert.match(text, /^U at 0\.5 64 0\.5 \(dimension 0, survival\), health 18\/20, food 17$/m)
  assert.match(text, /holding: minecraft:stone x10/)
  assert.match(text, /players in view: Steve 5 away at 5\.5 64 0\.5/)
  assert.match(text, /players out of view: Alex/)
  assert.match(text, /entities within 32: 1 minecraft:zombie \(nearest 5\)/)
  assert.deepEqual(structured.players, [{ name: 'Steve', distance: 5, position: { x: 5.5, y: 64, z: 0.5 } }])
  assert.deepEqual(structured.heldItem, { name: 'minecraft:stone', count: 10 })
})

test('inventory and nearbyEntities read the published state', async () => {
  assert.equal((await call(client, 'inventory')).text, 'slot 0: minecraft:stone x10')
  const { text, structured } = await call(client, 'nearbyEntities', { kind: 'zombie' })
  assert.equal(text, '90 minecraft:zombie 5 away at 3.5 64 4.5')
  assert.equal(structured.entities.length, 1)
  assert.equal((await call(client, 'nearbyEntities', { maxDistance: 2 })).text, 'nothing in range')
})

test('readChat returns lines since the last call, and can wait for the next', async () => {
  const line = (sender, plain) => ({ type: 'chat', kind: 'chat', sender, message: plain, plain, params: [], xuid: '' })
  via.push(via.event(line('Steve', 'hi bot'), 1), via.event(line('Alex', 'hello'), 2))
  await tick()
  assert.equal((await call(client, 'readChat')).text, '<Steve> hi bot\n<Alex> hello')
  assert.equal((await call(client, 'readChat')).text, 'no new chat')
  const waiting = call(client, 'readChat', { wait: 5 })
  setTimeout(() => via.push(via.event(line('Steve', 'still there?'), 3)), 50)
  const { text, structured } = await waiting
  assert.equal(text, '<Steve> still there?')
  assert.deepEqual(structured.lines, [{ kind: 'chat', sender: 'Steve', message: 'still there?' }])
})

test('goto walks to a player by name and reports arrival; a failure is a tool error with the acacia code', async () => {
  const going = call(client, 'goto', { player: 'steve' })
  const command = await sent(via, 'bot.goto')
  assert.deepEqual(command.params, { bot: 'bot-1', goal: { type: 'near', x: 5, y: 64, z: 0, range: 2 } })
  via.answer('bot.goto')
  assert.equal((await going).text, 'arrived; U is now at 0.5 64 0.5')

  const n = via.sent.length
  const failing = call(client, 'goto', { x: 1, y: 2, z: 3 })
  assert.deepEqual((await sent(via, 'bot.goto', n)).params.goal, { type: 'block', x: 1, y: 2, z: 3 })
  via.answer('bot.goto', { err: { code: 'NOT_POSSIBLE', message: 'no path' } })
  const { text, structured, isError } = await failing
  assert.ok(isError)
  assert.equal(text, 'NOT_POSSIBLE: no path')
  assert.deepEqual(structured.error, { code: 'NOT_POSSIBLE', message: 'no path' })

  const bad = await call(client, 'goto', { player: 'Nobody' })
  assert.deepEqual([bad.isError, bad.text], [true, 'NOT_POSSIBLE: no player or entity called Nobody in view'])
})

test('toss aims at a named entity; giveToPlayer walks to the player first', async () => {
  const tossing = call(client, 'toss', { item: 'dirt', count: 2, toward: 'Steve' })
  const toss = await sent(via, 'bot.toss')
  assert.deepEqual(toss.params, { bot: 'bot-1', item: 'dirt', count: 2, toward: { x: 5.5, y: 65.62, z: 0.5 } })
  via.answer('bot.toss', { ok: { tossed: 2 } })
  assert.deepEqual([(await tossing).text, (await tossing).structured], ['tossed 2 dirt', { tossed: 2 }])

  const n = via.sent.length
  const giving = call(client, 'giveToPlayer', { player: 'Steve', item: 'stone' })
  assert.deepEqual((await sent(via, 'bot.goto', n)).params.goal, { type: 'near', x: 5, y: 64, z: 0, range: 2 })
  via.answer('bot.goto')
  assert.deepEqual((await sent(via, 'bot.toss', n)).params, { bot: 'bot-1', item: 'stone', toward: { x: 5.5, y: 65.62, z: 0.5 } })
  via.answer('bot.toss', { ok: { tossed: 10 } })
  assert.equal((await giving).text, 'threw 10 stone to Steve; U is now at 0.5 64 0.5')
})

test('cancelling an MCP request cancels the bot call it started', async () => {
  const aborted = new AbortController()
  const following = client.callTool({ name: 'follow', arguments: { entity: 'Steve' } }, { signal: aborted.signal })
  const follow = await sent(via, 'bot.follow')
  assert.equal(follow.params.entity, '77')
  aborted.abort()
  await assert.rejects(following)
  const cancel = await sent(via, 'cancel')
  assert.deepEqual(cancel.params, { bot: 'bot-1', call: follow.call })
  via.push(via.done(follow.call, { err: { code: 'CANCELLED', message: 'cancelled' } }))
})

test('follow ends after `seconds` with success; stop cancels running actions', async () => {
  const following = call(client, 'follow', { entity: '77', seconds: 1 })
  const follow = await sent(via, 'bot.follow')
  const cancel = await sent(via, 'cancel', via.sent.indexOf(follow) + 1).catch(() => null)
  assert.equal(cancel, null, 'no cancel before the deadline')
  await new Promise((r) => setTimeout(r, 1100))
  assert.equal((await sent(via, 'cancel')).params.call, follow.call)
  via.push(via.done(follow.call, { err: { code: 'CANCELLED', message: 'cancelled' } }))
  assert.equal((await following).text, 'followed for 1 s; U is now at 0.5 64 0.5')

  const digging = call(client, 'dig', { x: 1, y: 63, z: 0 })
  const dig = await sent(via, 'bot.dig')
  assert.equal((await call(client, 'stop')).text, 'stopped 1 action')
  assert.equal((await sent(via, 'cancel', via.sent.indexOf(dig))).params.call, dig.call)
  via.push(via.done(dig.call, { err: { code: 'CANCELLED', message: 'cancelled' } }))
  assert.deepEqual(await digging, { text: 'CANCELLED: stopped by the stop tool', structured: { error: { code: 'CANCELLED', message: 'stopped by the stop tool' } }, isError: true })
})

test('forms show in status and answerForm answers the latest by button', async () => {
  const body = { type: 'simple', content: 'Pick', buttons: [{ text: 'Shop' }, { text: 'Warp' }], elements: [], positions: [] }
  via.push(via.event({ type: 'form', id: 7, title: 'Menu', body }))
  await tick()
  assert.match((await call(client, 'status')).text, /open form #7 "Menu" "Pick", buttons: 0 Shop, 1 Warp/)
  const answering = call(client, 'answerForm', { button: 1 })
  assert.deepEqual((await sent(via, 'bot.answerForm')).params, { bot: 'bot-1', form: 7, answer: { type: 'button', index: 1 } })
  via.answer('bot.answerForm')
  assert.equal((await answering).text, 'answered form 7')
  assert.match((await call(client, 'answerForm', { button: 0 })).text, /^NOT_POSSIBLE: no form is open/)
})

test('findBlocks gives positions as text and structured content; chat is immediate', async () => {
  const findBlocks = call(client, 'findBlocks', { matching: ['stone'] })
  assert.deepEqual((await sent(via, 'bot.findBlocks')).params, { bot: 'bot-1', matching: ['stone'] })
  via.answer('bot.findBlocks', { ok: { blocks: [vec(0, 63, 0)] } })
  const found = await findBlocks
  assert.equal(found.text, 'found 1: 0 63 0 (1.2 away)')
  assert.deepEqual(found.structured, { blocks: [vec(0, 63, 0)] })
  const chatting = call(client, 'chat', { message: 'hello' })
  assert.equal((await sent(via, 'bot.chat')).params.message, 'hello')
  via.answer('bot.chat')
  assert.equal((await chatting).text, 'done')
})

test('several bots: tools need `bot`; command stays off without --allow-commands', async () => {
  const second = bots.connect({ username: 'Two' })
  await tick()
  via.push(via.event({ type: 'spawn' }, 1, 'bot-2'), via.done(via.last('createBot').call, { ok: { bot: 'bot-2' } }, 'bot-2'))
  await second
  const { text, isError } = await call(client, 'chat', { message: 'x' })
  assert.ok(isError)
  assert.match(text, /^BAD_PARAMS: several bots .*\(U, Two\)/)
  const chatting = call(client, 'chat', { bot: 'two', message: 'from two' })
  assert.equal((await sent(via, 'bot.chat')).params.bot, 'bot-2')
  via.answer('bot.chat')
  assert.ok(!(await chatting).isError)
  await assert.rejects(client.callTool({ name: 'command', arguments: { command: '/op me' } }), /command/)
})
