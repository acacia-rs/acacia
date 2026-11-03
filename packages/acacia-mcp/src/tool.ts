import { AcaciaError } from '@acacia/bot'

import type { BotHandle, BotRegistry } from './bots.js'

export type JsonSchema = Record<string, unknown>
export type Args = Record<string, unknown>

/** A schema's properties. */
export const props = (schema: JsonSchema) => schema.properties as Record<string, JsonSchema>

/** `base` with properties added or replaced. */
export const withProps = (base: JsonSchema, add: Record<string, JsonSchema>): JsonSchema => ({
  ...base,
  properties: { ...props(base), ...add },
})

/** What a tool's result carries: `text` for the model, `structured` (an object) for clients that read it. */
export type ToolResult = {
  content: { type: 'text'; text: string }[]
  structuredContent?: Record<string, unknown>
  isError?: boolean
}

export interface ToolContext {
  bots: BotRegistry
  /** Aborted when the client cancels the request. */
  signal: AbortSignal
}

/** One MCP tool: generated from the method table (`generated.ts`) or hand-written (`composite.ts`). */
export interface ToolDef {
  name: string
  description: string
  inputSchema: JsonSchema
  outputSchema?: JsonSchema
  annotations?: { readOnlyHint?: boolean; destructiveHint?: boolean; openWorldHint?: boolean }
  run(args: Record<string, unknown>, ctx: ToolContext): Promise<ToolResult>
}

/** How one method's tool differs from its generated form; every hook is optional (contract: README "Tools"). */
export interface Adapter {
  /** Rewrites the generated input schema (to keep it flat, or to add tool-only params). */
  schema?(base: JsonSchema): JsonSchema
  /** Appended to the method's description. */
  note?: string
  /** Tool args (without `bot`) to the method's params (without `bot`). */
  params?(args: Args, handle: BotHandle): Args
  /** Milliseconds the call may run before it is cancelled (from the tool args). */
  timeout?(args: Args): number
  /** The result when the deadline ends the call, for calls that never end on their own (else a `TIMEOUT` error). */
  untilDeadline?(args: Args, handle: BotHandle): ToolResult
  /** The success result. */
  done?(result: unknown, params: Args, handle: BotHandle): ToolResult
}

export function ok(text: string, structured?: Record<string, unknown>): ToolResult {
  return { content: [{ type: 'text', text }], ...(structured && { structuredContent: structured }) }
}

/**
 * A failure as a tool error result (never a protocol error): `CODE: message` with the acacia code, `TIMEOUT` for a
 * tool deadline and `CANCELLED` for a cancelled request or `stop`.
 */
export function failure(error: unknown): ToolResult {
  const { code, message } = describe(error)
  return { content: [{ type: 'text', text: `${code}: ${message}` }], structuredContent: { error: { code, message } }, isError: true }
}

function describe(error: unknown): { code: string; message: string } {
  if (error instanceof AcaciaError) return { code: error.code, message: error.message }
  if (error instanceof Error && error.name === 'TimeoutError') return { code: 'TIMEOUT', message: error.message }
  if (error instanceof Error && error.name === 'AbortError') return { code: 'CANCELLED', message: error.message }
  return { code: 'INTERNAL', message: error instanceof Error ? error.message : String(error) }
}

export const badParams = (message: string) => new AcaciaError('BAD_PARAMS', message)

/** Runs `body`, turning a throw into a tool error result. */
export async function guarded(body: () => Promise<ToolResult>): Promise<ToolResult> {
  try {
    return await body()
  } catch (error) {
    return failure(error)
  }
}

/** The optional `bot` param every bot tool takes. */
export const BOT_PARAM: JsonSchema = {
  type: 'string',
  description: 'The bot (its username) to act as; may be left out while only one bot is connected.',
}

/** An object schema with the optional `bot` param added. */
export function botSchema(properties: Record<string, JsonSchema>, required: string[] = [], extra: JsonSchema = {}): JsonSchema {
  return { type: 'object', ...extra, properties: { bot: BOT_PARAM, ...properties }, required }
}

type Pos = { x: number; y: number; z: number }

export const round = (n: number) => Math.round(n * 10) / 10
export const fmtPos = (p: Pos) => `${round(p.x)} ${round(p.y)} ${round(p.z)}`
/** Euclidean, to a tenth of a block. */
export const distance = (a: Pos, b: Pos) => round(Math.hypot(a.x - b.x, a.y - b.y, a.z - b.z))
/** `; <bot> is now at x y z`, or nothing before the bot has a position. */
export const here = (handle: BotHandle) => {
  const position = handle.bot.entity?.position
  return position ? `; ${handle.name} is now at ${fmtPos(position)}` : ''
}
/** `zombie` → `minecraft:zombie`. */
export const kindName = (kind: string) => (kind.includes(':') ? kind : `minecraft:${kind}`).toLowerCase()
