// Inventory, crafting and container verbs over a fake Transport: what each sends and what it resolves with.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { connected } from './fake.mjs'

test('craft sends the item, count and table, and resolves with the count made', async () => {
  const { via, bot } = await connected()
  const crafting = bot.craft('stick', 4, { table: { x: 1, y: 2, z: 3, extra: true } })
  assert.deepEqual(via.answer('bot.craft', { ok: { crafted: 4 } }).params, {
    bot: 'bot-1', item: 'stick', count: 4, table: { x: 1, y: 2, z: 3 },
  })
  assert.equal(await crafting, 4)

  const inGrid = bot.craft({ window: 0, slot: 0, networkId: 5, count: 1, metadata: 0, name: 'minecraft:oak_planks' })
  assert.deepEqual(via.answer('bot.craft', { ok: { crafted: 4 } }).params, { bot: 'bot-1', item: 'minecraft:oak_planks', count: 1 })
  await inGrid
  await assert.rejects(bot.craft({ name: null }), { code: 'BAD_PARAMS' })

  const recipe = { id: 'stick', station: 'crafting_table', fits2x2: true, inputs: [{ item: '#minecraft:planks', count: 2 }], outputs: [{ item: 'minecraft:stick', count: 4 }] }
  const looking = bot.recipesFor('stick')
  assert.deepEqual(via.answer('bot.recipesFor', { ok: { recipes: [recipe] } }).params, { bot: 'bot-1', item: 'stick' })
  assert.deepEqual(await looking, [recipe])
})

test('container verbs take the cell, an item and an optional count, and unwrap their results', async () => {
  const { via, bot } = await connected()
  const chest = { x: 4, y: 64, z: -2, extra: 1 }
  const items = [{ slot: 0, item: 'minecraft:cobblestone', count: 20 }]
  const reading = bot.containerContents(chest)
  assert.deepEqual(via.answer('bot.containerContents', { ok: { items } }).params, { bot: 'bot-1', x: 4, y: 64, z: -2 })
  assert.deepEqual(await reading, items)

  const putting = bot.deposit(chest, 'cobblestone')
  assert.deepEqual(via.answer('bot.deposit', { ok: { moved: 10 } }).params, { bot: 'bot-1', x: 4, y: 64, z: -2, item: 'cobblestone' })
  assert.equal(await putting, 10)
  const taking = bot.withdraw(chest, 'oak_log', 3)
  assert.equal(via.answer('bot.withdraw', { ok: { moved: 3 } }).params.count, 3)
  assert.equal(await taking, 3)
})
