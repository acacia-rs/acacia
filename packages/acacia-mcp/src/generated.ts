import type { MethodName } from '@acacia/bot'

import { ADAPTERS, type Adapter } from './adapters.js'
import type { BotHandle } from './bots.js'
import { toolInput, toolName, toolOutput, type MethodEntry } from './methods.js'
import { fmtPos, guarded, ok, type ToolDef, type ToolResult } from './tool.js'

/** Deadlines by lane where the adapter names none; immediate calls answer at once, so theirs only guards a hang. */
const LANE_TIMEOUT = { queued: 30_000, immediate: 10_000, host: 60_000 }

/** One tool per method: the table's description and schemas, reshaped by the method's adapter (if any). */
export function generatedTools(methods: readonly MethodEntry[]): ToolDef[] {
  return methods.map((method) => toolFor(method, ADAPTERS[method.name] ?? {}))
}

function toolFor(method: MethodEntry, adapter: Adapter): ToolDef {
  const base = toolInput(method.params)
  const outputSchema = toolOutput(method.result)
  return {
    name: toolName(method.name),
    description: adapter.note ? `${method.description.trimEnd()} ${adapter.note}` : method.description,
    inputSchema: adapter.schema?.(base) ?? base,
    ...(outputSchema && { outputSchema }),
    run: ({ bot, ...args }, { bots, signal }) =>
      guarded(async () => {
        const handle = await bots.get(bot as string | undefined)
        const params = adapter.params?.(args, handle) ?? args
        const ms = adapter.timeout?.(args) ?? LANE_TIMEOUT[method.lane]
        let result: unknown
        try {
          result = await handle.action(signal, ms, (signal) =>
            handle.bot.request(method.name as MethodName, params as never, { signal }),
          )
        } catch (error) {
          if (adapter.untilDeadline && error instanceof Error && error.name === 'TimeoutError') {
            return adapter.untilDeadline(args, handle)
          }
          throw error
        }
        return adapter.done?.(result, params, handle) ?? doneResult(method, result, handle)
      }),
  }
}

function doneResult(method: MethodEntry, result: unknown, handle: BotHandle): ToolResult {
  if (result !== null && typeof result === 'object') return ok(JSON.stringify(result), result as Record<string, unknown>)
  const position = method.lane === 'queued' ? handle.bot.entity?.position : undefined
  return ok(position ? `done; ${handle.name} is now at ${fmtPos(position)}` : 'done')
}
