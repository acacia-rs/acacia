import { EventEmitter, on, once } from 'node:events'

import { AcaciaError } from './errors.js'
import { Subscriptions, type BotEvents } from './events.js'
import type { MethodName, Params, Result } from './generated/methods.js'
import type { Block } from './generated/types/Block.js'
import type { BlockPos } from './generated/types/BlockPos.js'
import type { BotId } from './generated/types/BotId.js'
import type { Event } from './generated/types/Event.js'
import type { EventGroup } from './generated/types/EventGroup.js'
import type { FormAnswer } from './generated/types/FormAnswer.js'
import type { Goal } from './generated/types/Goal.js'
import type { HotSnapshot } from './generated/types/HotSnapshot.js'
import type { Snapshot } from './generated/types/Snapshot.js'
import type { Vec3 } from './generated/types/Vec3.js'
import type { BotSink, CallOptions, Session } from './session.js'

type BotParams<M extends MethodName> = Omit<Params<M>, 'bot'>
/** Methods without a result (`null` on the wire) resolve to nothing. */
type Voided<T> = T extends null ? void : T

/** The bot's body as of its last tick. */
export interface EntityView {
  position: Vec3
  velocity: Vec3
  yaw: number
  pitch: number
  onGround: boolean
}

/**
 * One connected bot (contract: the package README). Queued actions (`goto`, `dig`, `answerForm`, `waitForTicks`)
 * run one at a time in call order; `chat`, `command` and the getters do not wait for them.
 */
export class Bot extends EventEmitter<BotEvents> implements BotSink {
  readonly id: BotId
  readonly username: string
  readonly #session: Session
  #ended = false

  /** @internal Use `createBot`. */
  constructor(session: Session, id: BotId, username: string, forcedEvents: readonly EventGroup[]) {
    super({ captureRejections: true })
    this.id = id
    this.username = username
    this.#session = session
    const subscriptions = new Subscriptions(forcedEvents, (events) => {
      if (!this.#ended) this.#call('bot.setEvents', { events }).catch(() => {})
    })
    const emitter = this as EventEmitter
    emitter.on('newListener', (name: string | symbol) => subscriptions.added(name))
    emitter.on('removeListener', (name: string | symbol) => subscriptions.removed(name))
    session.attach(id, this)
  }

  /** @internal Called by the session for each of this bot's events. */
  event(event: Event): void {
    if (event.type === 'end') this.#ended = true
    if (event.type !== 'error') {
      this.emit(event.type, event as never)
      return
    }
    const error = new AcaciaError('INTERNAL', event.message)
    if (this.listenerCount('error') > 0) this.emit('error', error)
    else process.emitWarning(error)
  }

  /** Whether the bot has ended (an `end` event came). */
  get ended(): boolean {
    return this.#ended
  }

  chat(message: string, options?: CallOptions): Promise<void> {
    return this.#call('bot.chat', { message }, options)
  }

  /** Runs a slash command (the leading `/` is optional). */
  command(command: string, options?: CallOptions): Promise<void> {
    return this.#call('bot.command', { command }, options)
  }

  /** Walks to `goal` (see `goals`); needs `physics`. Fails with `NOT_POSSIBLE` when there is no path. */
  goto(goal: Goal, options?: CallOptions): Promise<void> {
    return this.#call('bot.goto', { goal }, options)
  }

  /** Breaks the block at a cell, as a player would. */
  dig(pos: BlockPos, options?: CallOptions): Promise<void> {
    return this.#call('bot.dig', { x: pos.x, y: pos.y, z: pos.z }, options)
  }

  answerForm(form: number | { id: number }, answer: FormAnswer, options?: CallOptions): Promise<void> {
    return this.#call('bot.answerForm', { form: typeof form === 'number' ? form : form.id, answer }, options)
  }

  /** Lets `ticks` ticks (50 ms each) pass while the bot keeps playing. */
  waitForTicks(ticks: number, options?: CallOptions): Promise<void> {
    return this.#call('bot.waitForTicks', { ticks }, options)
  }

  /** The bot's whole state, built now (the getters read the per-tick part without a call). */
  snapshot(options?: CallOptions): Promise<Snapshot> {
    return this.#call('bot.snapshot', {}, options)
  }

  /** Sets the reported event groups outright; listeners change them again as they come and go. */
  setEvents(events: EventGroup[], options?: CallOptions): Promise<void> {
    return this.#call('bot.setEvents', { events }, options)
  }

  /** Disconnects; resolves after the `end` event (at once if the bot has ended). */
  close(): Promise<void> {
    return this.#ended ? Promise.resolve() : this.#call('bot.close', {})
  }

  /** The per-tick state, read without a call; null before the first tick or where the transport has no sync reads. */
  get hot(): HotSnapshot | null {
    return this.#session.reads?.hot(this.id) ?? null
  }

  get entity(): EntityView | null {
    const hot = this.hot
    return hot && { position: hot.position, velocity: hot.velocity, yaw: hot.yaw, pitch: hot.pitch, onGround: hot.onGround }
  }

  get health(): number | null {
    return this.hot?.health ?? null
  }

  get food(): number | null {
    return this.hot?.food ?? null
  }

  /** The block at a cell, read without a call; null for an unloaded chunk or a bot that tracks no terrain. */
  blockAt(pos: BlockPos): Block | null {
    return this.#session.reads?.blockAt(this.id, Math.floor(pos.x), Math.floor(pos.y), Math.floor(pos.z)) ?? null
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

  #call<M extends MethodName>(method: M, params: BotParams<M>, options?: CallOptions): Promise<Voided<Result<M>>> {
    return this.#session.call(method, { ...params, bot: this.id } as Params<M>, options) as Promise<Voided<Result<M>>>
  }
}
