// Running examples/ against a TestServer, shared by the in-process and the daemon suites.
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

/** examples/connection.mjs's variables for a TestServer, plus `env`. */
export const exampleEnv = (server, env = {}) => ({ ACACIA_HOST: '127.0.0.1', ACACIA_PORT: String(server.port), ACACIA_TRANSPORT: 'raknet', ...env })

/** Starts `node examples/<script> ...args` with `env` added, collecting its stdout and stderr. */
export function startExample(script, args = [], env = {}) {
  const child = spawn(process.execPath, [`examples/${script}`, ...args], { cwd: pkg, env: { ...process.env, ...env } })
  let out = ''
  child.stdout.on('data', (d) => (out += d))
  child.stderr.on('data', (d) => (out += d))
  const exited = once(child, 'exit')
  return {
    get out() {
      return out
    },
    /** Resolves with the first match of `pattern` in the output so far; the output is in the error on a timeout. */
    async waitFor(pattern, ms = 20_000) {
      let match
      await until(String(pattern), () => (match = pattern.exec(out)), ms).catch((e) => {
        throw new Error(`${e.message}\n${out}`)
      })
      return match
    },
    /** Resolves with the exit code, or `'still running'` after `ms`. */
    async exitCode(ms = 15_000) {
      const [code] = await Promise.race([exited, sleep(ms, ['still running'], { ref: false })])
      return code
    },
    kill: () => child.kill(),
  }
}

/** Runs examples/hello.mjs with `env` added: spawn, `!ping`, `!come` to a player, `!quit`, and it exits on its own. */
export async function runHello(native, env = {}) {
  const server = native.TestServer.start()
  const hello = startExample('hello.mjs', [`127.0.0.1:${server.port}`, 'Hello'], env)
  try {
    await hello.waitFor(/spawned as Hello/)
    server.chat('Steve', '!ping')
    await until('pong', () => server.receivedChat().includes('pong'))

    server.layFloor(8)
    const [x, y, z] = server.spawnFeet()
    server.addPlayer('Steve', 77, Math.floor(x) + 5.5, Math.floor(y), Math.floor(z) + 0.5)
    await hello.waitFor(/sees Steve/, 10_000)
    server.chat('Steve', '!come')
    await until('the walk to Steve', () => /here|no path|gave up/.test(server.receivedChat().join(' ')), 35_000).catch((e) => {
      throw new Error(`${e.message}\n${hello.out}`)
    })
    assert.match(hello.out, new RegExp(`coming to Steve at ${(Math.floor(x) + 5.5).toFixed(1)} `))
    assert.ok(server.receivedChat().includes('here'), `${hello.out}\n${server.receivedChat()}`)

    server.chat('Steve', '!quit')
    const quit = Date.now()
    const code = await hello.exitCode()
    assert.equal(code, 0, hello.out)
    assert.match(hello.out, /<Steve> !ping/)
    assert.match(hello.out, /ended: closed/)
    console.log(`${hello.out.trimEnd()}\n[exit ${code} ${Date.now() - quit} ms after !quit; server got ${JSON.stringify(server.receivedChat())}]`)
  } finally {
    hello.kill()
    server.stop()
  }
}
