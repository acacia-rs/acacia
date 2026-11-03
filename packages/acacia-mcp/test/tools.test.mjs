// Tool generation from acacia-api's methods.json: every `mcp` method becomes a tool with a flat, valid schema.
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { fromJsonSchema } from '@modelcontextprotocol/server'

import { METHODS, mcpMethods, toolName, tools } from '@acacia/mcp'
import { clientFor, fakeBots } from './helpers.mjs'

const COMPOSITES = ['connect', 'disconnect', 'status', 'inventory', 'nearbyEntities', 'readChat', 'giveToPlayer', 'stop']

const deref = (schema, root) => (schema.$ref ? root.$defs[schema.$ref.split('/').at(-1)] : schema)

/** Whether a param schema is a scalar, an enum, a nullable one of those, or an array of scalars. */
function flat(schema, root) {
  const s = deref(schema, root)
  if (s.anyOf) return s.anyOf.every((v) => flat(v, root))
  if (s.oneOf) return s.oneOf.every((v) => v.type === 'string')
  if (s.type === 'array') return flat(s.items, root)
  const types = [s.type].flat()
  return !types.includes('object')
}

test('every mcp method, and only those, becomes a tool; privileged ones need allowCommands', () => {
  const offered = METHODS.filter((m) => m.flags.includes('mcp'))
  assert.ok(offered.length >= 10, 'the table has mcp methods')
  const names = tools().map((t) => t.name)
  const privileged = offered.filter((m) => m.flags.includes('privileged')).map((m) => toolName(m.name))
  assert.deepEqual(privileged, ['command'])
  assert.deepEqual(names, [...COMPOSITES, ...offered.map((m) => toolName(m.name)).filter((n) => !privileged.includes(n))])
  assert.ok(tools({ allowCommands: true }).some((t) => t.name === 'command'))
  for (const m of METHODS.filter((m) => !m.flags.includes('mcp'))) {
    assert.ok(!names.includes(toolName(m.name)), `${m.name} is not flagged mcp`)
  }
  assert.ok(!names.includes('snapshot') && !names.includes('createBot') && !names.includes('cancel'))
  assert.equal(new Set(names).size, names.length, 'names are unique')
  assert.deepEqual(mcpMethods(METHODS, true).length, offered.length)
})

test('schemas are valid JSON Schema, flat, with an optional bot param, and every tool is described', async () => {
  for (const tool of tools({ allowCommands: true })) {
    const { inputSchema: s } = tool
    assert.equal(s.type, 'object', tool.name)
    assert.ok(tool.description.length > 10, `${tool.name} has a description`)
    assert.ok(!JSON.stringify(s).includes('"format"'), `${tool.name}: numeric formats dropped`)
    assert.match(tool.name, /^[A-Za-z][A-Za-z0-9]*$/, 'a plain camelCase name')
    for (const [name, prop] of Object.entries(s.properties)) {
      assert.ok(flat(prop, s), `${tool.name}.${name} is flat: ${JSON.stringify(prop)}`)
    }
    if (tool.name !== 'connect') assert.equal(s.properties.bot?.type, 'string', `${tool.name} takes bot`)
    assert.ok(!s.required.includes('bot'), `${tool.name}: bot is optional`)
    assert.equal(s.$schema, undefined)
    // The SDK's validator compiles it (it throws on a schema it cannot compile) and checks `required`.
    const checked = await fromJsonSchema(s)['~standard'].validate({})
    assert.equal(!checked.issues, s.required.length === 0, `${tool.name}: ${JSON.stringify(checked.issues)}`)
    if (tool.outputSchema) assert.equal(tool.outputSchema.type, 'object')
  }
})

test("generated params keep the table's fields and docs; adapters reshape goto, follow, attack and answerForm", () => {
  const byName = Object.fromEntries(tools().map((t) => [t.name, t]))
  const dig = METHODS.find((m) => m.name === 'bot.dig')
  assert.equal(byName.dig.description, dig.description)
  assert.deepEqual(byName.dig.inputSchema.required, ['x', 'y', 'z'])
  assert.equal(byName.chat.inputSchema.properties.message.description, 'The line everyone sees.')
  assert.equal(byName.findBlocks.inputSchema.properties.maxDistance.type[0], 'integer')
  assert.equal(byName.findBlocks.outputSchema.properties.blocks.type, 'array')
  assert.deepEqual(Object.keys(byName.goto.inputSchema.properties), ['bot', 'x', 'y', 'z', 'player', 'range'])
  assert.ok(byName.follow.inputSchema.properties.seconds)
  assert.match(byName.attack.inputSchema.properties.entity.description, /player's name/)
  assert.ok(!JSON.stringify(byName.answerForm.inputSchema).includes('oneOf'))
})

test('the server lists the same tools over MCP, with the schemas as given', async () => {
  const { bots } = await fakeBots()
  const { client, close } = await clientFor(bots)
  try {
    const { tools: listed } = await client.listTools()
    assert.deepEqual(listed.map((t) => t.name), tools().map((t) => t.name))
    const goto = listed.find((t) => t.name === 'goto')
    assert.deepEqual(goto.inputSchema, tools().find((t) => t.name === 'goto').inputSchema)
    assert.ok(listed.find((t) => t.name === 'findBlocks').outputSchema)
    assert.equal(listed.find((t) => t.name === 'status').annotations?.readOnlyHint, true)
  } finally {
    await close()
  }
})
