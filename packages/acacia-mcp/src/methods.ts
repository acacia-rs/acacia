import table from './generated/methods.json' with { type: 'json' }
import { BOT_PARAM, type JsonSchema } from './tool.js'

/** One entry of acacia-api's `methods.json` (its crate docs hold the format). */
export interface MethodEntry {
  name: string
  description: string
  lane: 'host' | 'queued' | 'immediate'
  flags: string[]
  params: JsonSchema
  result: JsonSchema
}

export const METHODS = table.methods as MethodEntry[]

/** The methods offered as tools: flagged `mcp`, and `privileged` ones only when allowed. */
export function mcpMethods(methods: readonly MethodEntry[] = METHODS, allowPrivileged = false): MethodEntry[] {
  return methods.filter((m) => m.flags.includes('mcp') && (allowPrivileged || !m.flags.includes('privileged')))
}

/** `bot.goto` → `goto`. */
export const toolName = (method: string) => method.replace(/^bot\./, '')

const NUMERIC_FORMAT = /^u?int(8|16|32|64)$|^float$|^double$/

/** `schema` without schemars' numeric `format`s (`int32`, ...), which validators warn about and clients ignore. */
function plain(schema: unknown): unknown {
  if (Array.isArray(schema)) return schema.map(plain)
  if (schema === null || typeof schema !== 'object') return schema
  const entries = Object.entries(schema).filter(([k, v]) => !(k === 'format' && NUMERIC_FORMAT.test(String(v))))
  return Object.fromEntries(entries.map(([k, v]) => [k, plain(v)]))
}

/** A method's params schema as a tool input: `bot` becomes the optional username param. */
export function toolInput(schema: JsonSchema): JsonSchema {
  const params = plain(schema) as JsonSchema
  const { $schema: _, title: __, properties = {}, required = [], $defs, ...rest } = params as JsonSchema & {
    properties?: Record<string, JsonSchema>
    required?: string[]
    $defs?: Record<string, JsonSchema>
  }
  const { bot: ___, ...props } = properties
  const { BotId: ____, ...defs } = $defs ?? {}
  return {
    ...rest,
    type: 'object',
    properties: { bot: BOT_PARAM, ...props },
    required: required.filter((name) => name !== 'bot'),
    ...(Object.keys(defs).length > 0 && { $defs: defs }),
  }
}

/** A method's result schema as a tool's `outputSchema`; none for methods without a result (or a non-object one). */
export function toolOutput(result: JsonSchema): JsonSchema | undefined {
  if (result.type !== 'object') return undefined
  const { $schema: _, title: __, ...rest } = plain(result) as JsonSchema
  return rest
}
