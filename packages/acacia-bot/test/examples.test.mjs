// examples/: every script parses, the TypeScript one type-checks, and follow.mjs and crafter.mjs run end to end
// against FakeServer (needs `npm run build:test`). ws-persist.mjs runs in ws-e2e.test.mjs, against the daemon.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { readdirSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, join } from 'node:path'
import { test } from 'node:test'
import { setTimeout as sleep } from 'node:timers/promises'

import * as native from '../native/index.js'
import { exampleEnv, pkg, startExample, until } from './hello-run.mjs'

const skip = native.TestServer ? false : 'the addon was built without TestServer (npm run build:test)'
const scripts = readdirSync(join(pkg, 'examples')).filter((name) => name.endsWith('.mjs'))

test('every example script parses', () => {
  assert.ok(scripts.length >= 7, scripts.join(' '))
  for (const name of scripts) {
    const check = spawnSync(process.execPath, ['--check', join(pkg, 'examples', name)], { encoding: 'utf8' })
    assert.equal(check.status, 0, `${name}: ${check.stderr}`)
  }
})

test('the TypeScript example type-checks against dist/', () => {
  // typescript's `exports` hides bin/: resolve the package root instead.
  const tsc = join(dirname(createRequire(join(pkg, 'package.json')).resolve('typescript/package.json')), 'bin/tsc')
  const run = spawnSync(process.execPath, [tsc, '-p', join(pkg, 'examples/typescript'), '--noEmit'], { cwd: pkg, encoding: 'utf8' })
  assert.equal(run.status, 0, run.stdout + run.stderr)
})

test('follow.mjs follows the player who says !follow, stops on !stop and leaves on !quit', { skip, timeout: 60_000 }, async () => {
  const server = native.TestServer.start()
  const follower = startExample('follow.mjs', ['Follower'], exampleEnv(server))
  try {
    await follower.waitFor(/spawned as Follower/)
    server.layFloor(8)
    const [x, y, z] = server.spawnFeet().map(Math.floor)
    server.addPlayer('Steve', 77, x + 5.5, y, z + 0.5)
    await follower.waitFor(/sees Steve/)
    server.chat('Steve', '!follow')
    await follower.waitFor(/following Steve/)
    server.moveEntity(77, x + 8.5, y, z + 0.5)
    await sleep(1000)
    server.chat('Steve', '!stop')
    await follower.waitFor(/stopped following Steve/)
    await until('the "stopped" chat', () => server.receivedChat().includes('stopped'))
    assert.doesNotMatch(follower.out, /lost Steve/)

    server.chat('Steve', '!quit')
    assert.equal(await follower.exitCode(), 0, follower.out)
    assert.match(follower.out, /ended: closed/)
  } finally {
    follower.kill()
    server.stop()
  }
})

test('crafter.mjs chops logs, crafts and places a table, and crafts a wooden pickaxe at it', { skip, timeout: 120_000 }, async () => {
  const server = native.TestServer.start()
  server.serveInventory()
  const crafter = startExample('crafter.mjs', ['Crafter'], exampleEnv(server, { ACACIA_HUMAN_SCALE: '0' }))
  try {
    await crafter.waitFor(/spawned as Crafter/)
    server.layFloor(8)
    const [x, y, z] = server.spawnFeet().map(Math.floor)
    server.setBlock(x + 1, y, z, 'minecraft:oak_log')
    const [, ...log] = await crafter.waitFor(/digging oak_log at (-?\d+) (-?\d+) (-?\d+)/)
    assert.deepEqual(log.map(Number), [x + 1, y, z])
    // FakeServer neither breaks blocks nor drops items: break it, and hand over the logs as if picked up.
    await sleep(1000)
    server.setBlock(x + 1, y, z, 'minecraft:air')
    server.give(1, 'minecraft:oak_log', 3)
    const placements = server.inventoryTransactions()

    const [, ...table] = await crafter.waitFor(/placing crafting_table at (-?\d+) (-?\d+) (-?\d+)/, 60_000)
    await until('the placement packet', () => server.inventoryTransactions() > placements)
    const [tx, ty, tz] = table.map(Number)
    server.setBlock(tx, ty, tz, 'minecraft:crafting_table')
    server.addContainer(tx, ty, tz, 'craftingTable', [])
    await crafter.waitFor(/crafted 1 wooden_pickaxe/, 60_000)
    assert.match(crafter.out, /crafted 12 oak_planks/)
    assert.equal(await crafter.exitCode(), 0, crafter.out)
    assert.ok(server.receivedChat().includes('made a wooden pickaxe'), crafter.out)
  } finally {
    crafter.kill()
    server.stop()
  }
})
