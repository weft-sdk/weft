import { DEFAULTS } from "../defaults.js"

export interface BackoffOptions {
  readonly minMs?: number
  readonly maxMs?: number
}

/**
 * Exponential backoff with jitter: the delay doubles from `minMs` up to `maxMs`, then a random
 * amount between half of it and all of it is used. `attempt` is the one that just failed (1, 2, ...).
 */
export function backoffMs(attempt: number, options: BackoffOptions = {}, random: () => number = Math.random): number {
  const min = options.minMs ?? DEFAULTS.messaging.backoffMinMs
  const max = options.maxMs ?? DEFAULTS.messaging.backoffMaxMs
  const base = Math.min(max, min * 2 ** Math.max(0, attempt - 1))
  return Math.round(base / 2 + (base / 2) * random())
}

/** Waits `ms`, or less if `signal` aborts. Resolves either way; the caller checks the signal. */
export function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    if (signal?.aborted) return resolve()
    const done = (): void => {
      clearTimeout(timer)
      signal?.removeEventListener("abort", done)
      resolve()
    }
    const timer = setTimeout(done, ms)
    signal?.addEventListener("abort", done, { once: true })
  })
}
