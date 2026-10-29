import type { ApiError } from './generated/types/ApiError.js'
import type { ErrorCode } from './generated/types/ErrorCode.js'
import type { MethodName } from './generated/methods.js'

export type { ErrorCode }

/** Every acacia failure; branch on `code` (`message` is free text). */
export class AcaciaError extends Error {
  readonly code: ErrorCode

  constructor(code: ErrorCode, message: string, options?: ErrorOptions) {
    super(message, options)
    this.code = code
    this.name = new.target.name
  }
}

/** A bot action failed: `TIMEOUT`, `DISCONNECTED`, `REJECTED`, `NOT_POSSIBLE`, `NOT_VISIBLE` or `CANCELLED`. */
export class ActionError extends AcaciaError {}

/** `createBot` could not connect. */
export class ConnectError extends AcaciaError {}

const ACTION_CODES = new Set<string>(['TIMEOUT', 'DISCONNECTED', 'REJECTED', 'NOT_POSSIBLE', 'NOT_VISIBLE', 'CANCELLED'])
const HOST_CODES = new Set<string>(['BUSY', 'UNKNOWN_BOT', 'UNKNOWN_METHOD', 'BAD_PARAMS', 'CLOSED', 'INTERNAL'])

/** The error class for a failed call of `method`. */
export function errorFrom(error: ApiError, method?: MethodName): AcaciaError {
  if (HOST_CODES.has(error.code)) return new AcaciaError(error.code, error.message)
  if (method === 'createBot') return new ConnectError(error.code, error.message)
  if (ACTION_CODES.has(error.code)) return new ActionError(error.code, error.message)
  return new AcaciaError(error.code, error.message)
}
