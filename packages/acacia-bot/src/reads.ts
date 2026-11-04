import { BotCore } from './core.js'
import { AcaciaError, ActionError } from './errors.js'
import type { Block } from './generated/types/Block.js'
import type { BlockPos } from './generated/types/BlockPos.js'
import type { ColdSnapshot } from './generated/types/ColdSnapshot.js'
import type { CursorHit } from './generated/types/CursorHit.js'
import type { Entity } from './generated/types/Entity.js'
import type { Event } from './generated/types/Event.js'
import type { EntityTable } from './entity-table.js'
import type { HotSnapshot } from './generated/types/HotSnapshot.js'
import type { ItemTable } from './generated/types/ItemTable.js'
import type { PlayerInfo } from './generated/types/PlayerInfo.js'
import type { Stack } from './generated/types/Stack.js'
import type { Vec3 } from './generated/types/Vec3.js'
import type { FindBlocksQuery } from './transport.js'

/** The bot's body as of its last tick. */
export interface EntityView {
  position: Vec3
  velocity: Vec3
  yaw: number
  pitch: number
  onGround: boolean
}

/** A stack with its item name (null until the server's item table arrived, or for an unknown id). */
export interface Item extends Stack {
  name: string | null
}

/** Another player: its player-list entry (if any) and its entity while in view. */
export interface Player {
  username: string
  /** Absent for a player seen only as an entity. */
  info: PlayerInfo | null
  entity: Entity | null
}

/** Derives a value from reads once per identity of its sources (the transport caches those per version). */
class Derived<S extends readonly unknown[], T> {
  #sources: S | undefined
  #value: T | undefined
  constructor(readonly derive: (...sources: S) => T) {}
  get(...sources: S): T {
    if (!this.#sources || sources.some((s, i) => s !== this.#sources![i])) {
      this.#sources = sources
      this.#value = this.derive(...sources)
    }
    return this.#value!
  }
}

const NO_ENTITIES: Readonly<Record<string, Entity>> = Object.freeze({})

/** A bot's reads without a call (contract: the package README). Each is null where the transport has none. */
export class BotReads extends BotCore {
  readonly #players = new Derived((cold: ColdSnapshot | null, table: EntityTable | null, _members: number) => players(cold, table))
  readonly #inventory = new Derived(inventory)

  /** @internal Adds the per-entity `entityMoved`, built only while it has a listener. */
  override event(event: Event): void {
    super.event(event)
    if (event.type !== 'entityMoves' || this.listenerCount('entityMoved') === 0) return
    const byId = this.entities
    for (const row of event.rows) {
      const entity = byId[row[0]]
      if (entity) this.emit('entityMoved', entity)
    }
  }

  /** The per-tick state; null before the first tick. */
  get hot(): HotSnapshot | null {
    return this.session.reads?.hot(this.id) ?? null
  }

  /**
   * Resolves once the bot has ticked (`bot.hot` set where the transport has reads) and reports the event groups of
   * the listeners attached so far (contract: the package README, Sync reads). Rejects `DISCONNECTED` if it ends first.
   */
  async ready(options: { signal?: AbortSignal } = {}): Promise<void> {
    const ticked = this.hot ? undefined : this.#firstTick(options.signal)
    await Promise.all([ticked, this.listenersApplied()])
  }

  #firstTick(signal?: AbortSignal): Promise<void> {
    if (this.ended) return Promise.reject(new ActionError('DISCONNECTED', 'the bot has ended'))
    if (signal?.aborted) return Promise.reject(signal.reason)
    return new Promise((resolve, reject) => {
      const settle = (then: () => void) => {
        this.off('physicsTick', tick)
        this.off('end', end)
        signal?.removeEventListener('abort', abort)
        then()
      }
      const tick = () => settle(resolve)
      const end = () => settle(() => reject(new ActionError('DISCONNECTED', 'the bot ended before its first tick')))
      const abort = () => settle(() => reject(signal!.reason))
      this.on('physicsTick', tick)
      this.on('end', end)
      signal?.addEventListener('abort', abort, { once: true })
    })
  }

  /** The bot's own body (mineflayer's `bot.entity`); null before the first tick. */
  get entity(): EntityView | null {
    const hot = this.hot
    return hot && { position: hot.position, velocity: hot.velocity, yaw: hot.yaw, pitch: hot.pitch, onGround: hot.onGround }
  }

  /** Health points, 0-20 for a player; null before the first tick. */
  get health(): number | null {
    return this.hot?.health ?? null
  }

  /** The hunger bar, 0-20; null before the first tick. */
  get food(): number | null {
    return this.hot?.food ?? null
  }

  /** Inventory, effects and the player list; one frozen object per change. */
  get cold(): ColdSnapshot | null {
    return this.session.reads?.cold(this.id) ?? null
  }

  /** Tracked entities by id (empty unless created with `entities`); one object per bot, updated in place. */
  get entities(): Readonly<Record<string, Entity>> {
    return this.#entityTable()?.byId ?? NO_ENTITIES
  }

  /** Other players by name: the player list joined with player entities in view, plus players seen only in view. */
  get players(): Readonly<Record<string, Player>> {
    const table = this.#entityTable()
    return this.#players.get(this.cold, table, table?.members ?? 0)
  }

  /** The tracked entity closest to the bot's feet that passes `filter`; null if none (or before the first tick). */
  nearestEntity(filter: (entity: Entity) => boolean = () => true): Entity | null {
    const me = this.hot?.position
    if (!me) return null
    return this.#entityTable()?.nearest(me.x, me.y, me.z, filter) ?? null
  }

  /**
   * Tracked entities within `maxDistance` blocks of `point` (default the bot's feet) that pass `filter`, nearest
   * first; empty before the first tick.
   */
  entitiesNear(maxDistance: number, options: { point?: Vec3; filter?: (entity: Entity) => boolean } = {}): Entity[] {
    const { point = this.hot?.position, filter = () => true } = options
    if (!point) return []
    return this.#entityTable()?.within(point.x, point.y, point.z, maxDistance, filter) ?? []
  }

  /** In a bed, as the server shows it. */
  get isSleeping(): boolean {
    return this.hot?.sleeping ?? false
  }

  /** The block the bot is looking at within `maxDistance` (default 6, at most 64) blocks of its eyes; null if none. */
  blockAtCursor(maxDistance?: number): CursorHit | null {
    const reads = this.session.reads
    if (!reads) throw new AcaciaError('NOT_POSSIBLE', 'this transport has no sync reads')
    return reads.blockAtCursor(this.id, maxDistance === undefined ? {} : { maxDistance })
  }

  /** The block at a cell; null for an unloaded chunk or a bot that tracks no terrain. */
  blockAt(pos: BlockPos): Block | null {
    return this.session.reads?.blockAt(this.id, Math.floor(pos.x), Math.floor(pos.y), Math.floor(pos.z)) ?? null
  }

  /** Non-empty stacks of the bot's own windows, named through the server's item table; one frozen array per change. */
  get inventory(): readonly Item[] {
    return this.#inventory.get(this.cold, this.session.reads?.itemTable(this.id) ?? null)
  }

  /** The stack in the selected hotbar slot; null for an empty hand. */
  get heldItem(): Item | null {
    const slot = this.hot?.heldSlot
    return this.inventory.find((i) => i.window === 0 && i.slot === slot) ?? null
  }

  /**
   * The nearest loaded blocks with any of `matching`'s names, nearest first (README, Inventory and blocks).
   * Throws `NOT_POSSIBLE` on a transport without sync reads (`WsTransport`: use `bot.request('bot.findBlocks', ...)`).
   *
   * @example
   * ```js
   * const ores = bot.findBlocks({ matching: ['iron_ore', 'deepslate_iron_ore'], maxDistance: 32, count: 10 })
   * ```
   */
  findBlocks(query: FindBlocksQuery): BlockPos[] {
    const reads = this.session.reads
    if (!reads) throw new AcaciaError('NOT_POSSIBLE', 'this transport has no sync reads')
    return reads.findBlocks(this.id, query)
  }

  #entityTable(): EntityTable | null {
    return this.session.reads?.entities(this.id) ?? null
  }
}

/** Item names by network id, per item table object (the transport hands back one per table). */
const itemNames = new WeakMap<ItemTable, ReadonlyMap<number, string>>()
const NO_NAMES: ReadonlyMap<number, string> = new Map()

function namesOf(table: ItemTable | null): ReadonlyMap<number, string> {
  if (!table) return NO_NAMES
  let names = itemNames.get(table)
  if (!names) itemNames.set(table, (names = new Map(table.items.map((i) => [i.id, i.name]))))
  return names
}

function inventory(cold: ColdSnapshot | null, table: ItemTable | null): readonly Item[] {
  const names = namesOf(table)
  const items = (cold?.inventory ?? []).map((s) => Object.freeze({ ...s, name: names.get(s.networkId) ?? null }))
  return Object.freeze(items)
}

function players(cold: ColdSnapshot | null, table: EntityTable | null): Readonly<Record<string, Player>> {
  const byUnique = new Map((table?.list ?? []).filter((e) => e.username !== undefined).map((e) => [e.uniqueId, e]))
  const out: Record<string, Player> = {}
  for (const info of cold?.players ?? []) {
    out[info.username] = Object.freeze({ username: info.username, info, entity: byUnique.get(info.uniqueId) ?? null })
    byUnique.delete(info.uniqueId)
  }
  for (const entity of byUnique.values()) {
    out[entity.username!] ??= Object.freeze({ username: entity.username!, info: null, entity })
  }
  return Object.freeze(out)
}
