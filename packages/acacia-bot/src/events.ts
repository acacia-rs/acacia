import type { Entity } from './generated/types/Entity.js'
import type { Event } from './generated/types/Event.js'
import type { EventGroup } from './generated/types/EventGroup.js'

/** A raw event's name: an {@link Event}'s `type`. */
export type EventName = Event['type']

/** The event of type `K`, as the bot emits it (the record's `event`, `type` included). */
export type EventOf<K extends EventName> = Extract<Event, { type: K }>

/** The mineflayer-named events derived from the ones above (README, Events). */
export interface DerivedEvents {
  /** The server kicked the bot: `end`'s message; `end` follows. */
  kicked: [reason: string, end: EventOf<'end'>]
  /** A whisper (`/tell`) to the bot, from a sender the packet names. */
  whisper: [username: string, message: string, line: EventOf<'chat'>]
  /** Per entity of an `entityMoves` batch that is in `bot.entities`: its live object (README, Events). */
  entityMoved: [entity: Entity]
}

/** `chat:<name>`: a chat line matched the `addChatPattern` named `name`; its capture groups. */
export type ChatPatternEvents = Record<`chat:${string}`, [groups: string[], line: EventOf<'chat'>]>

/** `bot.on(name, (payload) => ...)`: one payload per event. `error` carries an `Error`, as Node expects. */
export type BotEvents = { [K in Exclude<EventName, 'error'>]: [EventOf<K>] } & { error: [Error] } & DerivedEvents & ChatPatternEvents

/** The group a listener for `name` needs enabled; events not listed (spawn, end, error, lagged, ...) always come. */
const GROUP_OF: Partial<Record<string, EventGroup>> = {
  chat: 'chat',
  whisper: 'chat',
  form: 'forms',
  health: 'health',
  death: 'health',
  respawn: 'health',
  playerJoined: 'players',
  playerLeft: 'players',
  windowOpen: 'windows',
  windowUpdate: 'windows',
  windowClose: 'windows',
  signEditor: 'windows',
  title: 'titles',
  sleep: 'sleep',
  wake: 'sleep',
  physicsTick: 'ticks',
  entitySpawn: 'entities',
  entityGone: 'entities',
  entityMoves: 'entityMoves',
  entityMoved: 'entityMoves',
}

/**
 * Which event groups a bot reports: the forced ones plus every group with a listener. Changes are sent once per
 * microtask, and only when the set changed.
 */
export class Subscriptions {
  readonly #forced: ReadonlySet<EventGroup>
  readonly #listeners = new Map<EventGroup, number>()
  readonly #send: (groups: EventGroup[]) => void
  #sent: string
  #scheduled = false

  constructor(forced: readonly EventGroup[], send: (groups: EventGroup[]) => void) {
    this.#forced = new Set(forced)
    this.#send = send
    this.#sent = key([...this.#forced])
  }

  added(name: string | symbol): void {
    this.#change(name, 1)
  }

  removed(name: string | symbol): void {
    this.#change(name, -1)
  }

  /** Sends a change still waiting for its microtask now. */
  flush(): void {
    if (!this.#scheduled) return
    this.#scheduled = false
    const groups = [...new Set([...this.#forced, ...[...this.#listeners].filter(([, n]) => n > 0).map(([g]) => g)])]
    const next = key(groups)
    if (next === this.#sent) return
    this.#sent = next
    this.#send(groups.sort())
  }

  #change(name: string | symbol, by: number): void {
    const group = typeof name !== 'string' ? undefined : name.startsWith('chat:') ? 'chat' : GROUP_OF[name]
    if (!group) return
    this.#listeners.set(group, Math.max(0, (this.#listeners.get(group) ?? 0) + by))
    if (this.#scheduled) return
    this.#scheduled = true
    queueMicrotask(() => this.flush())
  }
}

const key = (groups: EventGroup[]) => [...groups].sort().join(',')
