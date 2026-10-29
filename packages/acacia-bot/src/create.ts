import { Bot } from './bot.js'
import type { CreateBotParams } from './generated/types/CreateBotParams.js'
import type { EventGroup } from './generated/types/EventGroup.js'
import { NativeTransport } from './native.js'
import { Session } from './session.js'
import type { Transport } from './transport.js'

export interface BotOptions extends Omit<CreateBotParams, 'physics' | 'events'> {
  /** Simulate movement and track terrain, needed to walk (default true). */
  physics?: boolean
  /** Event groups reported whatever the listeners (default none: listeners turn their groups on). */
  events?: EventGroup[]
  /** The host to run on (default: one in-process `NativeTransport` shared by every bot). */
  via?: Transport
}

const sessions = new WeakMap<Transport, Session>()
let shared: NativeTransport | undefined

function sessionFor(transport: Transport): Session {
  let session = sessions.get(transport)
  if (!session) sessions.set(transport, (session = new Session(transport)))
  return session
}

/** Connects a bot; resolves once it has spawned. Rejects with a `ConnectError` when it cannot connect. */
export async function createBot(options: BotOptions): Promise<Bot> {
  const { via, physics = true, events = [], ...params } = options
  const session = sessionFor(via ?? (shared ??= new NativeTransport()))
  const { bot } = await session.call('createBot', { ...params, physics, events })
  return new Bot(session, bot, params.username, events)
}

/** Shuts down the shared in-process host (every bot on it closes); later `createBot` calls start a new one. */
export async function closeAll(): Promise<void> {
  const transport = shared
  shared = undefined
  if (transport) await sessionFor(transport).close()
}
