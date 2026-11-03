import type { NativeHost } from '../native/index.js'
import type { Entity } from './generated/types/Entity.js'
import type { EntityList } from './generated/types/EntityList.js'
import type { Vec3 } from './generated/types/Vec3.js'

/** Numbers per entity, in acacia-host's `EntityFrame` order: feet xyz, velocity xyz, yaw, pitch, health. */
const STRIDE = 9

/** One membership generation's numbers, which the addon overwrites in place as its entities move. */
interface Frame {
  readonly numbers: Float64Array
}

type Member = [id: string, uniqueId: string, kind: string, username: string | null]

const NO_NUMBERS = new Float64Array(0)
const SLOT = Symbol('slot')
type Live = Entity & { [SLOT]: { frame: Frame; base: number } }

const at = (e: Live, k: number): number => {
  const { frame, base } = e[SLOT]
  return frame.numbers[base + k]
}
const vec = (e: Live, k: number): Vec3 => ({ x: at(e, k), y: at(e, k + 1), z: at(e, k + 2) })

// Own enumerable accessors, so spreads and JSON.stringify see the fields.
const LIVE: PropertyDescriptorMap = {
  position: { enumerable: true, get(this: Live) { return vec(this, 0) } },
  velocity: { enumerable: true, get(this: Live) { return vec(this, 3) } },
  yaw: { enumerable: true, get(this: Live) { return at(this, 6) } },
  pitch: { enumerable: true, get(this: Live) { return at(this, 7) } },
  health: { enumerable: true, get(this: Live) { const h = at(this, 8); return Number.isNaN(h) ? undefined : h } },
}

function live([id, uniqueId, kind, username]: Member, frame: Frame, base: number): Live {
  const e = username === null ? { id, uniqueId, kind } : { id, uniqueId, kind, username }
  Object.defineProperty(e, SLOT, { value: { frame, base } })
  return Object.freeze(Object.defineProperties(e, LIVE)) as Live
}

/** `previous`'s entity for this member, moved to read slot `base` of `frame`; undefined if it is not the same entity. */
function carried(previous: Live | undefined, [, uniqueId, kind, username]: Member, frame: Frame, base: number): Live | undefined {
  if (!previous || previous.uniqueId !== uniqueId || previous.kind !== kind || (previous.username ?? null) !== username) return
  Object.assign(previous[SLOT], { frame, base })
  return previous
}

interface Last {
  version: number
  members: number
  list: EntityList
  frame: Frame
  byId: Map<string, Live>
}

/**
 * Per bot, the entity list as live entities (contract: the package README, Sync reads): one list per membership,
 * whose entities read the numbers of the newest frame fetched. A move costs one copy into its array, no JSON; a
 * membership change parses only ids, kinds and names, and entities that stayed keep their objects.
 */
export class EntityFrames {
  readonly #host: NativeHost
  readonly #last = new Map<string, Last>()

  constructor(host: NativeHost) {
    this.#host = host
  }

  read(bot: string): EntityList | null {
    const version = this.#host.entitiesVersion(bot)
    const cached = this.#last.get(bot)
    if (version === null) {
      this.#last.delete(bot)
      return null
    }
    if (cached?.version === version) return cached.list
    const got = this.#host.entityFrame(bot, cached?.members, cached?.frame.numbers ?? NO_NUMBERS)
    if (got === null) return null
    if (cached && got.numbers == null) {
      cached.version = got.version
      return cached.list
    }
    const frame = { numbers: got.numbers! }
    const byId = new Map<string, Live>()
    const entities = (JSON.parse(got.members!) as Member[]).map((m, i) => {
      const e = carried(cached?.byId.get(m[0]), m, frame, i * STRIDE) ?? live(m, frame, i * STRIDE)
      byId.set(m[0], e)
      return e
    })
    Object.freeze(entities)
    const list: EntityList = Object.freeze({ version: got.membersVersion, entities })
    this.#last.set(bot, { version: got.version, members: got.membersVersion, list, frame, byId })
    return list
  }

  clear(): void {
    this.#last.clear()
  }
}
