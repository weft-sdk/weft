import { WeftError } from "./errors.js"

/**
 * Runs `fn` with an abort signal that fires after `ms`, or when `parent` aborts. If the time runs
 * out first the result is a `Timeout` error; if the parent aborts, `Unavailable`. `fn` should pass
 * the signal on to whatever it calls.
 */
export async function withTimeout<T>(ms: number, fn: (signal: AbortSignal) => Promise<T>, parent?: AbortSignal): Promise<T> {
  if (parent?.aborted) throw WeftError.unavailable("The call was cancelled")
  const controller = new AbortController()
  let timer: ReturnType<typeof setTimeout> | undefined
  let onParent: (() => void) | undefined
  const guard = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      controller.abort()
      reject(WeftError.timeout(`Timed out after ${ms} ms`))
    }, ms)
    if (parent !== undefined) {
      onParent = () => {
        controller.abort()
        reject(WeftError.unavailable("The call was cancelled"))
      }
      parent.addEventListener("abort", onParent, { once: true })
    }
  })
  try {
    return await Promise.race([fn(controller.signal), guard])
  } finally {
    clearTimeout(timer)
    if (parent !== undefined && onParent !== undefined) parent.removeEventListener("abort", onParent)
    guard.catch(() => undefined)
  }
}

/** The earlier of "`ms` from now" and an existing deadline, so a deadline only ever gets shorter. */
export function deadlineAfter(ms: number, existing?: number, now = Date.now()): number {
  const mine = now + ms
  return existing === undefined ? mine : Math.min(existing, mine)
}
