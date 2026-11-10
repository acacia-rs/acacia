// Markdown tables from results/*.jsonl: per cell the median over its reps (RESULTS.md quotes these).
// usage: node bench/summarize.mjs [results dir]
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { median, round } from './common.mjs'

const dir = process.argv[2] ?? join(dirname(fileURLToPath(import.meta.url)), 'results')
const rows = (bench) => {
  const path = join(dir, `${bench}.jsonl`)
  if (!existsSync(path)) return []
  return readFileSync(path, 'utf8').split('\n').filter((l) => l.startsWith('{')).map((l) => JSON.parse(l))
}
const groupBy = (list, key) => list.reduce((m, r) => m.set(key(r), [...(m.get(key(r)) ?? []), r]), new Map())
const med = (list, pick) => {
  const values = list.map(pick).filter((v) => typeof v === 'number')
  return values.length ? round(median(values), 2) : '–'
}
const table = (head, lines) => [`| ${head.join(' | ')} |`, `|${head.map(() => '---').join('|')}|`, ...lines.map((l) => `| ${l.join(' | ')} |`)].join('\n')

const events = rows('events')
if (events.length) {
  const cells = groupBy(events, (r) => `${r.tag ?? ''}|${r.bots}|${r.clocks}|${r.mobs}|${r.shards ?? ''}|${r.read ?? 'none'}`)
  console.log('## events (median of reps)\n')
  console.log(table(
    ['tag', 'bots', 'clocks/mobs', 'read', 'reps', 'spawned', 'events/s', 'ticks/bot/s', 'moved ents/s', 'batches/s', 'rec/batch', 'JS CPU %', 'proc CPU %', 'server CPU %', 'JS µs/ev', 'loop p50/p99 ms', 'imm RTT p50/p99 ms', 'wait1 RTT p50/p99 ms', 'lagged', 'ended', 'load'],
    [...cells.values()].map((c) => [
      c[0].tag ?? '–', c[0].bots, `${c[0].clocks}/${c[0].mobs}`, c[0].read ?? 'none', c.length, med(c, (r) => r.spawned), med(c, (r) => r.eventsPerS), med(c, (r) => r.ticksPerBotS),
      med(c, (r) => r.perS.movedEntities), med(c, (r) => r.perS.batches), med(c, (r) => r.recordsPerBatch), med(c, (r) => r.cpuPct.js),
      med(c, (r) => r.cpuPct.process), med(c, (r) => r.cpuPct.server), med(c, (r) => r.jsUsPerEvent),
      `${med(c, (r) => r.loopDelayMs.p50)} / ${med(c, (r) => r.loopDelayMs.p99)}`,
      `${med(c, (r) => r.rttMs.immediate?.p50)} / ${med(c, (r) => r.rttMs.immediate?.p99)}`,
      `${med(c, (r) => r.rttMs.waitForTicks1?.p50)} / ${med(c, (r) => r.rttMs.waitForTicks1?.p99)}`,
      c.map((r) => r.lagged).join(','), c.map((r) => Object.values(r.ends ?? {}).reduce((n, e) => n + e.count, 0)).join(','),
      c.map((r) => r.load[0]).join(','),
    ]),
  ))
  console.log('\nthreads (% of a core, last rep):', JSON.stringify(Object.fromEntries([...cells].map(([k, c]) => [k, c.at(-1).cpuPct.threads]))))
}

const memory = rows('memory').filter((r) => !r.capped)
if (memory.length) {
  const variant = (r) => [r.physics ? 'phys on' : 'phys off', r.snapshots === false && 'snapshots off', r.shards && `${r.shards} shard`, r.mallocArenaMax && `arenas ${r.mallocArenaMax}`].filter(Boolean).join(', ')
  const cells = groupBy(memory, (r) => `${variant(r)}|${r.bots}`)
  console.log('\n## memory (median of reps, MB)\n')
  console.log(table(
    ['variant', 'bots', 'reps', 'spawned', 'ended', 'rss', 'smaps Rss', 'Anonymous', 'heapUsed', 'external', 'VmHWM', 'join s', 'loaded rss', 'bare rss', 'server rss'],
    [...cells.values()].map((c) => [
      variant(c[0]), c[0].bots, c.length, c.map((r) => r.spawned).join(','), c.map((r) => r.ended).join(','), med(c, (r) => r.memoryMb.rss),
      med(c, (r) => r.smapsKb.Rss / 1024), med(c, (r) => r.smapsKb.Anonymous / 1024), med(c, (r) => r.memoryMb.heapUsed),
      med(c, (r) => r.memoryMb.external), med(c, (r) => r.statusKb.VmHWM / 1024), med(c, (r) => r.joinS),
      med(c, (r) => r.loaded.memoryMb.rss), med(c, (r) => r.bare.memoryMb.rss), med(c, (r) => (r.serverRssKb ?? NaN) / 1024),
    ]),
  ))
}

for (const r of rows('reads')) {
  console.log(`\n## reads (${r.tag ?? '–'}, ${r.at}, ${r.entities} entities, ${r.stacks} stacks, load ${r.load[0]})\n`)
  console.log(table(['read', 'ns/op'], Object.entries(r.nsPerOp)))
  console.log()
  console.log(table(['sampled read', 'misses', 'miss p50/p99 ns', 'hits', 'hit p50/p99 ns'],
    Object.entries(r.sampled).map(([k, s]) => [k, s.misses, `${s.missNs.p50} / ${s.missNs.p99}`, s.hits, `${s.hitNs.p50} / ${s.hitNs.p99}`])))
}
