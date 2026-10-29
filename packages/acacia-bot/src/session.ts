import { AcaciaError, errorFrom } from './errors.js'
import type { Command, MethodName, Params, Result } from './generated/methods.js'
import type { BotId } from './generated/types/BotId.js'
import type { Event } from './generated/types/Event.js'
import type { Out } from './generated/types/Out.js'
import type { SyncReads, Transport } from './transport.js'

export interface CallOptions {
  /** Aborting cancels the call (`cancel`); it then rejects with `signal.reason` once the bot has stopped. */
  signal?: AbortSignal
}

/** Where a bot's events go. */
export interface BotSink {
  event(event: Event): void
}

interface Pending {
  method: MethodName
  resolve(value: unknown): void
  reject(error: unknown): void
  signal?: AbortSignal
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

  get reads(): SyncReads | undefined {
    return this.#transport.reads
  }

  call<M extends MethodName>(method: M, params: Params<M>, options: CallOptions = {}): Promise<Result<M>> {
    const { signal } = options
    if (signal?.aborted) return Promise.reject(signal.reason)
    if (this.#ended) return Promise.reject(new AcaciaError('CLOSED', 'the host has shut down'))
    const call = this.#nextCall++
    const promise = new Promise<Result<M>>((resolve, reject) => {
      this.#pending.set(call, { method, resolve: resolve as (v: unknown) => void, reject, signal })
    })
    try {
      this.#transport.send({ call, method, params } as Command)
    } catch (e) {
      this.#pending.delete(call)
      return Promise.reject(e)
    }
    this.#updateKeepAlive()
    if (signal && 'bot' in params) {
      const cancel = () => this.call('cancel', { bot: params.bot, call }).catch(() => {})
      signal.addEventListener('abort', cancel, { once: true })
      const cleanup = () => signal.removeEventListener('abort', cancel)
      promise.then(cleanup, cleanup)
    }
    return promise
  }

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
          if (pending.method !== 'createBot') return false
          this.#live.add((result.ok as Result<'createBot'>).bot)
          return true
        }
        const aborted = result.err.code === 'CANCELLED' && pending.signal?.aborted
        pending.reject(aborted ? pending.signal!.reason : errorFrom(result.err, pending.method))
        return false
      }
      case 'lagged':
        for (const bot of this.#bots.values()) bot.event({ type: 'lagged', dropped: record.dropped })
        return false
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
