import { createRequire } from 'node:module'

import type { NativeHost } from '../native/index.js'
import { NativeEntities } from './native-entities.js'
import { NativeHot } from './native-hot.js'
import { AcaciaError } from './errors.js'
import type { Command } from './generated/methods.js'
import type { Block } from './generated/types/Block.js'
import type { ColdSnapshot } from './generated/types/ColdSnapshot.js'
import type { ErrorCode } from './generated/types/ErrorCode.js'
import type { FindBlocksResult } from './generated/types/FindBlocksResult.js'
import type { ItemTable } from './generated/types/ItemTable.js'
import type { SyncReads, Transport, TransportListener } from './transport.js'

let addon: typeof import('../native/index.js') | undefined

/** The addon, loaded by the first `NativeTransport` so WebSocket-only use needs no binary (README, Packaging). */
function loadAddon(): typeof import('../native/index.js') {
  return (addon ??= createRequire(import.meta.url)('../native/index.js'))
}

/** Runs an addon call, turning what it throws (a `code` and a message) into an `AcaciaError`. */
function rethrown<T>(call: () => T): T {
  try {
    return call()
  } catch (e) {
    const { code, message } = e as { code?: string; message?: string }
    throw new AcaciaError((code ?? 'INTERNAL') as ErrorCode, message ?? String(e), { cause: e })
  }
}

/** Per bot, the parse of a versioned read, refetched only when its version moves. */
class Versioned<T> {
  readonly #last = new Map<string, { version: number; value: T }>()

  read(bot: string, version: number | null, fetch: () => string | null): T | null {
    const cached = this.#last.get(bot)
    if (version === null) {
      this.#last.delete(bot)
      return null
    }
    if (cached?.version === version) return cached.value
    const json = fetch()
    if (json === null) return null
    const value = Object.freeze(JSON.parse(json) as T)
    this.#last.set(bot, { version, value })
    return value
  }

  clear(): void {
    this.#last.clear()
  }
}

export interface NativeTransportOptions {
  /** Shard threads running the bots (default: the CPU count, at most 4). */
  shards?: number
  /** Bots publish their state for the sync reads (default true; off, `bot.hot` and the other reads are null). */
  snapshots?: boolean
  /** Join spacing per address and its jitter, ms (default 500 and 250; why: the package README, Joins). */
  joinSpacingMs?: number
  joinJitterMs?: number
  /** Largest decompressed batch a bot accepts, bytes (default 128 MiB; README, Joins). */
  maxBatchBytes?: number
}

/** The acacia host in this process, through the native addon (crates/acacia-node). */
export class NativeTransport implements Transport {
  readonly #host: NativeHost
  #listener?: TransportListener
  /** Keep-alive asked for by the session, by a close in progress; never after the stream ended. */
  #held = false
  #closing = false
  #ended = false
  readonly #closed: Promise<void>
  #resolveClosed!: () => void
  readonly #hot: NativeHot
  readonly #cold = new Versioned<ColdSnapshot>()
  readonly #entities: NativeEntities
  /** Per bot, the item table once it arrived (it comes once). */
  readonly #items = new Map<string, ItemTable>()

  constructor(options: NativeTransportOptions = {}) {
    this.#closed = new Promise((resolve) => (this.#resolveClosed = resolve))
    this.#host = new (loadAddon().NativeHost)({ ...options, onRecords: (batch) => this.#receive(batch) })
    this.#entities = new NativeEntities(this.#host)
    this.#hot = new NativeHot(this.#host)
  }

  open(listener: TransportListener): void {
    this.#listener = listener
  }

  send(command: Command): void {
    rethrown(() => this.#host.call(JSON.stringify(command)))
  }

  setKeepAlive(on: boolean): void {
    this.#held = on
    this.#applyKeepAlive()
  }

  /** Holds the event loop itself until the stream has ended. */
  close(): Promise<void> {
    this.#closing = true
    this.#applyKeepAlive()
    this.#host.close()
    return this.#closed
  }

  #applyKeepAlive(): void {
    this.#host.setKeepAlive(!this.#ended && (this.#held || this.#closing))
  }

  readonly reads: SyncReads = {
    hot: (bot) => this.#hot.read(bot),
    cold: (bot) => this.#cold.read(bot, this.#host.coldVersion(bot), () => this.#host.snapshotCold(bot)),
    entities: (bot) => this.#entities.read(bot),
    blockAt: (bot, x, y, z) => {
      const json = this.#host.blockAt(bot, x, y, z)
      return json === null ? null : (JSON.parse(json) as Block)
    },
    itemTable: (bot) => {
      const cached = this.#items.get(bot)
      if (cached) return cached
      const json = this.#host.itemTable(bot)
      if (json === null) return null
      const table = Object.freeze(JSON.parse(json) as ItemTable)
      if (table.items.length > 0) this.#items.set(bot, table)
      return table
    },
    findBlocks: (bot, query) => {
      const json = rethrown(() => this.#host.findBlocks(bot, JSON.stringify({ ...query, bot })))
      return (JSON.parse(json) as FindBlocksResult).blocks
    },
  }

  #receive(batch: string | null): void {
    if (batch === null) {
      this.#ended = true
      this.#applyKeepAlive()
      this.#hot.clear()
      this.#cold.clear()
      this.#entities.clear()
      this.#items.clear()
      this.#listener?.closed()
      this.#resolveClosed()
      return
    }
    this.#listener?.records(JSON.parse(batch))
  }
}
