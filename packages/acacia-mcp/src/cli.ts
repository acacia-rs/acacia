#!/usr/bin/env node
import { parseArgs } from 'node:util'

import { serveStdio } from '@modelcontextprotocol/server/stdio'

import { BotRegistry, type ConnectOptions } from './bots.js'
import { createServer } from './server.js'

const USAGE = `usage: acacia-mcp [--host H] [--port P] [--username NAME] [--auth offline|microsoft] [--offline]
                  [--profiles-folder DIR] [--transport auto|raknet|nethernet] [--allow-commands] [--no-connect]
MCP server on stdio. With --host and --username the bot connects at start; --no-connect waits for the connect tool.`

const { values } = parseArgs({
  options: {
    host: { type: 'string' },
    port: { type: 'string' },
    username: { type: 'string' },
    auth: { type: 'string' },
    offline: { type: 'boolean' },
    'profiles-folder': { type: 'string' },
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

if (values.auth !== undefined && values.auth !== 'offline' && values.auth !== 'microsoft') {
  console.error(`--auth takes offline or microsoft\n${USAGE}`)
  process.exit(2)
}
// --offline is bedrock-protocol's spelling of --auth offline.
const defaults: ConnectOptions = {
  host: values.host,
  port: values.port === undefined ? undefined : Number(values.port),
  username: values.username,
  transport: values.transport as ConnectOptions['transport'],
  auth: values.offline ? 'offline' : (values.auth as ConnectOptions['auth']),
  profilesFolder: values['profiles-folder'],
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
