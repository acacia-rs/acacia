// Loading the package: the addon only with a NativeTransport (test/no-addon.mjs checks), and require() from CJS.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { join } from 'node:path'
import { test } from 'node:test'

import { pkg } from './hello-run.mjs'

const noAddon = join(pkg, 'test/no-addon.mjs')
const run = (args) => spawnSync(process.execPath, args, { cwd: pkg, encoding: 'utf8' })

test('importing the package and building a WsTransport loads no addon; a NativeTransport does', () => {
  const ws = run(['--import', noAddon, '--input-type=module', '-e', "const { WsTransport } = await import('@acacia-rs/bot'); new WsTransport('ws://127.0.0.1:1')"])
  assert.equal(ws.status, 0, ws.stderr)
  const native = run(['--import', noAddon, '--input-type=module', '-e', "const { NativeTransport } = await import('@acacia-rs/bot'); await new NativeTransport({ shards: 1 }).close()"])
  assert.equal(native.status, 1, 'the check sees the addon once it is loaded')
  assert.match(native.stderr, /the native addon was loaded/)
})

test('CommonJS require()s the package (require(esm), Node >= 22.12)', () => {
  const cjs = run(['--input-type=commonjs', '-e', "const { createBot, WsTransport } = require('@acacia-rs/bot'); if (typeof createBot !== 'function' || typeof WsTransport !== 'function') process.exit(2)"])
  assert.equal(cjs.status, 0, cjs.stderr)
})
