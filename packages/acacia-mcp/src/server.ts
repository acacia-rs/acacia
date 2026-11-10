import { fromJsonSchema, McpServer, type StandardSchemaWithJSON } from '@modelcontextprotocol/server'

import type { BotRegistry } from './bots.js'
import { COMPOSITE_TOOLS } from './composite.js'
import { generatedTools } from './generated.js'
import { mcpMethods, METHODS, type MethodEntry } from './methods.js'
import type { ToolDef } from './tool.js'

type Input = StandardSchemaWithJSON<Record<string, unknown>, Record<string, unknown>>

export interface ToolOptions {
  /** Offer `privileged` methods (`command`: slash commands). */
  allowCommands?: boolean
  /** The method table (default: the one this package was built with). */
  methods?: readonly MethodEntry[]
}

/** Every tool: the hand-written ones, then one per `mcp`-flagged method. */
export function tools(options: ToolOptions = {}): ToolDef[] {
  return [...COMPOSITE_TOOLS, ...generatedTools(mcpMethods(options.methods ?? METHODS, options.allowCommands))]
}

const INSTRUCTIONS = `Controls Minecraft Bedrock bots. Call status first to see where the bot is and who is near; actions such as
goto, follow and dig wait until done and can be cancelled. Errors start with an acacia code (NOT_POSSIBLE: cannot be
done from here; NOT_VISIBLE: hidden behind blocks, move and retry; TIMEOUT; CANCELLED). Read new chat with readChat.`

/** One MCP server instance over the shared `bots` (the stdio entry builds one per connection). */
export function createServer(bots: BotRegistry, options: ToolOptions = {}): McpServer {
  const server = new McpServer({ name: 'acacia', version: '0.1.0' }, { instructions: INSTRUCTIONS })
  for (const tool of tools(options)) {
    const config = {
      description: tool.description,
      inputSchema: fromJsonSchema<Record<string, unknown>>(tool.inputSchema),
      outputSchema: tool.outputSchema && fromJsonSchema(tool.outputSchema),
      annotations: tool.annotations,
    }
    server.registerTool<StandardSchemaWithJSON, Input>(tool.name, config as typeof config & { outputSchema: StandardSchemaWithJSON }, (args, ctx) =>
      tool.run(args, { bots, signal: ctx.mcpReq.signal }),
    )
  }
  return server
}
