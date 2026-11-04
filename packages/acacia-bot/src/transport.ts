import type { Command } from './generated/methods.js'
import type { BotId } from './generated/types/BotId.js'
import type { Block } from './generated/types/Block.js'
import type { BlockAtCursorParams } from './generated/types/BlockAtCursorParams.js'
import type { CursorHit } from './generated/types/CursorHit.js'
import type { BlockPos } from './generated/types/BlockPos.js'
import type { ColdSnapshot } from './generated/types/ColdSnapshot.js'
import type { EntityTable } from './entity-table.js'
import type { FindBlocksParams } from './generated/types/FindBlocksParams.js'
import type { HotSnapshot } from './generated/types/HotSnapshot.js'
import type { ItemTable } from './generated/types/ItemTable.js'
import type { Out } from './generated/types/Out.js'

/** What a transport delivers to. */
export interface TransportListener {
  /** One batch of the host's record stream, in order. */
  records(records: Out[]): void
  /** The stream has ended (the host shut down); nothing follows. */
  closed(): void
}

/**
 * Reads served without a call, where the transport can (the native one can; a socket one would need a mirror).
 * `hot` and `cold` return the same frozen object until the value changes, so callers can cache on it. `entities`
 * is one `EntityTable` per bot, kept current by applying deltas to it (README, Sync reads).
 */
export interface SyncReads {
  hot(bot: BotId): HotSnapshot | null
  cold(bot: BotId): ColdSnapshot | null
  entities(bot: BotId): EntityTable | null
  blockAt(bot: BotId, x: number, y: number, z: number): Block | null
  /** The server's item names; the same object once it has arrived (it comes once, at login). */
  itemTable(bot: BotId): ItemTable | null
  /** `bot.findBlocks` without a call; throws an `AcaciaError` as the call would fail. */
  findBlocks(bot: BotId, query: FindBlocksQuery): BlockPos[]
  /** `bot.blockAtCursor` without a call; throws as `findBlocks`. */
  blockAtCursor(bot: BotId, query: Omit<BlockAtCursorParams, 'bot'>): CursorHit | null
}

export type FindBlocksQuery = Omit<FindBlocksParams, 'bot'>

/**
 * Moves commands to an acacia host and its records back (contract: the package README). `NativeTransport` runs the
 * host in-process; a WebSocket transport implements the same interface.
 */
export interface Transport {
  /** Sets the one listener; called once, before the first `send`. */
  open(listener: TransportListener): void
  /** Enqueues one command. Throws an `AcaciaError` for a refusal, which gets no `done`. */
  send(command: Command): void
  /** Whether the transport should hold Node's event loop open (only an in-process one needs telling). */
  setKeepAlive?(on: boolean): void
  /** Shuts the host down; resolves after the listener's `closed`. */
  close(): Promise<void>
  readonly reads?: SyncReads
}
