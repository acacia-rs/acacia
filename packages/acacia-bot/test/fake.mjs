// A fake Transport for the TS layer's tests: records commands, lets the test push records and serve sync reads.
import { setImmediate as tick } from 'node:timers/promises'

import { AcaciaError, createBot } from '@acacia/bot'

export class FakeTransport {
  sent = []
  keepAlive = []
  refuse = null
  /** Per bot: what the sync reads return (`hot`, `cold`, `entities`, ...); each read hands back the same object. */
  state = {}
  reads = {
    hot: (bot) => this.state[bot]?.hot ?? null,
    cold: (bot) => this.state[bot]?.cold ?? null,
    entities: (bot) => this.state[bot]?.entities ?? null,
    blockAt: (bot, x, y, z) => this.state[bot]?.blocks?.[`${x},${y},${z}`] ?? null,
    itemTable: (bot) => this.state[bot]?.items ?? null,
    findBlocks: (bot, query) => {
      this.queries.push(query)
      return this.state[bot]?.found ?? []
    },
  }
  queries = []
  open(listener) {
    this.listener = listener
  }
  send(command) {
    if (this.refuse) throw new AcaciaError(this.refuse, 'refused')
    this.sent.push(command)
  }
  setKeepAlive(on) {
    this.keepAlive.push(on)
  }
  close() {
    this.listener.closed()
    return Promise.resolve()
  }
  push(...records) {
    this.listener.records(records)
  }
  last(method) {
    return this.sent.findLast((c) => c.method === method)
  }
  done(call, result = { ok: null }, bot = 'bot-1') {
    return { type: 'done', bot, call, result }
  }
  event(event, seq = 1, bot = 'bot-1') {
    return { type: 'event', bot, seq, event }
  }
  /** Answers the last call of `method` with `result` and returns its command. */
  answer(method, result = { ok: null }) {
    const command = this.last(method)
    this.push(this.done(command.call, result))
    return command
  }
}

export async function connected(options = {}) {
  const via = new FakeTransport()
  const creating = createBot({ host: 'h', username: 'U', via, ...options })
  via.push(via.event({ type: 'spawn' }), via.done(via.last('createBot').call, { ok: { bot: 'bot-1' } }))
  const bot = await creating
  await tick()
  return { via, bot }
}

export const vec = (x, y, z) => ({ x, y, z })

export function entity(id, kind, position, extra = {}) {
  return { id, uniqueId: `-${id}`, kind, position, velocity: vec(0, 0, 0), yaw: 0, pitch: 0, ...extra }
}
