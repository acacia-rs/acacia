import type { Entity } from './generated/types/Entity.js'
import type { Vec3 } from './generated/types/Vec3.js'

/** Numbers per slot: feet xyz, velocity xyz, yaw, pitch, health (NaN when unknown); acacia-host's ENTITY_NUMBERS. */
export const STRIDE = 9
const MIN_SLOTS = 16

/** `[slot, x, y, z, vx, vy, vz, yaw, pitch, health | null]`. */
export type NumbersRow = [slot: number, ...numbers: number[]]
/** `[slot, id, uniqueId, kind, username | null]`. */
export type AddedRow = [slot: number, id: string, uniqueId: string, kind: string, username: string | null]

/** Changes to a bot's entity list since the table's `version` (format: acacia-daemon's crate docs, Entity deltas). */
export interface EntityDelta {
  version: number
  full?: boolean
  removed?: NumbersRow[]
  added?: AddedRow[]
  numbers?: NumbersRow[]
}

interface Store {
  numbers: Float64Array
}
/** Where an entity reads its numbers: the table's array while listed, its own copy once it left. */
interface Place {
  store: Store
  base: number
  index: number
}

const PLACE = Symbol('place')
type Live = Entity & { readonly [PLACE]: Place }

const at = (e: Live, k: number): number => {
  const { store, base } = e[PLACE]
  return store.numbers[base + k]
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

function live([, id, uniqueId, kind, username]: AddedRow, place: Place): Live {
  const e = username === null ? { id, uniqueId, kind } : { id, uniqueId, kind, username }
  Object.defineProperty(e, PLACE, { value: place })
  return Object.freeze(Object.defineProperties(e, LIVE)) as Live
}

const same = (e: Live, [, id, uniqueId, kind, username]: AddedRow): boolean =>
  e.id === id && e.uniqueId === uniqueId && e.kind === kind && (e.username ?? null) === username

function own(numbers: ArrayLike<number | null>, from: number): Store {
  const copy = new Float64Array(STRIDE)
  for (let k = 0; k < STRIDE; k++) copy[k] = numbers[from + k] ?? NaN
  return { numbers: copy }
}

/**
 * One bot's entities, kept up to date by applying deltas (contract: the package README, Sync reads): `list` and
 * `byId` change in place, entities keep their objects while listed, and their numbers live in `numbers` by slot.
 */
export class EntityTable implements Store {
  numbers: Float64Array = new Float64Array(0)
  /** The delta version applied last; null before the first. */
  version: number | null = null
  /** Moves whenever an entity came or went. */
  members = 0
  readonly list: Entity[] = []
  readonly byId: Record<string, Entity> = {}
  readonly #bySlot: (Live | undefined)[] = []

  /** `numbers`, when given, replaces the array after removals (an in-process reader whose addon wrote the rows). */
  apply(delta: EntityDelta, numbers?: Float64Array): void {
    if (delta.full) this.#dropAllBut(delta.added ?? [])
    for (const row of delta.removed ?? []) {
      const e = this.#bySlot[row[0]]
      if (e) this.#drop(e, own(row, 1))
    }
    if (numbers) this.numbers = numbers
    for (const row of delta.added ?? []) {
      const kept = this.#bySlot[row[0]]
      if (!kept || !same(kept, row)) this.#add(row)
    }
    for (const row of delta.numbers ?? []) this.#write(row)
    this.version = delta.version
  }

  /** The entity closest to (x, y, z) that passes `filter`, reading numbers without building positions. */
  nearest(x: number, y: number, z: number, filter: (entity: Entity) => boolean): Entity | null {
    let best: Entity | null = null
    let bestDistance = Infinity
    for (const e of this.list as Live[]) {
      const { store, base } = e[PLACE]
      const n = store.numbers
      const d = (n[base] - x) ** 2 + (n[base + 1] - y) ** 2 + (n[base + 2] - z) ** 2
      if (d < bestDistance && filter(e)) [best, bestDistance] = [e, d]
    }
    return best
  }

  #dropAllBut(added: AddedRow[]): void {
    const kept = new Set(added.map((row) => this.#bySlot[row[0]]).filter((e, i) => e && same(e, added[i])))
    for (const e of [...this.list] as Live[]) if (!kept.has(e)) this.#drop(e, own(this.numbers, e[PLACE].base))
  }

  #drop(e: Live, last: Store): void {
    const place = e[PLACE]
    const moved = this.list.pop() as Live
    if (moved !== e) {
      this.list[place.index] = moved
      moved[PLACE].index = place.index
    }
    delete this.byId[e.id]
    this.#bySlot[place.base / STRIDE] = undefined
    Object.assign(place, { store: last, base: 0 })
    this.members++
  }

  #add(row: AddedRow): void {
    const e = live(row, { store: this, base: row[0] * STRIDE, index: this.list.length })
    this.list.push(e)
    this.byId[e.id] = e
    this.#bySlot[row[0]] = e
    this.members++
  }

  #write(row: NumbersRow): void {
    const base = row[0] * STRIDE
    if (base + STRIDE > this.numbers.length) {
      const grown = new Float64Array(2 ** Math.ceil(Math.log2(Math.max(MIN_SLOTS, row[0] + 1))) * STRIDE)
      grown.set(this.numbers)
      this.numbers = grown
    }
    for (let k = 0; k < STRIDE; k++) this.numbers[base + k] = row[k + 1] ?? NaN
  }
}
