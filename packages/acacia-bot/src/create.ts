import { Bot } from './bot.js'
import type { AutoEatPolicy } from './generated/types/AutoEatPolicy.js'
import type { CreateBotParams } from './generated/types/CreateBotParams.js'
import type { EventGroup } from './generated/types/EventGroup.js'
import type { SignInParams } from './generated/types/SignInParams.js'
import type { SignInResult } from './generated/types/SignInResult.js'
import { NativeTransport } from './native.js'
import { Session, type AttachParams, type HostCallOptions, type MsaCodePrompt } from './session.js'
import type { Transport } from './transport.js'
import { WsTransport } from './ws.js'

/** Where a bot runs: a transport, or an acacia-daemon URL (`ws://127.0.0.1:19180`) with its token. */
export interface Via {
  /** The host to run on (default: one in-process `NativeTransport` shared by every bot). A `ws://` URL shares one `WsTransport` per URL. */
  via?: Transport | string
  /** The daemon's `--token`, for a `ws://` `via`. */
  token?: string
}

/** {@link createBot}'s options: the server and account, what the bot tracks, and where it runs. */
export interface BotOptions extends Omit<CreateBotParams, 'physics' | 'entities' | 'events' | 'autoEat'>, Via, HostCallOptions {
  /** Eat when hungry, from the start: `true` (or `{}`) for the defaults, or a policy (default off; `setAutoEat`). */
  autoEat?: boolean | AutoEatPolicy
  /** Simulate movement and track terrain, needed to walk (default true). */
  physics?: boolean
  /** Track entities and other players: `entities`, `players`, entity events, `follow`, `attack` (default true). */
  entities?: boolean
  /** Event groups reported whatever the listeners (default none: listeners turn their groups on). */
  events?: EventGroup[]
  /** Over acacia-daemon: keep the bot running after this connection ends, for `attachBot` (ignored in-process). */
  persist?: boolean
}

/** {@link signIn}'s options: the account, and where its tokens are cached. */
export interface SignInOptions extends SignInParams, Via, HostCallOptions {}

/** The default `onMsaCode`: the prompt's `message` on stderr, as bedrock-protocol does. */
export function logMsaCode(prompt: MsaCodePrompt): void {
  process.stderr.write(`${prompt.message}\n`)
}

/** {@link attachBot}'s options: the daemon, and the persisted bot by `bot` id or `username`. */
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

/**
 * Connects a bot; resolves once it has spawned. Rejects with a `ConnectError` when it cannot connect, or with
 * `signal.reason` when `signal` aborts first (signing in, connecting or between join retries).
 *
 * @example
 * ```js
 * const bot = await createBot({ host: '127.0.0.1', port: 19132, username: 'Sprout' })
 * bot.on('chat', ({ sender, plain }) => console.log(`<${sender}> ${plain}`))
 * await bot.ready()
 * ```
 * @example A Java Edition 26.3 server (port 25565 by default)
 * ```js
 * const bot = await createBot({ host: 'mc.example.net', edition: 'java', username: 'Sprout' })
 * ```
 * @example Over acacia-daemon, giving up after 30 s
 * ```js
 * const bot = await createBot({ host, username, via: 'ws://127.0.0.1:19180', signal: AbortSignal.timeout(30_000) })
 * ```
 */
export async function createBot(options: BotOptions): Promise<Bot> {
  const { via, token, physics = true, entities = true, events = [], persist, signal, onMsaCode = logMsaCode, autoEat, ...params } = options
  const session = sessionFor(transportFor({ via, token }))
  const keep = persist === undefined ? {} : { persist }
  const eat = autoEat === undefined || autoEat === false ? {} : { autoEat: autoEat === true ? {} : autoEat }
  const { bot: id } = await session.call('createBot', { ...params, physics, entities, events, ...keep, ...eat }, { signal, onMsaCode })
  const bot = new Bot(session, id, params.username, events)
  if (signal?.aborted) {
    // The abort came after the bot spawned: the caller gave up on it.
    await bot.close()
    throw signal.reason
  }
  return bot
}

/**
 * Signs a Microsoft account in without connecting, so later `createBot({ auth: 'microsoft' })` calls with that
 * `username` find its tokens cached. Rejects with `AUTH` when sign-in fails or the device code lapses, or with
 * `signal.reason` when `signal` aborts first.
 *
 * @example
 * ```js
 * const { displayName } = await signIn({ username: 'main', onMsaCode: ({ message }) => console.log(message) })
 * const bot = await createBot({ host: 'play.example.net', username: 'main', auth: 'microsoft' })
 * ```
 */
export async function signIn(options: SignInOptions): Promise<SignInResult> {
  const { via, token, signal, onMsaCode = logMsaCode, ...params } = options
  return sessionFor(transportFor({ via, token })).call('signIn', params, { signal, onMsaCode })
}

/**
 * Takes over a bot kept running on acacia-daemon (`createBot({ persist: true })`), by `bot` id or `username`.
 * Rejects `UNKNOWN_BOT` when the daemon has no such persisted bot.
 *
 * @example
 * ```js
 * const bot = await attachBot({ via: 'ws://127.0.0.1:19180', username: 'Keeper' })
 * ```
 */
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
