import { Bot } from './bot.js'
import type { CreateBotParams } from './generated/types/CreateBotParams.js'
import type { EventGroup } from './generated/types/EventGroup.js'
import type { SignInParams } from './generated/types/SignInParams.js'
import type { SignInResult } from './generated/types/SignInResult.js'
import { NativeTransport } from './native.js'
import { Session, type AttachParams, type MsaCodePrompt } from './session.js'
import type { Transport } from './transport.js'
import { WsTransport } from './ws.js'

/** Where a bot runs: a transport, or an acacia-daemon URL (`ws://127.0.0.1:19180`) with its token. */
export interface Via {
  /** The host to run on (default: one in-process `NativeTransport` shared by every bot). A `ws://` URL shares one `WsTransport` per URL. */
  via?: Transport | string
  /** The daemon's `--token`, for a `ws://` `via`. */
  token?: string
}

export interface BotOptions extends Omit<CreateBotParams, 'physics' | 'entities' | 'events'>, Via {
  /** Simulate movement and track terrain, needed to walk (default true). */
  physics?: boolean
  /** Track entities and other players: `entities`, `players`, entity events, `follow`, `attack` (default true). */
  entities?: boolean
  /** Event groups reported whatever the listeners (default none: listeners turn their groups on). */
  events?: EventGroup[]
  /** Over acacia-daemon: keep the bot running after this connection ends, for `attachBot` (ignored in-process). */
  persist?: boolean
  /** `auth: 'microsoft'`: shows a device code to a person when the account is not signed in (default `logMsaCode`). */
  onMsaCode?: (prompt: MsaCodePrompt) => void
}

export interface SignInOptions extends SignInParams, Via {
  /** Shows the device code to a person (default `logMsaCode`). */
  onMsaCode?: (prompt: MsaCodePrompt) => void
}

/** The default `onMsaCode`: the prompt's `message` on stderr, as bedrock-protocol does. */
export function logMsaCode(prompt: MsaCodePrompt): void {
  process.stderr.write(`${prompt.message}\n`)
}

export interface AttachOptions extends AttachParams, Via {
  /** Event groups reported whatever the listeners (default none). */
  events?: EventGroup[]
}

const sessions = new WeakMap<Transport, Session>()
const sockets = new Map<string, WsTransport>()
let shared: NativeTransport | undefined

function sessionFor(transport: Transport): Session {
  let session = sessions.get(transport)
  if (!session) sessions.set(transport, (session = new Session(transport)))
  return session
}

function transportFor({ via, token }: Via): Transport {
  if (via === undefined) return (shared ??= new NativeTransport())
  if (typeof via !== 'string') return via
  const key = `${via}\n${token ?? ''}`
  let socket = sockets.get(key)
  if (!socket || socket.ended) sockets.set(key, (socket = new WsTransport(via, { token })))
  return socket
}

/** Connects a bot; resolves once it has spawned. Rejects with a `ConnectError` when it cannot connect. */
export async function createBot(options: BotOptions): Promise<Bot> {
  const { via, token, physics = true, entities = true, events = [], persist, onMsaCode = logMsaCode, ...params } = options
  const session = sessionFor(transportFor({ via, token }))
  const keep = persist === undefined ? {} : { persist }
  const { bot } = await session.call('createBot', { ...params, physics, entities, events, ...keep }, { onMsaCode })
  return new Bot(session, bot, params.username, events)
}

/**
 * Signs a Microsoft account in without connecting, so later `createBot({ auth: 'microsoft' })` calls with that
 * `username` find its tokens cached. Rejects with `AUTH` when sign-in fails or the device code lapses.
 */
export async function signIn(options: SignInOptions): Promise<SignInResult> {
  const { via, token, onMsaCode = logMsaCode, ...params } = options
  return sessionFor(transportFor({ via, token })).call('signIn', params, { onMsaCode })
}

/** Takes over a bot kept running on acacia-daemon (`createBot({ persist: true })`), by `bot` id or `username`. */
export async function attachBot(options: AttachOptions): Promise<Bot> {
  const { via, token, events = [], ...which } = options
  const transport = transportFor({ via, token })
  const { bot, username } = await sessionFor(transport).attachBot(which)
  return new Bot(sessionFor(transport), bot, username, events)
}

/** Shuts down the shared in-process host and disconnects the shared `ws://` transports (their bots close unless persisted). */
export async function closeAll(): Promise<void> {
  const transports: Transport[] = [...sockets.values()]
  if (shared) transports.push(shared)
  shared = undefined
  sockets.clear()
  await Promise.all(transports.map((t) => sessionFor(t).close()))
}
