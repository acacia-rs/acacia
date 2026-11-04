import { AcaciaError, errorFrom } from './errors.js'
import type { Command, MethodName, Params, Result } from './generated/methods.js'
import type { BotId } from './generated/types/BotId.js'
import type { Event } from './generated/types/Event.js'
import type { MsaCode } from './generated/types/MsaCode.js'
import type { Out } from './generated/types/Out.js'
import type { Snapshot } from './generated/types/Snapshot.js'
import type { SyncReads, Transport } from './transport.js'

/** acacia-daemon's `attach`: a running bot by id or username (contract: the package README). */
export interface AttachParams {
  /** The bot's id. */
  bot?: string
  /** Or the username it was created with. */
  username?: string
}

/** What the daemon answers `attach` with. */
export interface AttachResult {
  bot: BotId
  username: string
  persist: boolean
  /** The bot's state as of the attach. */
  snapshot: Snapshot
}

/** The options every bot call takes last. */
export interface CallOptions {
  /** Aborting cancels the call (`cancel`, or `cancelCall` for `createBot` / `signIn`); it then rejects with `signal.reason` once the host has stopped it. */
  signal?: AbortSignal
}

/** A Microsoft device code for a person to enter at `verificationUri` (`message` says so in words). */
export type MsaCodePrompt = Omit<MsaCode, 'call'>

/** `createBot` / `signIn` options of a call: where its sign-in prompt goes. */
export interface HostCallOptions extends CallOptions {
  /** `auth: 'microsoft'`: shows a device code to a person when the account is not signed in (default `logMsaCode`). */
  onMsaCode?: (prompt: MsaCodePrompt) => void
}

/** Host calls with no bot to name, cancelled by call id. */
const HOST_CANCELLABLE: ReadonlySet<string> = new Set(['createBot', 'signIn'])

/** Where a bot's events go. */
export interface BotSink {
  /** Called once per event record of the bot, in stream order. */
  event(event: Event): void
}

interface Pending {
  method: MethodName | 'attach'
  resolve(value: unknown): void
  reject(error: unknown): void
  signal?: AbortSignal
  onMsaCode?: (prompt: MsaCodePrompt) => void
}

/**
 * Calls and records over one transport: call ids, settling each call from its `done` record, routing events to
 * bots, and keep-alive (contract: the package README).
 */
export class Session {
  readonly #transport: Transport
  #nextCall = 1
  readonly #pending = new Map<number, Pending>()
  readonly #bots = new Map<BotId, BotSink>()
  readonly #live = new Set<BotId>()
  readonly #queue: Out[] = []
  #paused = false
  /** The transport has delivered its last records; `#ended` once they are dispatched too. */
  #streamEnded = false
  #ended = false
  #keepAlive = false

  constructor(transport: Transport) {
    this.#transport = transport
    transport.open({ records: (records) => this.#receive(records), closed: () => this.#closed() })
  }

  /** The transport's sync reads, if it has them. */
  get reads(): SyncReads | undefined {
    return this.#transport.reads
  }

  /** Sends one call; settles from its `done` record, or rejects at once for a refusal or an aborted `signal`. */
  call<M extends MethodName>(method: M, params: Params<M>, options: HostCallOptions = {}): Promise<Result<M>> {
    const { signal, onMsaCode } = options
    if (signal?.aborted) return Promise.reject(signal.reason)
    if (this.#ended) return Promise.reject(new AcaciaError('CLOSED', 'the host has shut down'))
    const call = this.#nextCall++
    const promise = new Promise<Result<M>>((resolve, reject) => {
      this.#pending.set(call, { method, resolve: resolve as (v: unknown) => void, reject, signal, onMsaCode })
    })
    try {
      this.#transport.send({ call, method, params } as Command)
    } catch (e) {
      this.#pending.delete(call)
      return Promise.reject(e)
    }
    this.#updateKeepAlive()
    const cancel =
      'bot' in params ? () => this.call('cancel', { bot: params.bot, call }).catch(() => {})
      : HOST_CANCELLABLE.has(method) ? () => this.call('cancelCall', { call }).catch(() => {})
      : undefined
    if (signal && cancel) {
      signal.addEventListener('abort', cancel, { once: true })
      const cleanup = () => signal.removeEventListener('abort', cancel)
      promise.then(cleanup, cleanup)
    }
    return promise
  }

  /** `attach` on an acacia-daemon transport; settles like `createBot` (the new bot's records wait a macrotask). */
  attachBot(params: AttachParams): Promise<AttachResult> {
    return this.call('attach' as MethodName, params as never) as Promise<unknown> as Promise<AttachResult>
  }

  /** Routes `bot`'s events to `sink` (a `Bot` attaches itself when built). */
  attach(bot: BotId, sink: BotSink): void {
    this.#bots.set(bot, sink)
  }

  /** Shuts the transport's host down (every bot closes); resolves once the stream has ended. */
  close(): Promise<void> {
    return this.#transport.close()
  }

  #receive(records: Out[]): void {
    this.#queue.push(...records)
    this.#drain()
  }

  #drain(): void {
    if (this.#paused) return
    let thrown: { error: unknown } | undefined
    while (this.#queue.length > 0) {
      const record = this.#queue.shift()!
      let yieldAfter = false
      try {
        yieldAfter = this.#dispatch(record)
      } catch (error) {
        thrown ??= { error }
      }
      if (yieldAfter) {
        // A new Bot's `createBot` resolved: let the caller attach listeners before its next records.
        this.#paused = true
        setImmediate(() => {
          this.#paused = false
          this.#drain()
        })
        break
      }
    }
    if (!this.#paused && this.#streamEnded && !this.#ended) this.#finish()
    this.#updateKeepAlive()
    // A throwing listener must not cost the rest of the batch; it still surfaces as an uncaught exception.
    if (thrown) process.nextTick(() => { throw thrown.error })
  }

  /** True when the stream should pause for the caller (a bot was created). */
  #dispatch(record: Out): boolean {
    switch (record.type) {
      case 'event':
        if (record.event.type === 'end') this.#live.delete(record.bot)
        this.#bots.get(record.bot)?.event(record.event)
        if (record.event.type === 'end') this.#bots.delete(record.bot)
        return false
      case 'done': {
        const pending = this.#pending.get(record.call)
        if (!pending) return false
        this.#pending.delete(record.call)
        const { result } = record
        if ('ok' in result) {
          pending.resolve(result.ok)
          if (pending.method !== 'createBot' && pending.method !== 'attach') return false
          this.#live.add((result.ok as Result<'createBot'>).bot)
          return true
        }
        const aborted = result.err.code === 'CANCELLED' && pending.signal?.aborted
        pending.reject(aborted ? pending.signal!.reason : errorFrom(result.err, pending.method as MethodName))
        return false
      }
      case 'lagged':
        for (const bot of this.#bots.values()) bot.event({ type: 'lagged', dropped: record.dropped })
        return false
      case 'msaCode': {
        const { type: _, call, ...prompt } = record
        this.#pending.get(call)?.onMsaCode?.(prompt)
        return false
      }
      default:
        return false
    }
  }

  #closed(): void {
    this.#streamEnded = true
    this.#drain()
  }

  #finish(): void {
    this.#ended = true
    for (const pending of this.#pending.values()) pending.reject(new AcaciaError('CLOSED', 'the host has shut down'))
    this.#pending.clear()
    this.#live.clear()
  }

  /** Holds the event loop while a script is owed something: live bots, unanswered calls (a closing transport holds it). */
  #updateKeepAlive(): void {
    const want = !this.#ended && (this.#pending.size > 0 || this.#live.size > 0)
    if (want === this.#keepAlive) return
    this.#keepAlive = want
    this.#transport.setKeepAlive?.(want)
  }
}
