import { WeftError } from "./errors.js"

/** What every handler of every transport sees. Transports add their own fields. */
export interface Context {
  readonly transport: string
  /** The operation: an event name, a route name, a method. */
  readonly operation: string
  /** Aborted when the call is cancelled, its deadline passes, or shutdown gives up on it. */
  readonly signal: AbortSignal
  readonly headers: Record<string, string>
  /** Absolute deadline in milliseconds since the epoch, if the call has one. */
  readonly deadline?: number
  /** Scratch space for middleware to pass values along. */
  readonly state: Record<string, unknown>
}

export type Next = () => Promise<void>
export type Middleware<C extends Context = Context> = (ctx: C, next: Next) => Promise<void> | void

/** Milliseconds until the context's deadline, or `undefined` when it has none. Never negative. */
export function remainingMs(ctx: Pick<Context, "deadline">, now = Date.now()): number | undefined {
  return ctx.deadline === undefined ? undefined : Math.max(0, ctx.deadline - now)
}

/**
 * One chain of middleware, ending in `handler`. Each middleware calls `next()` once to continue;
 * not calling it stops the chain, calling it twice is an error.
 */
export function compose<C extends Context>(
  middleware: readonly Middleware<C>[],
): (ctx: C, handler: (ctx: C) => Promise<void> | void) => Promise<void> {
  return (ctx, handler) => {
    let last = -1
    const run = async (i: number): Promise<void> => {
      if (i <= last) throw WeftError.internal("next() was called more than once in the same middleware")
      last = i
      const mw = middleware[i]
      if (mw === undefined) {
        await handler(ctx)
        return
      }
      await mw(ctx, () => run(i + 1))
    }
    return run(0)
  }
}
