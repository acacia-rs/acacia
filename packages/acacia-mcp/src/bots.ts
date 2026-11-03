import { AcaciaError, closeAll, createBot, type Bot, type ChatLine, type Form, type Transport } from '@acacia-rs/bot'

import { badParams } from './tool.js'

export interface ConnectOptions {
  host?: string
  port?: number
  username?: string
  transport?: 'auto' | 'raknet' | 'nethernet'
}

/** Unread chat lines kept per bot; older ones are dropped first. */
const CHAT_KEEP = 200
/** One-tick waits `connect` spends at most on the first snapshot (a transport without reads never has one). */
const FIRST_SNAPSHOT_WAITS = 40

/** One connected bot and what the tools keep for it: unread chat, open forms, running actions. */
export class BotHandle {
  readonly unread: ChatLine[] = []
  readonly forms = new Map<number, Form>()
  readonly #actions = new Set<AbortController>()
  readonly #chatWaiters = new Set<() => void>()

  constructor(readonly bot: Bot) {
    bot.on('chat', (line) => {
      // BDS echoes the bot's own lines back to it.
      if (line.sender?.toLowerCase() === bot.username.toLowerCase()) return
      this.unread.push(line)
      if (this.unread.length > CHAT_KEEP) this.unread.shift()
      for (const wake of this.#chatWaiters) wake()
    })
    bot.on('form', (form) => this.forms.set(form.id, form))
  }

  get name(): string {
    return this.bot.username
  }

  /** Runs one action under a signal that aborts on `request`, on `stop()`, or (when given) after `ms`. */
  async action<T>(request: AbortSignal, ms: number | undefined, body: (signal: AbortSignal) => Promise<T>): Promise<T> {
    const stop = new AbortController()
    this.#actions.add(stop)
    const deadline = ms === undefined ? undefined : setTimeout(() => stop.abort(new DOMException(`gave up after ${ms / 1000} s`, 'TimeoutError')), ms)
    try {
      return await body(AbortSignal.any([request, stop.signal]))
    } finally {
      clearTimeout(deadline)
      this.#actions.delete(stop)
    }
  }

  /** Aborts every running action; returns how many there were. */
  stop(): number {
    const n = this.#actions.size
    for (const action of this.#actions) action.abort(new DOMException('stopped by the stop tool', 'AbortError'))
    return n
  }

  /** The unread chat lines, marked read; waits up to `ms` for one when there are none. */
  async takeChat(ms: number, signal: AbortSignal): Promise<ChatLine[]> {
    if (this.unread.length === 0 && ms > 0 && !signal.aborted) {
      await new Promise<void>((resolve) => {
        const done = () => {
          clearTimeout(timer)
          signal.removeEventListener('abort', done)
          this.#chatWaiters.delete(done)
          resolve()
        }
        const timer = setTimeout(done, ms)
        signal.addEventListener('abort', done, { once: true })
        this.#chatWaiters.add(done)
      })
    }
    return this.unread.splice(0)
  }
}

/** The bots this server runs, by username (case-insensitive); `defaults` come from the command line. */
export class BotRegistry {
  readonly #bots = new Map<string, BotHandle>()
  readonly #connecting = new Map<string, Promise<BotHandle>>()

  /** `via`: the transport to run bots on (default: `@acacia-rs/bot`'s shared in-process host). */
  constructor(
    readonly defaults: ConnectOptions = {},
    readonly via?: Transport,
  ) {}

  /** Connects a bot (or returns the one already connected or connecting under that name). */
  connect(options: ConnectOptions = {}): Promise<BotHandle> {
    const { host, port, username, transport } = { ...this.defaults, ...options }
    if (!host || !username) return Promise.reject(badParams('connect needs a host and a username (no defaults were given)'))
    const key = username.toLowerCase()
    const known = this.#bots.get(key) ?? this.#connecting.get(key)
    if (known) return Promise.resolve(known)
    const connecting = createBot({ host, port, username, transport, via: this.via })
      .then(async (bot) => {
        const handle = new BotHandle(bot)
        // createBot resolves before the first snapshot, and waitForTicks is a timed pause that may end before a tick.
        for (let i = 0; i < FIRST_SNAPSHOT_WAITS && !bot.hot; i++) await bot.waitForTicks(1)
        this.#bots.set(key, handle)
        bot.once('end', () => this.#bots.delete(key))
        return handle
      })
      .finally(() => this.#connecting.delete(key))
    this.#connecting.set(key, connecting)
    return connecting
  }

  /** The bot named `name`, or the only one; connects the default bot when none is connected or connecting. */
  async get(name?: string): Promise<BotHandle> {
    if (name !== undefined) {
      const key = name.toLowerCase()
      const handle = this.#bots.get(key) ?? (await this.#connecting.get(key))
      if (!handle) throw new AcaciaError('UNKNOWN_BOT', `no bot named ${name}; connected: ${this.names().join(', ') || 'none'}`)
      return handle
    }
    const all = [...this.#bots.values()]
    if (all.length === 1 && this.#connecting.size === 0) return all[0]
    if (all.length === 0 && this.#connecting.size === 1) return [...this.#connecting.values()][0]
    if (all.length === 0 && this.#connecting.size === 0) {
      if (this.defaults.host && this.defaults.username) return this.connect()
      throw new AcaciaError('UNKNOWN_BOT', 'no bot is connected: call connect first')
    }
    throw badParams(`several bots are connected or connecting (${this.names().join(', ')}): name one with \`bot\``)
  }

  names(): string[] {
    return [...this.#bots.values()].map((h) => h.name)
  }

  /** Disconnects bot `name` (or the only one); resolves once it has ended. */
  async disconnect(name?: string): Promise<string> {
    const handle = await this.get(name)
    handle.stop()
    await handle.bot.close()
    return handle.name
  }

  /** Disconnects every bot and shuts the host down. */
  async close(): Promise<void> {
    await Promise.allSettled([...this.#connecting.values()])
    await Promise.allSettled([...this.#bots.values()].map((h) => h.bot.close()))
    if (!this.via) await closeAll()
  }
}
