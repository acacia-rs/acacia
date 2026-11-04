// The generated BlockName / ItemName unions, checked by compiling test/types/names-check.ts against dist/.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { dirname, join } from 'node:path'
import { test } from 'node:test'

import { pkg } from './hello-run.mjs'

test('name unions type-check vanilla, namespaced and custom names, and reject typos', () => {
  // typescript's `exports` hides bin/: resolve the package root instead.
  const tsc = join(dirname(createRequire(join(pkg, 'package.json')).resolve('typescript/package.json')), 'bin/tsc')
  const run = spawnSync(process.execPath, [tsc, '-p', join(pkg, 'test/types')], { cwd: pkg, encoding: 'utf8' })
  assert.equal(run.status, 0, run.stdout + run.stderr)
})
