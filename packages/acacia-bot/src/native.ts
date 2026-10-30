import { NativeHost } from '../native/index.js'
import { AcaciaError } from './errors.js'
import type { Command } from './generated/methods.js'
import type { Block } from './generated/types/Block.js'
import type { ColdSnapshot } from './generated/types/ColdSnapshot.js'
import type { EntityList } from './generated/types/EntityList.js'
import type { ErrorCode } from './generated/types/ErrorCode.js'
import type { HotSnapshot } from './generated/types/HotSnapshot.js'
import type { SyncReads, Transport, TransportListener } from './transport.js'

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
  /** Per bot: the last hot JSON and its parse, so a getter within one tick parses nothing. */
  readonly #hot = new Map<string, { json: string; value: HotSnapshot }>()
  readonly #cold = new Versioned<ColdSnapshot>()
  readonly #entities = new Versioned<EntityList>()

  constructor(options: NativeTransportOptions = {}) {
    this.#closed = new Promise((resolve) => (this.#resolveClosed = resolve))
    this.#host = new NativeHost({ shards: options.shards, onRecords: (batch) => this.#receive(batch) })
  }

  open(listener: TransportListener): void {
    this.#listener = listener
  }

  send(command: Command): void {
    try {
      this.#host.call(JSON.stringify(command))
    } catch (e) {
      const { code, message } = e as { code?: string; message?: string }
      throw new AcaciaError((code ?? 'INTERNAL') as ErrorCode, message ?? String(e), { cause: e })
    }
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
    hot: (bot) => {
      const json = this.#host.snapshotHot(bot)
      if (json === null) {
        this.#hot.delete(bot)
        return null
      }
      const cached = this.#hot.get(bot)
      if (cached?.json === json) return cached.value
      const value = Object.freeze(JSON.parse(json) as HotSnapshot)
      this.#hot.set(bot, { json, value })
      return value
    },
    cold: (bot) => this.#cold.read(bot, this.#host.coldVersion(bot), () => this.#host.snapshotCold(bot)),
    entities: (bot) => this.#entities.read(bot, this.#host.entitiesVersion(bot), () => this.#host.snapshotEntities(bot)),
    blockAt: (bot, x, y, z) => {
      const json = this.#host.blockAt(bot, x, y, z)
      return json === null ? null : (JSON.parse(json) as Block)
    },
  }

  #receive(batch: string | null): void {
    if (batch === null) {
      this.#ended = true
      this.#applyKeepAlive()
      this.#hot.clear()
      this.#cold.clear()
      this.#entities.clear()
      this.#listener?.closed()
      this.#resolveClosed()
      return
    }
    this.#listener?.records(JSON.parse(batch))
  }
}
