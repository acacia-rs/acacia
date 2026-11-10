// A minimal WebSocket server (text frames, JSON) standing in for acacia-daemon in WsTransport's unit tests.
import { createHash } from 'node:crypto'
import { EventEmitter, once } from 'node:events'
import { createServer } from 'node:http'

const GUID = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11'

/** One client connection: `message` events carry parsed JSON, `close` fires when it ends. */
class Peer extends EventEmitter {
  #buf = Buffer.alloc(0)
  messages = []

  constructor(socket) {
    super()
    this.socket = socket
    socket.on('data', (d) => this.#data(d))
    socket.on('close', () => this.emit('close'))
  }

  #data(data) {
    this.#buf = Buffer.concat([this.#buf, data])
    for (;;) {
      if (this.#buf.length < 2) return
      const opcode = this.#buf[0] & 0x0f
      let len = this.#buf[1] & 0x7f
      let at = 2
      if (len === 126) [len, at] = [this.#buf.readUInt16BE(2), 4]
      else if (len === 127) [len, at] = [Number(this.#buf.readBigUInt64BE(2)), 10]
      if (this.#buf.length < at + 4 + len) return
      const mask = this.#buf.subarray(at, at + 4)
      const payload = Buffer.from(this.#buf.subarray(at + 4, at + 4 + len)).map((b, i) => b ^ mask[i % 4])
      this.#buf = this.#buf.subarray(at + 4 + len)
      if (opcode === 1) {
        const message = JSON.parse(payload.toString())
        this.messages.push(message)
        this.emit('message', message)
      } else if (opcode === 8) {
        this.socket.end(Buffer.from([0x88, 0]))
      }
    }
  }

  /** The next message (one already received but not yet taken counts). */
  async next() {
    if (this.messages.length > this.taken) return this.messages[this.taken++]
    await once(this, 'message')
    return this.messages[this.taken++]
  }
  taken = 0

  send(value) {
    const data = Buffer.from(JSON.stringify(value))
    const head = data.length < 126 ? Buffer.from([0x81, data.length]) : Buffer.from([0x81, 126, data.length >> 8, data.length & 0xff])
    this.socket.write(Buffer.concat([head, data]))
  }

  /** Answers the hello with `result` (or `error`); returns the hello's params. */
  async greet(reply = { result: { protocolVersion: 1, reads: false, mirror: true } }) {
    const hello = await this.next()
    this.send({ jsonrpc: '2.0', id: hello.id, ...reply })
    return hello.params
  }

  drop() {
    this.socket.destroy()
  }
}

/** Listens on a free port; `connection()` resolves with the next peer. */
export async function wsServer() {
  const peers = new EventEmitter()
  const server = createServer()
  server.on('upgrade', (req, socket) => {
    const accept = createHash('sha1').update(req.headers['sec-websocket-key'] + GUID).digest('base64')
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`)
    peers.emit('peer', new Peer(socket))
  })
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  return {
    url: `ws://127.0.0.1:${server.address().port}`,
    connection: async () => (await once(peers, 'peer'))[0],
    close: () => {
      server.closeAllConnections()
      server.close()
    },
  }
}
