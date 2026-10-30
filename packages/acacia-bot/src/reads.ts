import { BotCore } from './core.js'
import { AcaciaError } from './errors.js'
import type { Block } from './generated/types/Block.js'
import type { BlockPos } from './generated/types/BlockPos.js'
import type { ColdSnapshot } from './generated/types/ColdSnapshot.js'
import type { Entity } from './generated/types/Entity.js'
import type { EntityList } from './generated/types/EntityList.js'
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

const EMPTY_ENTITIES: EntityList = { version: 0, entities: [] }

/** A bot's reads without a call (contract: the package README). Each is null where the transport has none. */
export class BotReads extends BotCore {
  readonly #byId = new Derived((list: EntityList) => Object.freeze(Object.fromEntries(list.entities.map((e) => [e.id, e]))))
  readonly #players = new Derived(players)
  readonly #inventory = new Derived(inventory)

  /** The per-tick state; null before the first tick. */
  get hot(): HotSnapshot | null {
    return this.session.reads?.hot(this.id) ?? null
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

  /** Inventory, effects and the player list; one frozen object per change. */
  get cold(): ColdSnapshot | null {
    return this.session.reads?.cold(this.id) ?? null
  }

  /** Tracked entities by id (empty unless created with `entities`); one frozen object per change. */
  get entities(): Readonly<Record<string, Entity>> {
    return this.#byId.get(this.#entityList())
  }

  /** Other players by name: the player list joined with player entities in view, plus players seen only in view. */
  get players(): Readonly<Record<string, Player>> {
    return this.#players.get(this.cold, this.#entityList())
  }

  /** The tracked entity closest to the bot's feet that passes `filter`; null if none (or before the first tick). */
  nearestEntity(filter: (entity: Entity) => boolean = () => true): Entity | null {
    const me = this.hot?.position
    if (!me) return null
    let best: Entity | null = null
    let bestDistance = Infinity
    for (const e of this.#entityList().entities) {
      const d = (e.position.x - me.x) ** 2 + (e.position.y - me.y) ** 2 + (e.position.z - me.z) ** 2
      if (d < bestDistance && filter(e)) [best, bestDistance] = [e, d]
    }
    return best
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

  /** The nearest loaded blocks with any of `matching`'s names, nearest first (contract: bot.findBlocks). */
  findBlocks(query: FindBlocksQuery): BlockPos[] {
    const reads = this.session.reads
    if (!reads) throw new AcaciaError('NOT_POSSIBLE', 'this transport has no sync reads')
    return reads.findBlocks(this.id, query)
  }

  #entityList(): EntityList {
    return this.session.reads?.entities(this.id) ?? EMPTY_ENTITIES
  }
}

function inventory(cold: ColdSnapshot | null, table: ItemTable | null): readonly Item[] {
  const names = new Map(table?.items.map((i) => [i.id, i.name]))
  const items = (cold?.inventory ?? []).map((s) => Object.freeze({ ...s, name: names.get(s.networkId) ?? null }))
  return Object.freeze(items)
}

function players(cold: ColdSnapshot | null, list: EntityList): Readonly<Record<string, Player>> {
  const byUnique = new Map(list.entities.filter((e) => e.username !== undefined).map((e) => [e.uniqueId, e]))
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
