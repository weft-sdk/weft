import type { Middleware } from "../middleware.js"
import type { MessageContext } from "./types.js"

export const IDEMPOTENCY_HEADER = "weft-idempotency-key"

export interface DedupeOptions {
  /** How many keys to remember. Default 10 000. The oldest is forgotten first. */
  readonly max?: number
}

/**
 * Skips a message whose idempotency key (else its CloudEvents id) was already handled successfully.
 * Memory only: it covers redelivery to this process, not a restart. A key is remembered only after
 * the handler succeeds, so a failed message is retried.
 */
export function dedupe(options: DedupeOptions = {}): Middleware<MessageContext> {
  const max = options.max ?? 10_000
  const seen = new Set<string>()
  return async (ctx, next) => {
    const key = ctx.envelope.headers[IDEMPOTENCY_HEADER] ?? ctx.envelope.id
    if (seen.has(key)) return
    await next()
    seen.add(key)
    if (seen.size > max) seen.delete(seen.values().next().value as string)
  }
}
