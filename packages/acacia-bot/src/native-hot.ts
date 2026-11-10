import type { NativeHost } from '../native/index.js'
import type { HotSnapshot } from './generated/types/HotSnapshot.js'

/** Slots `hotNumbers` fills, in the order of crates/acacia-node/src/hot.rs. */
const HOT_NUMBERS = 26

/** Per bot, `bot.hot` from the addon's numbers: no JSON per tick, and one object per tick however often it is read. */
export class NativeHot {
  readonly #host: NativeHost
  readonly #numbers = new Float64Array(HOT_NUMBERS)
  readonly #last = new Map<string, HotSnapshot>()

  constructor(host: NativeHost) {
    this.#host = host
  }

  read(bot: string): HotSnapshot | null {
    const n = this.#numbers
    if (!this.#host.hotNumbers(bot, n)) {
      this.#last.delete(bot)
      return null
    }
    const cached = this.#last.get(bot)
    if (cached?.tick === n[0]) return cached
    const value = Object.freeze(snapshot(n))
    this.#last.set(bot, value)
    return value
  }

  clear(): void {
    this.#last.clear()
  }
}

function snapshot(n: Float64Array): HotSnapshot {
  const hot: HotSnapshot = {
    tick: n[0]!,
    position: { x: n[1]!, y: n[2]!, z: n[3]! },
    velocity: { x: n[4]!, y: n[5]!, z: n[6]! },
    yaw: n[7]!,
    pitch: n[8]!,
    onGround: n[9] === 1,
    health: n[10]!,
    maxHealth: n[11]!,
    food: n[12]!,
    saturation: n[13]!,
    xpLevel: n[14]!,
    xpProgress: n[15]!,
    air: n[16]!,
    alive: n[17] === 1,
    gameMode: n[18]!,
    dimension: n[19]!,
    heldSlot: n[20]!,
    flying: n[22] === 1,
    sleeping: n[23] === 1,
    riding: n[24] === 1,
    gliding: n[25] === 1,
  }
  if (!Number.isNaN(n[21])) hot.openWindow = n[21]
  return hot
}
