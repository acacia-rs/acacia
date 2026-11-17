// Riding, item moves and workstation verbs over a fake Transport: what each sends and resolves with.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { connected, entity, vec } from './fake.mjs'

test('riding and item moves: entities go by id, slots as { window, slot } or an item name', async () => {
  const { via, bot } = await connected()
  const boat = entity('9', 'minecraft:boat', vec(1, 2, 3))
  const vehicle = { uniqueId: '90', id: '9', kind: 'minecraft:boat', driver: true }
  const offHand = { window: 119, slot: 0, networkId: 5, count: 1, metadata: 0, name: 'minecraft:shield' }
  const cases = [
    [() => bot.mount(boat), 'bot.mount', { entity: '9' }, vehicle, vehicle],
    [() => bot.dismount(), 'bot.dismount', {}, null, null],
    [() => bot.moveItem(2, 10, 5), 'bot.moveItem', { from: { window: 0, slot: 2 }, to: { window: 0, slot: 10 }, count: 5 }, null, null],
    [() => bot.moveItem('stone', { window: 120, slot: 0 }), 'bot.moveItem', { from: 'stone', to: { window: 120, slot: 0 } }, null, null],
    [() => bot.swapItems(offHand, 0), 'bot.swapItems', { a: { window: 119, slot: 0 }, b: { window: 0, slot: 0 } }, null, null],
    [() => bot.trade('7', 1), 'bot.trade', { entity: '7', offer: 1, times: 1 }, { traded: 1 }, 1],
  ]
  for (const [run, method, params, result, want] of cases) {
    const running = run()
    assert.deepEqual(via.answer(method, { ok: result }).params, { bot: 'bot-1', ...params }, method)
    assert.deepEqual(await running, want, method)
  }
})

test('workstation verbs send the block cell, slots and only the options given', async () => {
  const { via, bot } = await connected()
  const cell = { x: 1, y: 2, z: 3 }
  const main = (slot) => ({ window: 0, slot })
  const cases = [
    [() => bot.anvil(cell, 0, { rename: 'Sting' }), 'bot.anvil', { input: main(0), rename: 'Sting' }, null, null],
    [() => bot.anvil(cell, 'iron_sword', { material: 'iron_ingot' }), 'bot.anvil', { input: 'iron_sword', material: 'iron_ingot' }, null, null],
    [() => bot.enchant(cell, 4, 2), 'bot.enchant', { item: main(4), option: 2 }, null, null],
    [() => bot.grindstone(cell, 1), 'bot.grindstone', { input: main(1) }, null, null],
    [() => bot.grindstone(cell, 1, { additional: 2 }), 'bot.grindstone', { input: main(1), additional: main(2) }, null, null],
    [() => bot.smith(cell, 1, 2, 3), 'bot.smith', { base: main(1), addition: main(2), template: main(3) }, null, null],
    [() => bot.stonecut(cell, 'stone_bricks', 4), 'bot.stonecut', { item: 'stone_bricks', count: 4 }, { made: 4 }, 4],
    [() => bot.loom(cell, 0, 1, 'bo'), 'bot.loom', { banner: main(0), dye: main(1), pattern: 'bo' }, null, null],
    [() => bot.loom(cell, 0, 1, 'flo', { patternItem: 2 }), 'bot.loom', { banner: main(0), dye: main(1), pattern: 'flo', patternItem: main(2) }, null, null],
    [() => bot.cartography(cell, 0), 'bot.cartography', { map: main(0) }, { crafted: 1 }, 1],
    [() => bot.cartography(cell, 0, { additional: 'empty_map', rename: 'Home', times: 2 }), 'bot.cartography', { map: main(0), additional: 'empty_map', rename: 'Home', times: 2 }, { crafted: 2 }, 2],
    [() => bot.beacon(cell, 'speed', 'iron_ingot'), 'bot.beacon', { primary: 'speed', payment: 'iron_ingot' }, null, null],
    [() => bot.beacon(cell, 'haste', 0, { secondary: 'regeneration' }), 'bot.beacon', { primary: 'haste', payment: main(0), secondary: 'regeneration' }, null, null],
    [() => bot.brew(cell, ['potion', 'potion', 7], 'nether_wart'), 'bot.brew', { bottles: ['potion', 'potion', main(7)], ingredient: 'nether_wart' }, null, null],
  ]
  for (const [run, method, params, result, want] of cases) {
    const running = run()
    assert.deepEqual(via.answer(method, { ok: result }).params, { bot: 'bot-1', ...cell, ...params }, method)
    assert.deepEqual(await running, want, method)
  }
})

test('a refused workstation verb rejects with the error code', async () => {
  const { via, bot } = await connected()
  const enchanting = bot.enchant({ x: 0, y: 0, z: 0 }, 0, 1)
  via.answer('bot.enchant', { err: { code: 'NOT_POSSIBLE', message: 'not enough lapis' } })
  await assert.rejects(enchanting, { code: 'NOT_POSSIBLE' })
})
