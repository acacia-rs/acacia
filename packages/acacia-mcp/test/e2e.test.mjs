// An MCP client drives `acacia-mcp` (the stdio CLI, as Claude Desktop would) against acacia-testserver's FakeServer.
// Needs @acacia/bot's addon built with TestServer (`npm run build:test`); prints the session as a transcript.
import assert from 'node:assert/strict'
import { dirname, join } from 'node:path'
import { test } from 'node:test'
import { setTimeout as sleep } from 'node:timers/promises'
import { fileURLToPath } from 'node:url'

import { Client } from '@modelcontextprotocol/client'
import { StdioClientTransport } from '@modelcontextprotocol/client/stdio'

import * as native from '../../acacia-bot/native/index.js'
import { call } from './helpers.mjs'

const pkg = join(dirname(fileURLToPath(import.meta.url)), '..')
const skip = native.TestServer ? false : "@acacia/bot's addon was built without TestServer (npm run build:test)"

test('an MCP client walks a bot to a player, collects and places a block, chats, follows and cancels', { skip, timeout: 120_000 }, async () => {
  const server = native.TestServer.start()
  const transport = new StdioClientTransport({
    command: process.execPath,
    args: ['dist/cli.js', '--host', '127.0.0.1', '--port', String(server.port), '--username', 'Claude', '--transport', 'raknet', '--offline'],
    cwd: pkg,
    stderr: 'pipe',
  })
  let stderr = ''
  transport.stderr.on('data', (d) => (stderr += d))
  const client = new Client({ name: 'e2e', version: '0' })
  const transcript = []
  const tool = async (name, args = {}, options) => {
    const result = await call(client, name, args, options)
    transcript.push(`> ${name} ${JSON.stringify(args)}\n${result.isError ? '[error] ' : ''}${result.text}`)
    return result
  }
  async function until(what, check, ms = 20_000) {
    const deadline = Date.now() + ms
    for (;;) {
      const value = await check()
      if (value) return value
      if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}\n${transcript.join('\n')}\n${stderr}`)
      await sleep(100)
    }
  }
  try {
    await client.connect(transport)
    const { tools } = await client.listTools()
    transcript.push(`tools: ${tools.map((t) => t.name).join(', ')}`)
    for (const name of ['status', 'goto', 'follow', 'readChat', 'findBlocks', 'dig', 'equip', 'placeBlock']) {
      assert.ok(tools.some((t) => t.name === name), name)
    }
    assert.ok(!tools.some((t) => t.name === 'command'), 'command needs --allow-commands')

    await until('the bot in the world', async () => (await call(client, 'status')).structured?.position)
    server.layFloor(8)
    const [fx, fy, fz] = server.spawnFeet().map(Math.floor)
    await until('the floor under the bot', async () => (await call(client, 'findBlocks', { matching: ['stone'], maxDistance: 2 })).structured?.blocks?.length)
    const status = await tool('status')
    assert.match(status.text, /^Claude at /)

    await tool('chat', { message: 'hello from MCP' })
    await until('the chat at the server', () => server.receivedChat().includes('hello from MCP'))
    server.chat('Steve', 'hi Claude')
    assert.equal((await tool('readChat', { wait: 10 })).text, '<Steve> hi Claude')

    // In place of crafting (not in the API yet): collect a block with findBlocks + dig, then build with equip + placeBlock.
    const found = await tool('findBlocks', { matching: ['stone'], maxDistance: 3, count: 50 })
    assert.ok(found.structured.blocks.some((b) => b.x === fx + 1 && b.y === fy - 1 && b.z === fz))
    const digging = tool('dig', { x: fx + 1, y: fy - 1, z: fz })
    await sleep(1000)
    server.setBlock(fx + 1, fy - 1, fz, 'minecraft:air')
    assert.ok(!(await digging).isError, transcript.at(-1))

    server.give(0, 'minecraft:stone', 4)
    await until('stone in the inventory', async () => (await call(client, 'inventory')).text.includes('minecraft:stone'))
    await tool('inventory')
    assert.ok(!(await tool('equip', { item: 'stone' })).isError)
    const before = server.inventoryTransactions()
    const placing = tool('placeBlock', { x: fx - 1, y: fy - 1, z: fz, face: 'up' })
    await until('the placement packet', () => server.inventoryTransactions() > before)
    server.setBlock(fx - 1, fy, fz, 'minecraft:stone')
    assert.ok(!(await placing).isError, transcript.at(-1))

    server.addPlayer('Steve', 77, fx + 0.5, fy, fz + 6.5)
    await until('Steve in view', async () => (await call(client, 'status')).structured?.players?.length)
    await tool('status')
    const walked = await tool('goto', { player: 'Steve' })
    assert.match(walked.text, /^arrived/)
    const [steve] = (await tool('status')).structured.players
    assert.ok(steve.distance <= 3, `within 2 of Steve's cell: ${steve.distance}`)

    const aborted = new AbortController()
    const following = tool('follow', { entity: 'Steve', seconds: 60 }, { signal: aborted.signal })
    await sleep(1000)
    const cancelledAt = Date.now()
    aborted.abort()
    await assert.rejects(following)
    transcript.push('(client cancelled the follow request)')
    assert.ok(!(await tool('waitForTicks', { ticks: 2 })).isError, 'the bot is free again')
    assert.ok(Date.now() - cancelledAt < 5_000, 'the cancel reached the bot quickly')

    assert.match((await tool('disconnect')).text, /^Claude disconnected/)
    console.log(transcript.join('\n'))
  } finally {
    await client.close().catch(() => {})
    server.stop()
  }
})
