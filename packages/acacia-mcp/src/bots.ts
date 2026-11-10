import { AcaciaError, closeAll, createBot, type Auth, type Bot, type ChatLine, type Form, type MsaCodePrompt, type Transport } from '@acacia-rs/bot'

import { badParams } from './tool.js'

export interface ConnectOptions {
  host?: string
  port?: number
  username?: string
  transport?: 'auto' | 'raknet' | 'nethernet'
  auth?: Auth
  profilesFolder?: string
}

/** What `connectOrPrompt` got first: the bot, or the sign-in a person must finish while it keeps connecting. */
export type Connecting = { handle: BotHandle } | { prompt: MsaCodePrompt; username: string }

/** Unread chat lines kept per bot; older ones are dropped first. */
const CHAT_KEEP = 200

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
  /** The device code a connecting bot waits on, and who waits to hear of one. */
  readonly #prompts = new Map<string, MsaCodePrompt>()
  readonly #promptWaiters = new Map<string, Set<(prompt: MsaCodePrompt) => void>>()
  /** How long `connectOrPrompt` waits on a person who already has a code. */
  signInWaitMs = 50_000

  /** `via`: the transport to run bots on (default: `@acacia-rs/bot`'s shared in-process host). */
  constructor(
    readonly defaults: ConnectOptions = {},
    readonly via?: Transport,
  ) {}

  /** Connects a bot (or returns the one already connected or connecting under that name). */
  connect(options: ConnectOptions = {}): Promise<BotHandle> {
    const { host, port, username, transport, auth, profilesFolder } = { ...this.defaults, ...options }
    if (!host || !username) return Promise.reject(badParams('connect needs a host and a username (no defaults were given)'))
    const key = username.toLowerCase()
    const known = this.#bots.get(key) ?? this.#connecting.get(key)
    if (known) return Promise.resolve(known)
    const onMsaCode = (prompt: MsaCodePrompt) => {
      process.stderr.write(`acacia-mcp: ${username}: ${prompt.message}\n`)
      this.#prompts.set(key, prompt)
      for (const wake of this.#promptWaiters.get(key) ?? []) wake(prompt)
    }
    const connecting = createBot({ host, port, username, transport, auth, profilesFolder, onMsaCode, via: this.via })
      .then(async (bot) => {
        const handle = new BotHandle(bot)
        await bot.ready()
        this.#bots.set(key, handle)
        bot.once('end', () => this.#bots.delete(key))
        return handle
      })
      .finally(() => {
        this.#connecting.delete(key)
        this.#prompts.delete(key)
      })
    this.#connecting.set(key, connecting)
    return connecting
  }

  /**
   * `connect`, but settling early with the device code when a person must sign the account in first. While a code
   * is already out it waits up to `signInWaitMs` for the bot, then hands the same code back (MCP clients time out).
   */
  async connectOrPrompt(options: ConnectOptions = {}): Promise<Connecting> {
    const connecting = this.connect(options)
    const username = options.username ?? this.defaults.username ?? ''
    const key = username.toLowerCase()
    const known = this.#prompts.get(key)
    let wake!: (prompt: MsaCodePrompt) => void
    const prompted = new Promise<MsaCodePrompt>((resolve) => (wake = resolve))
    const waiters = this.#promptWaiters.get(key) ?? this.#promptWaiters.set(key, new Set()).get(key)!
    const timer = known && setTimeout(() => wake(this.#prompts.get(key) ?? known), this.signInWaitMs)
    if (!known) waiters.add(wake)
    try {
      return await Promise.race([connecting.then((handle) => ({ handle })), prompted.then((prompt) => ({ prompt, username }))])
    } finally {
      clearTimeout(timer)
      waiters.delete(wake)
      if (waiters.size === 0) this.#promptWaiters.delete(key)
    }
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
