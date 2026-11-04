import { EventEmitter, on, once } from 'node:events'

import { ChatPatterns, type ChatPatternOptions } from './chat-patterns.js'
import { AcaciaError } from './errors.js'
import { Subscriptions, type BotEvents } from './events.js'
import type { MethodName, Params, Result } from './generated/methods.js'
import type { BotId } from './generated/types/BotId.js'
import type { Event } from './generated/types/Event.js'
import type { EventGroup } from './generated/types/EventGroup.js'
import type { Snapshot } from './generated/types/Snapshot.js'
import type { BotSink, CallOptions, Session } from './session.js'

type BotParams<M extends MethodName> = Omit<Params<M>, 'bot'>
/** Methods without a result (`null` on the wire) resolve to nothing. */
type Voided<T> = T extends null ? void : T

/** A bot's events, calls and lifecycle; `Bot` adds the reads and verbs. */
export class BotCore extends EventEmitter<BotEvents> implements BotSink {
  readonly id: BotId
  readonly username: string
  protected readonly session: Session
  #ended = false
  readonly #subscriptions: Subscriptions
  #groupsSent: Promise<void> = Promise.resolve()
  readonly #chatPatterns = new ChatPatterns()

  /** @internal Use `createBot`. */
  constructor(session: Session, id: BotId, username: string, forcedEvents: readonly EventGroup[]) {
    super({ captureRejections: true })
    this.id = id
    this.username = username
    this.session = session
    this.#subscriptions = new Subscriptions(forcedEvents, (events) => {
      if (!this.#ended) this.#groupsSent = this.call('bot.setEvents', { events }).catch(() => {})
    })
    const emitter = this as EventEmitter
    emitter.on('newListener', (name: string | symbol) => this.#subscriptions.added(name))
    emitter.on('removeListener', (name: string | symbol) => this.#subscriptions.removed(name))
    session.attach(id, this)
  }

  /** Resolves once the bot reports the event groups of the listeners attached so far. */
  protected listenersApplied(): Promise<void> {
    this.#subscriptions.flush()
    return this.#groupsSent
  }

  /** @internal Called by the session for each of this bot's events. */
  event(event: Event): void {
    switch (event.type) {
      case 'error': {
        const error = new AcaciaError('INTERNAL', event.message)
        if (this.listenerCount('error') > 0) this.emit('error', error)
        else process.emitWarning(error)
        return
      }
      case 'end':
        this.#ended = true
        if (event.reason === 'kicked') this.emit('kicked', event.message, event)
        break
      case 'chat':
        this.emit('chat', event)
        if (event.kind === 'whisper' && event.sender) this.emit('whisper', event.sender, event.plain, event)
        for (const [name, groups] of this.#chatPatterns.match(event.plain)) this.emit(`chat:${name}`, groups, event)
        return
    }
    this.emit(event.type, event as never)
  }

  /**
   * Emits `chat:<name>` with the capture groups whenever a chat line's `plain` text matches `regex` (mineflayer's
   * `addChatPattern`); adding a name again replaces its pattern.
   */
  addChatPattern(name: string, regex: RegExp, options?: ChatPatternOptions): void {
    this.#chatPatterns.add(name, regex, options)
  }

  /** Stops matching the pattern `name`; false when there was none. */
  removeChatPattern(name: string): boolean {
    return this.#chatPatterns.remove(name)
  }

  /** Whether the bot has ended (an `end` event came). */
  get ended(): boolean {
    return this.#ended
  }

  chat(message: string, options?: CallOptions): Promise<void> {
    return this.call('bot.chat', { message }, options)
  }

  /** Runs a slash command (the leading `/` is optional). */
  command(command: string, options?: CallOptions): Promise<void> {
    return this.call('bot.command', { command }, options)
  }

  /** The bot's whole state, built now (the getters read published state without a call). */
  snapshot(options?: CallOptions): Promise<Snapshot> {
    return this.call('bot.snapshot', {}, options)
  }

  /** Sets the reported event groups outright; listeners change them again as they come and go. */
  setEvents(events: EventGroup[], options?: CallOptions): Promise<void> {
    return this.call('bot.setEvents', { events }, options)
  }

  /** Disconnects; resolves after the `end` event (at once if the bot has ended). */
  close(): Promise<void> {
    return this.#ended ? Promise.resolve() : this.call('bot.close', {})
  }

  /** `name`'s payloads as an async iterator; it ends when the bot does (or throws on `signal`). */
  async *events<K extends keyof BotEvents>(name: K, options: { signal?: AbortSignal } = {}): AsyncGenerator<BotEvents[K][0]> {
    const emitter = this as EventEmitter
    if (name === 'end') {
      if (this.#ended) return
      const [payload] = await once(emitter, 'end', options)
      yield payload
      return
    }
    for await (const [payload] of on(emitter, name, { ...options, close: ['end'] })) yield payload
  }

  /** Calls any table method by name with this bot's id filled in, for generic callers (`@acacia-rs/mcp`). */
  request<M extends MethodName>(method: M, params: BotParams<M>, options?: CallOptions): Promise<Voided<Result<M>>> {
    return this.call(method, params, options)
  }

  protected call<M extends MethodName>(method: M, params: BotParams<M>, options?: CallOptions): Promise<Voided<Result<M>>> {
    return this.session.call(method, { ...params, bot: this.id } as Params<M>, options) as Promise<Voided<Result<M>>>
  }
}
