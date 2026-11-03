// Crafting, container and item tools over an in-memory MCP client against @acacia-rs/bot's fake transport.
import assert from 'node:assert/strict'
import { afterEach, beforeEach, test } from 'node:test'

import { vec } from '../../acacia-bot/test/fake.mjs'
import { call, clientFor, fakeBots, sent } from './helpers.mjs'

let via, client, close
beforeEach(async () => {
  let bots
  ;({ via, bots } = await fakeBots())
  via.state['bot-1'] = { hot: { tick: 1, position: vec(0.5, 64, 0.5), velocity: vec(0, 0, 0), heldSlot: 0 }, found: [] }
  ;({ client, close } = await clientFor(bots))
})
afterEach(() => close())

/** Calls `tool`, answers its bot call with `result` and returns the tool result and the call's params. */
async function answered(tool, args, method, result = { ok: null }) {
  const n = via.sent.length
  const calling = call(client, tool, args)
  const command = await sent(via, method, n)
  via.push(via.done(command.call, result))
  return { ...(await calling), params: command.params }
}

test('craft uses a crafting table in reach unless told not to, and says what it made', async () => {
  const inGrid = await answered('craft', { item: 'stick', count: 4 }, 'bot.craft', { ok: { crafted: 4 } })
  assert.deepEqual(inGrid.params, { bot: 'bot-1', item: 'stick', count: 4 })
  assert.equal(inGrid.text, 'crafted 4 stick')
  assert.deepEqual(inGrid.structured, { crafted: 4 })

  const refused = await call(client, 'craft', { item: 'wooden_pickaxe', table: true })
  assert.deepEqual([refused.isError, refused.text], [true, 'NOT_POSSIBLE: no crafting table within 4 blocks'])

  via.state['bot-1'].found = [vec(2, 64, 0)]
  const atTable = await answered('craft', { item: 'wooden_pickaxe' }, 'bot.craft', { ok: { crafted: 1 } })
  assert.deepEqual(atTable.params.table, vec(2, 64, 0))
  assert.equal(atTable.text, 'crafted 1 wooden_pickaxe at the crafting table at 2 64 0')
  assert.equal(via.queries.at(-1).matching[0], 'crafting_table')
  const grid = await answered('craft', { item: 'stick', table: false }, 'bot.craft', { ok: { crafted: 4 } })
  assert.equal(grid.params.table, undefined)
})

test('container tools: contents by slot, and how many moved where', async () => {
  const chest = { x: 4, y: 64, z: -2 }
  const items = [{ slot: 0, item: 'minecraft:cobblestone', count: 20 }, { slot: 3, item: 'minecraft:oak_log', count: 5 }]
  const viewed = await answered('containerContents', chest, 'bot.containerContents', { ok: { items } })
  assert.equal(viewed.text, 'slot 0: minecraft:cobblestone x20\nslot 3: minecraft:oak_log x5')
  assert.equal((await answered('containerContents', chest, 'bot.containerContents', { ok: { items: [] } })).text, 'empty')

  const put = await answered('deposit', { ...chest, item: 'cobblestone' }, 'bot.deposit', { ok: { moved: 10 } })
  assert.deepEqual(put.params, { bot: 'bot-1', ...chest, item: 'cobblestone' })
  assert.deepEqual([put.text, put.structured], ['10 cobblestone put into the container at 4 64 -2', { moved: 10 }])
  const taken = await answered('withdraw', { ...chest, item: 'oak_log', count: 3 }, 'bot.withdraw', { ok: { moved: 3 } })
  assert.equal(taken.text, '3 oak_log taken from the container at 4 64 -2')
})

test('smelt and consume say what happened', async () => {
  const furnace = { x: 1, y: 64, z: 3 }
  const smelted = await answered('smelt', { ...furnace, item: 'raw_iron', count: 2, fuel: 'coal' }, 'bot.smelt', { ok: { smelted: 2 } })
  assert.deepEqual(smelted.params, { bot: 'bot-1', ...furnace, item: 'raw_iron', count: 2, fuel: 'coal' })
  assert.deepEqual([smelted.text, smelted.structured], ['smelted raw_iron: took 2 items out of the furnace at 1 64 3', { smelted: 2 }])
  const ate = await answered('consume', { item: 'bread' }, 'bot.consume')
  assert.deepEqual([ate.params, ate.text], [{ bot: 'bot-1', item: 'bread' }, 'consumed bread; food now ?'])
})

test('recipesFor lists each recipe as one line', async () => {
  const recipes = [
    { id: 'a', station: 'crafting_table', fits2x2: true, inputs: [{ item: '#minecraft:planks', count: 2 }], outputs: [{ item: 'minecraft:stick', count: 4 }] },
    { id: 'b', station: 'crafting_table', fits2x2: false, inputs: [{ item: 'minecraft:bamboo', count: 2 }], outputs: [{ item: 'minecraft:stick', count: 1 }] },
  ]
  const { text, structured } = await answered('recipesFor', { item: 'stick' }, 'bot.recipesFor', { ok: { recipes } })
  assert.equal(text, 'minecraft:stick x4 from #minecraft:planks x2 (2x2 grid)\nminecraft:stick x1 from minecraft:bamboo x2 (crafting table)')
  assert.deepEqual(structured.recipes, recipes)
  assert.equal((await answered('recipesFor', { item: 'dirt' }, 'bot.recipesFor', { ok: { recipes: [] } })).text, 'no recipe makes dirt')
})
