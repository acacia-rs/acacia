// Shared by the benches (method: RESULTS.md): sapling's load server, spaced joins, /proc readings, percentiles.
import { spawn } from 'node:child_process'
import { readdirSync, readFileSync } from 'node:fs'
import { loadavg } from 'node:os'

export const env = (key, fallback) => process.env[key] ?? fallback
export const num = (key, fallback) => Number(env(key, fallback))
export const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
export const mb = (bytes) => Math.round((bytes / 1048576) * 10) / 10
export const round = (v, digits = 1) => Math.round(v * 10 ** digits) / 10 ** digits
export const log = (...args) => console.error(`[${new Date().toISOString().slice(11, 19)}]`, ...args)
export const load = () => loadavg().map((l) => round(l, 2))

/** One JSON line on stdout: the bench's row (bench/matrix.sh appends it to results/<bench>.jsonl); TAG labels a build. */
export const emit = (row) => console.log(JSON.stringify({ ...row, tag: env('TAG', null), at: new Date().toISOString(), node: process.version }))

/**
 * A fresh sapling `load_server` (`$SAPLING/target/release/examples/load_server <port> <clocks> <mobs>`): a stone
 * floor, `clocks` observer clocks, `mobs` strolling pigs. `ADDRESS=host:port` uses a running server instead.
 */
export async function startServer({ port, clocks = 0, mobs = 0 }) {
  if (process.env.ADDRESS) {
    const [host, p] = process.env.ADDRESS.split(':')
    return { host, port: Number(p), pid: null, stop: async () => {} }
  }
  const sapling = env('SAPLING', '/srv/agents/work/acacia/sapling-load')
  const child = spawn(`${sapling}/target/release/examples/load_server`, [port, clocks, mobs].map(String), {
    env: { ...process.env, RUST_LOG: 'info' },
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  const kill = () => child.kill('SIGKILL')
  process.once('exit', kill)
  let out = ''
  const started = new Promise((resolve, reject) => {
    const take = (d) => {
      if (out.length < 65536) out += d
      if (/\bstarted\b/.test(out)) resolve()
    }
    child.stdout.on('data', take)
    child.stderr.on('data', take)
    child.once('exit', (code) => reject(new Error(`load_server exited (${code}): ${out.slice(-400)}`)))
    setTimeout(() => reject(new Error(`load_server did not start in 60 s: ${out.slice(-400)}`)), 60_000).unref()
  })
  await started
  const stop = async () => {
    process.off('exit', kill)
    if (child.exitCode !== null) return
    const exited = new Promise((resolve) => child.once('exit', resolve))
    child.kill('SIGINT')
    const timer = setTimeout(kill, 10_000)
    await exited
    clearTimeout(timer)
  }
  return { host: '127.0.0.1', port, pid: child.pid, stop, log: () => out }
}

/** Starts `make(i)` for i < n, `spacingMs` apart; a join that fails or takes over `timeoutMs` counts as failed. */
export async function joinBots(n, spacingMs, make, timeoutMs = 60_000) {
  const t0 = performance.now()
  const bots = []
  const errors = {}
  await Promise.all(
    Array.from({ length: n }, async (_, i) => {
      await sleep(i * spacingMs)
      let timer
      const timeout = new Promise((_, reject) => {
        timer = setTimeout(() => reject(Object.assign(new Error('join timeout'), { code: 'JOIN_TIMEOUT' })), timeoutMs)
      })
      try {
        bots.push(await Promise.race([make(i), timeout]))
      } catch (e) {
        const key = e.code ?? e.message
        errors[key] = (errors[key] ?? 0) + 1
      } finally {
        clearTimeout(timer)
      }
    }),
  )
  return { bots, spawned: bots.length, failed: n - bots.length, errors, joinS: round((performance.now() - t0) / 1000, 2) }
}

const fieldsKb = (path, keys) => {
  const text = readFileSync(path, 'utf8')
  return Object.fromEntries(keys.map((k) => [k, Number(text.match(new RegExp(`^${k}:\\s+(\\d+)`, 'm'))?.[1])]))
}
export const statusKb = (pid = 'self') => fieldsKb(`/proc/${pid}/status`, ['VmRSS', 'VmHWM', 'RssAnon', 'RssFile'])
export const smapsKb = (pid = 'self') => fieldsKb(`/proc/${pid}/smaps_rollup`, ['Rss', 'Pss', 'Anonymous', 'Private_Dirty'])

export function memoryMb() {
  const m = process.memoryUsage()
  return { rss: mb(m.rss), heapTotal: mb(m.heapTotal), heapUsed: mb(m.heapUsed), external: mb(m.external), arrayBuffers: mb(m.arrayBuffers) }
}

/** utime + stime in clock ticks (100 Hz) from a /proc stat line; the comm may hold spaces and parens. */
function statTicks(text) {
  const close = text.lastIndexOf(')')
  const fields = text.slice(close + 2).split(' ')
  return { comm: text.slice(text.indexOf('(') + 1, close), ticks: Number(fields[11]) + Number(fields[12]) }
}

export const pidTicks = (pid) => (pid ? statTicks(readFileSync(`/proc/${pid}/stat`, 'utf8')).ticks : null)

/** Per-thread CPU ticks of this process, by tid. */
export function threadTicks() {
  const out = new Map()
  for (const tid of readdirSync('/proc/self/task')) {
    try {
      out.set(tid, statTicks(readFileSync(`/proc/self/task/${tid}/stat`, 'utf8')))
    } catch {
      // The thread exited between the listing and the read.
    }
  }
  return out
}

/** % of one core per thread group over `seconds`: `js` is the main thread, the rest grouped by name. */
export function threadCpu(before, after, seconds) {
  const groups = {}
  for (const [tid, { comm, ticks }] of after) {
    const group = tid === String(process.pid) ? 'js' : comm.replace(/[-\d]+$/, '') || comm
    groups[group] = (groups[group] ?? 0) + ticks - (before.get(tid)?.ticks ?? 0)
  }
  return Object.fromEntries(Object.entries(groups).map(([g, t]) => [g, round(t / seconds)]))
}

export function percentiles(values, ps = [50, 99]) {
  if (values.length === 0) return Object.fromEntries(ps.map((p) => [`p${p}`, null]))
  const sorted = [...values].sort((a, b) => a - b)
  return Object.fromEntries(ps.map((p) => [`p${p}`, sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))]]))
}

export const median = (values) => percentiles(values, [50]).p50
