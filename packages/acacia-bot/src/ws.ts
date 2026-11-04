import { AcaciaError } from './errors.js'
import { PROTOCOL_VERSION, type Command } from './generated/methods.js'
import type { ErrorCode } from './generated/types/ErrorCode.js'
import type { Out } from './generated/types/Out.js'
import type { SyncReads, Transport, TransportListener } from './transport.js'
import { WsMirror, type MirrorRecord } from './ws-mirror.js'

/** {@link WsTransport}'s options. */
export interface WsTransportOptions {
  /** The daemon's `--token`. */
  token?: string
  /** Have the daemon push each bot's state for the sync getters (default true; off, they read null). */
  mirror?: boolean
}

interface RpcError {
  code: number
  message: string
  data?: { code?: string }
}

interface Frame {
  id?: number | string | null
  result?: unknown
  error?: RpcError
  method?: string
  params?: { records?: (Out | MirrorRecord)[] }
}

/** An open call: what its response becomes a `done` record for. */
interface OpenCall {
  method: string
  bot?: string
  persist: boolean
}

const HELLO = 'hello'

/**
 * acacia-daemon over WebSocket, JSON-RPC 2.0 (contract: the package README; the protocol: acacia-daemon's crate
 * docs). Connects on the first command, and closes the socket while the session holds nothing open.
 * `createBot({ via: 'ws://...' })` makes and shares one per URL and token; build one to pass options or own its
 * lifetime.
 *
 * @example
 * ```js
 * const via = new WsTransport('ws://127.0.0.1:19180', { token: process.env.ACACIA_TOKEN })
 * const bot = await createBot({ host: '127.0.0.1', username: 'Remote', via })
 * // ...
 * await via.close()
 * ```
 */
export class WsTransport implements Transport {
  /** The daemon's URL. */
  readonly url: string
  readonly #options: WsTransportOptions
  #listener?: TransportListener
  #socket?: WebSocket
  #ready = false
  readonly #queued: string[] = []
  readonly #calls = new Map<number, OpenCall>()
  /** Live bots seen on this transport, and whether each persists. */
  readonly #bots = new Map<string, boolean>()
  readonly #mirror = new WsMirror()
  #closing = false
  #ended = false
  readonly #closed: Promise<void>
  #resolveClosed!: () => void

  constructor(url: string, options: WsTransportOptions = {}) {
    this.url = url
    this.#options = options
    this.#closed = new Promise((resolve) => (this.#resolveClosed = resolve))
  }

  /** The mirror's reads (no terrain); undefined with `mirror: false`. */
  get reads(): SyncReads | undefined {
    return this.#options.mirror === false ? undefined : this.#mirror.reads
  }

  /** The stream has ended (closed, or the connection dropped); a new transport is needed. */
  get ended(): boolean {
    return this.#ended
  }

  open(listener: TransportListener): void {
    this.#listener = listener
  }

  send(command: Command): void {
    if (this.#ended || this.#closing) throw new AcaciaError('CLOSED', 'the transport is closed')
    const params = command.params as { bot?: string; persist?: boolean }
    this.#calls.set(command.call, { method: command.method, bot: params.bot, persist: params.persist === true })
    this.#write(JSON.stringify({ jsonrpc: '2.0', id: command.call, method: command.method, params }))
  }

  /** Off with nothing open: the socket closes, so Node can exit; the next command reconnects. */
  setKeepAlive(on: boolean): void {
    if (on || this.#calls.size > 0 || this.#bots.size > 0 || !this.#socket) return
    const socket = this.#socket
    this.#socket = undefined
    this.#ready = false
    socket.close(1000)
  }

  close(): Promise<void> {
    if (!this.#ended && !this.#closing) {
      this.#closing = true
      if (this.#socket) this.#socket.close(1000)
      else this.#end('CLOSED', 'the transport closed')
    }
    return this.#closed
  }

  #write(frame: string): void {
    if (this.#ready) return this.#socket!.send(frame)
    this.#queued.push(frame)
    if (!this.#socket) this.#connect()
  }

  #connect(): void {
    const socket = new WebSocket(this.url)
    this.#socket = socket
    socket.onopen = () => {
      const { token, mirror = true } = this.#options
      const params = { protocolVersion: PROTOCOL_VERSION, client: '@acacia-rs/bot', mirror, ...(token === undefined ? {} : { token }) }
      socket.send(JSON.stringify({ jsonrpc: '2.0', id: HELLO, method: 'hello', params }))
    }
    socket.onmessage = (message) => {
      if (socket === this.#socket) this.#frame(JSON.parse(String(message.data)) as Frame)
    }
    socket.onclose = () => {
      if (socket !== this.#socket) return
      const reached = this.#ready
      this.#socket = undefined
      this.#ready = false
      if (this.#closing) this.#end('CLOSED', 'the transport closed')
      else if (reached) this.#end('DISCONNECTED', `the connection to ${this.url} dropped`)
      else this.#end('IO', `cannot reach an acacia daemon at ${this.url}`)
    }
  }

  #frame(frame: Frame): void {
    if (frame.id === HELLO) {
      if (frame.error) return this.#end(codeOf(frame.error), frame.error.message)
      this.#ready = true
      for (const queued of this.#queued.splice(0)) this.#socket!.send(queued)
      return
    }
    if (frame.method === 'out') return this.#records(frame.params?.records ?? [])
    if (typeof frame.id === 'number') this.#response(frame.id, frame)
  }

  #records(records: (Out | MirrorRecord)[]): void {
    const out: Out[] = []
    for (const record of records) {
      if (record.type === 'mirror') {
        this.#mirror.update(record)
        continue
      }
      if (record.type === 'event' && record.event.type === 'end') {
        this.#bots.delete(record.bot)
        this.#mirror.forget(record.bot)
      }
      out.push(record)
    }
    if (out.length > 0) this.#listener?.records(out)
  }

  #response(call: number, frame: Frame): void {
    const open = this.#calls.get(call)
    if (!open) return
    this.#calls.delete(call)
    const ok = frame.result as { bot?: string } | null
    if (!frame.error && (open.method === 'createBot' || open.method === 'attach') && ok?.bot) {
      this.#bots.set(ok.bot, open.persist || (ok as { persist?: boolean }).persist === true)
    }
    const result = frame.error ? { err: { code: codeOf(frame.error), message: frame.error.message } } : { ok: frame.result }
    this.#listener?.records([{ type: 'done', bot: open.bot ?? ok?.bot ?? '', call, result } as Out])
  }

  /** The stream ends: open calls fail with `code`, live bots get an `end`, then the listener's `closed`. */
  #end(code: ErrorCode, message: string): void {
    if (this.#ended) return
    this.#ended = true
    this.#socket?.close(1000)
    this.#socket = undefined
    const records: Out[] = []
    for (const [call, open] of this.#calls) {
      records.push({ type: 'done', bot: open.bot ?? '', call, result: { err: { code, message } } })
    }
    for (const [bot, persist] of this.#bots) {
      const why = persist ? `${message}; the bot keeps running on the daemon (attachBot)` : message
      records.push({ type: 'event', bot, seq: 0, event: { type: 'end', reason: this.#closing ? 'closed' : 'lost', message: why } })
    }
    this.#calls.clear()
    this.#bots.clear()
    this.#mirror.clear()
    this.#queued.length = 0
    if (records.length > 0) this.#listener?.records(records)
    this.#listener?.closed()
    this.#resolveClosed()
  }
}

function codeOf(error: RpcError): ErrorCode {
  return (error.data?.code ?? 'INTERNAL') as ErrorCode
}
