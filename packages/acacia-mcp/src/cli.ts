#!/usr/bin/env node
import { parseArgs } from 'node:util'

import { serveStdio } from '@modelcontextprotocol/server/stdio'

import { BotRegistry, type ConnectOptions } from './bots.js'
import { createServer } from './server.js'

const USAGE = `usage: acacia-mcp [--host H] [--port P] [--username NAME] [--offline] [--transport auto|raknet|nethernet]
                  [--allow-commands] [--no-connect]
MCP server on stdio. With --host and --username the bot connects at start; --no-connect waits for the connect tool.`

const { values } = parseArgs({
  options: {
    host: { type: 'string' },
    port: { type: 'string' },
    username: { type: 'string' },
    offline: { type: 'boolean' },
    transport: { type: 'string' },
    'allow-commands': { type: 'boolean' },
    'no-connect': { type: 'boolean' },
    help: { type: 'boolean', short: 'h' },
  },
})
if (values.help) {
  console.error(USAGE)
  process.exit(0)
}

// --offline is accepted for familiarity: offline auth is the only kind so far.
const defaults: ConnectOptions = {
  host: values.host,
  port: values.port === undefined ? undefined : Number(values.port),
  username: values.username,
  transport: values.transport as ConnectOptions['transport'],
}
const bots = new BotRegistry(defaults)
const log = (message: string) => process.stderr.write(`acacia-mcp: ${message}\n`)

if (defaults.host && defaults.username && !values['no-connect']) {
  bots.connect().then(
    (h) => log(`${h.name} connected to ${defaults.host}`),
    (e: Error) => log(`connecting ${defaults.username} failed: ${e.message}`),
  )
}

const handle = serveStdio(() => createServer(bots, { allowCommands: values['allow-commands'] }), {
  onerror: (e) => log(e.message),
})

let closing = false
async function shutdown() {
  if (closing) return
  closing = true
  await handle.close().catch(() => {})
  await bots.close()
}
// The bots' host keeps Node alive, so the client closing stdin must end them for the process to exit.
process.stdin.once('end', shutdown)
process.stdin.once('close', shutdown)
process.once('SIGINT', shutdown)
process.once('SIGTERM', shutdown)
