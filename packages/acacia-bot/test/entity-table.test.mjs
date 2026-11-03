// The entity delta applier both transports share (src/entity-table.ts; format: acacia-daemon's crate docs).
import assert from 'node:assert/strict'
import { test } from 'node:test'

import { EntityTable } from '@acacia-rs/bot'

const row = (slot, x, health = null) => [slot, x, 64, 0, 0, 0, 0, 90, 0, health]
const zombie = (slot, id) => [slot, id, `-${id}`, 'minecraft:zombie', null]
const ids = (table) => table.list.map((e) => e.id).sort()

test('moves write numbers in place, entities keep their objects while listed', () => {
  const t = new EntityTable()
  t.apply({ version: 1, full: true, added: [zombie(0, '3'), [1, '4', '-4', 'minecraft:player', 'Steve']], numbers: [row(0, 1, 20), row(1, 2)] })
  const [three, four] = [t.byId['3'], t.byId['4']]
  assert.deepEqual({ ...three }, { id: '3', uniqueId: '-3', kind: 'minecraft:zombie', position: { x: 1, y: 64, z: 0 }, velocity: { x: 0, y: 0, z: 0 }, yaw: 90, pitch: 0, health: 20 })
  assert.equal(four.username, 'Steve')
  assert.equal(four.health, undefined, 'null health reads as absent')
  const [list, byId, members] = [t.list, t.byId, t.members]

  t.apply({ version: 2, numbers: [row(1, 7)] })
  assert.equal(four.position.x, 7)
  assert.equal(t.members, members, 'a move is no membership change')
  t.apply({ version: 3, added: [zombie(2, '5')], numbers: [row(2, 3)] })
  assert.equal(t.byId['3'], three, 'others keep their objects')
  assert.equal(t.list, list)
  assert.equal(t.byId, byId, 'the same objects, updated in place')
  assert.notEqual(t.members, members)
  assert.equal(t.version, 3)
})

test('removals keep the entity its last numbers and free the slot for a new object', () => {
  const t = new EntityTable()
  t.apply({ version: 1, full: true, added: [zombie(0, '3'), zombie(1, '4'), zombie(2, '5')], numbers: [row(0, 1), row(1, 2), row(2, 3)] })
  const three = t.byId['3']
  t.apply({ version: 2, removed: [row(0, 9)], added: [zombie(0, '6')], numbers: [row(0, 4)] })
  assert.equal(three.position.x, 9, 'the removed row\'s numbers, not the slot\'s new ones')
  assert.equal(t.byId['3'], undefined)
  assert.equal(t.byId['6'].position.x, 4)
  assert.deepEqual(ids(t), ['4', '5', '6'])
  t.apply({ version: 3, removed: [row(1, 2)] })
  assert.deepEqual(ids(t), ['5', '6'], 'the list stays dense')
  assert.equal(t.nearest(4.2, 64, 0, () => true), t.byId['6'])
  assert.equal(t.nearest(4.2, 64, 0, (e) => e.id === '5'), t.byId['5'])
})

test('a full delta keeps entities re-added in their slot and drops the rest', () => {
  const t = new EntityTable()
  t.apply({ version: 1, full: true, added: [zombie(0, '3'), zombie(1, '4')], numbers: [row(0, 1), row(1, 2)] })
  const [three, four] = [t.byId['3'], t.byId['4']]
  t.apply({ version: 40, full: true, added: [zombie(0, '3'), zombie(1, '7')], numbers: [row(0, 5), row(1, 6)] })
  assert.equal(t.byId['3'], three, 'same slot and id: the same object')
  assert.equal(three.position.x, 5)
  assert.equal(t.byId['4'], undefined)
  assert.equal(four.position.x, 2, 'dropped with the numbers it had')
  assert.deepEqual(ids(t), ['3', '7'])
})

test('a native reader hands in the array the addon wrote', () => {
  const t = new EntityTable()
  t.apply({ version: 1, full: true, added: [zombie(40, '3')], numbers: [row(40, 1)] })
  assert.ok(t.numbers.length >= 41 * 9, 'grows to fit the slot')
  const written = new Float64Array(64 * 9)
  written.set(row(40, 8).slice(1).map((n) => n ?? NaN), 40 * 9)
  t.apply({ version: 2 }, written)
  assert.equal(t.byId['3'].position.x, 8)
})
