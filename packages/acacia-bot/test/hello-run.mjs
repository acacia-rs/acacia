// The hello world's end-to-end run against a TestServer, shared by the in-process and the daemon suites.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { dirname, join } from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'
import { fileURLToPath } from 'node:url'

export const pkg = join(dirname(fileURLToPath(import.meta.url)), '..')

export async function until(what, check, ms = 10_000) {
  const deadline = Date.now() + ms
  while (!check()) {
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`)
    await sleep(20)
  }
}

/** Runs examples/hello.mjs with `env` added: spawn, `!ping`, `!come` to a player, `!quit`, and it exits on its own. */
export async function runHello(native, env = {}) {
  const server = native.TestServer.start()
  const child = spawn(process.execPath, ['examples/hello.mjs', `127.0.0.1:${server.port}`, 'Hello'], { cwd: pkg, env: { ...process.env, ...env } })
  let out = ''
  child.stdout.on('data', (d) => (out += d))
  child.stderr.on('data', (d) => (out += d))
  const exited = once(child, 'exit')
  try {
    await until('spawn', () => out.includes('spawned as Hello'), 20_000)
    server.chat('Steve', '!ping')
    await until('pong', () => server.receivedChat().includes('pong'))

    server.layFloor(8)
    const [x, y, z] = server.spawnFeet()
    server.addPlayer('Steve', 77, Math.floor(x) + 5.5, Math.floor(y), Math.floor(z) + 0.5)
    await until('Steve in view', () => out.includes('sees Steve'))
    server.chat('Steve', '!come')
    await until('the walk to Steve', () => /here|no path|gave up/.test(server.receivedChat().join(' ')), 35_000).catch((e) => {
      throw new Error(`${e.message}\n${out}`)
    })
    assert.match(out, new RegExp(`coming to Steve at ${(Math.floor(x) + 5.5).toFixed(1)} `))
    assert.ok(server.receivedChat().includes('here'), `${out}\n${server.receivedChat()}`)

    server.chat('Steve', '!quit')
    const quit = Date.now()
    const [code] = await Promise.race([exited, sleep(15_000, ['still running'], { ref: false })])
    assert.equal(code, 0, out)
    assert.match(out, /<Steve> !ping/)
    assert.match(out, /ended: closed/)
    console.log(`${out.trimEnd()}\n[exit ${code} ${Date.now() - quit} ms after !quit; server got ${JSON.stringify(server.receivedChat())}]`)
  } finally {
    child.kill()
    server.stop()
  }
}
