import type { NativeHost } from '../native/index.js'
import { EntityTable, type EntityDelta } from './entity-table.js'

const NO_NUMBERS = new Float64Array(0)
const NO_MEMBERS: Pick<EntityDelta, 'removed' | 'added'> = {}

/**
 * Per bot, the entity list kept by deltas from the addon: an unchanged version costs one call, a move one call that
 * writes the moved entities' numbers into the table's array, and only entities that came or went cross as JSON.
 */
export class NativeEntities {
  readonly #host: NativeHost
  readonly #tables = new Map<string, EntityTable>()

  constructor(host: NativeHost) {
    this.#host = host
  }

  read(bot: string): EntityTable | null {
    const version = this.#host.entitiesVersion(bot)
    let table = this.#tables.get(bot)
    if (version === null) {
      this.#tables.delete(bot)
      return null
    }
    if (table?.version === version) return table
    const got = this.#host.entityDelta(bot, table?.version, table?.numbers ?? NO_NUMBERS)
    if (got === null) return null
    if (!table) this.#tables.set(bot, (table = new EntityTable()))
    const members = got.members === null || got.members === undefined ? NO_MEMBERS : JSON.parse(got.members)
    table.apply({ version: got.version, full: got.full, ...members }, got.numbers ?? undefined)
    return table
  }

  clear(): void {
    this.#tables.clear()
  }
}
