import { AcaciaError } from './errors.js'
import type { ColdSnapshot } from './generated/types/ColdSnapshot.js'
import type { EntityList } from './generated/types/EntityList.js'
import type { HotSnapshot } from './generated/types/HotSnapshot.js'
import type { ItemTable } from './generated/types/ItemTable.js'
import type { SyncReads } from './transport.js'

/** acacia-daemon's `mirror` record: the parts of a bot's published state that changed. */
export interface MirrorRecord {
  type: 'mirror'
  bot: string
  hot?: HotSnapshot
  cold?: ColdSnapshot
  entities?: EntityList
  items?: ItemTable
}

type State = { hot: HotSnapshot | null; cold: ColdSnapshot | null; entities: EntityList | null; items: ItemTable | null }

const PARTS = ['hot', 'cold', 'entities', 'items'] as const

function noTerrain(what: string): never {
  throw new AcaciaError('NOT_POSSIBLE', `${what} has no sync read over WebSocket; await bot.request('bot.findBlocks', ...) instead`)
}

/** The bots' state as the daemon last pushed it, served as `SyncReads` (contract: the package README). */
export class WsMirror {
  readonly #bots = new Map<string, State>()

  update(record: MirrorRecord): void {
    let state = this.#bots.get(record.bot)
    if (!state) this.#bots.set(record.bot, (state = { hot: null, cold: null, entities: null, items: null }))
    for (const part of PARTS) {
      const value = record[part]
      if (value !== undefined) (state as Record<string, unknown>)[part] = Object.freeze(value)
    }
  }

  forget(bot: string): void {
    this.#bots.delete(bot)
  }

  clear(): void {
    this.#bots.clear()
  }

  readonly reads: SyncReads = {
    hot: (bot) => this.#bots.get(bot)?.hot ?? null,
    cold: (bot) => this.#bots.get(bot)?.cold ?? null,
    entities: (bot) => this.#bots.get(bot)?.entities ?? null,
    itemTable: (bot) => this.#bots.get(bot)?.items ?? null,
    blockAt: () => noTerrain('blockAt'),
    findBlocks: () => noTerrain('findBlocks'),
  }
}
