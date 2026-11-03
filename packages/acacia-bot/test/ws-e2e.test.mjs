// Against a real acacia-daemon (`cargo build -p acacia-daemon`; ACACIA_DAEMON overrides the binary's path) and a
// TestServer from the addon (`npm run build:test`): the hello world unchanged but for `ACACIA_VIA`, and a bot that
// outlives its script.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { join } from 'node:path'
import { test } from 'node:test'

import * as native from '../native/index.js'
import { attachBot, createBot, WsTransport } from '@acacia-rs/bot'
import { pkg, runHello, until } from './hello-run.mjs'

const daemonBin = process.env.ACACIA_DAEMON ?? join(pkg, '../../target/debug/acacia-daemon')
const skip = !native.TestServer
  ? 'the addon was built without TestServer (npm run build:test)'
  : !existsSync(daemonBin) && `no daemon at ${daemonBin} (cargo build -p acacia-daemon)`

/** Starts the daemon on a free port; resolves with its URL and a stop function. */
async function startDaemon() {
  const child = spawn(daemonBin, ['--listen', '127.0.0.1:0', '--shards', '1'], { stdio: ['ignore', 'pipe', 'inherit'] })
  const url = await new Promise((resolve, reject) => {
    let out = ''
    child.stdout.on('data', (d) => {
      out += d
      const ready = /listening on (ws:\/\/\S+)/.exec(out)
      if (ready) resolve(ready[1])
    })
    child.on('exit', (code) => reject(new Error(`the daemon exited (${code}): ${out}`)))
  })
  return { url, stop: () => child.kill() }
}

test('hello world over the daemon: the same script with ACACIA_VIA=ws://...', { skip, timeout: 90_000 }, async (t) => {
  const daemon = await startDaemon()
  t.after(daemon.stop)
  await runHello(native, { ACACIA_VIA: daemon.url })
})

test('a persisted bot outlives its connection and a new script attaches to it', { skip, timeout: 60_000 }, async (t) => {
  const daemon = await startDaemon()
  const server = native.TestServer.start()
  t.after(() => {
    daemon.stop()
    server.stop()
  })
  const first = new WsTransport(daemon.url)
  const bot = await createBot({ host: '127.0.0.1', port: server.port, username: 'Keeper', transport: 'raknet', physics: false, persist: true, via: first })
  await bot.waitForTicks(2)
  assert.ok(bot.hot?.alive, 'the mirror serves bot.hot')
  assert.throws(() => bot.blockAt({ x: 0, y: 0, z: 0 }), { code: 'NOT_POSSIBLE' })
  const { x, y, z } = bot.hot.position
  server.addEntity('minecraft:zombie', 90, x + 2, y, z)
  await until('the zombie in the mirror', () => bot.entities['90'] !== undefined)
  const zombie = bot.entities['90']
  server.moveEntity(90, x + 4, y, z)
  await until('the zombie moved', () => Math.abs(zombie.position.x - (x + 4)) < 1e-3)
  assert.equal(bot.entities['90'], zombie, 'a delta keeps the object')
  const ended = new Promise((resolve) => bot.once('end', resolve))
  await first.close()
  assert.match((await ended).message, /keeps running on the daemon/)

  const again = await attachBot({ via: daemon.url, username: 'Keeper' })
  assert.equal(again.id, bot.id)
  await until('the attached mirror', () => again.entities['90'] !== undefined)
  assert.ok(Math.abs(again.entities['90'].position.x - (x + 4)) < 1e-3, 'a full delta on attach')
  const lines = []
  again.on('chat', (line) => lines.push(line.plain))
  await again.waitForTicks(2)
  server.chat('Steve', 'welcome back')
  await again.waitForTicks(10)
  assert.deepEqual(lines, ['welcome back'])
  assert.ok(again.hot.tick > 0)
  await again.close()
  assert.ok(again.ended)
})
