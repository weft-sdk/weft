import { describe, expect, it, vi } from "vitest"
import { WeftError, compose, deadlineAfter, remainingMs, withTimeout, type Context, type Middleware } from "../src/index.js"

function ctx(extra: Partial<Context> = {}): Context {
  return { transport: "test", operation: "op", signal: new AbortController().signal, headers: {}, state: {}, ...extra }
}

describe("compose", () => {
  it("runs middleware in order around the handler", async () => {
    const log: string[] = []
    const mw = (name: string): Middleware => async (_c, next) => {
      log.push(`${name} in`)
      await next()
      log.push(`${name} out`)
    }
    await compose([mw("a"), mw("b")])(ctx(), () => {
      log.push("handler")
    })
    expect(log).toEqual(["a in", "b in", "handler", "b out", "a out"])
  })

  it("stops when a middleware does not call next", async () => {
    const handler = vi.fn()
    await compose<Context>([() => undefined])(ctx(), handler)
    expect(handler).not.toHaveBeenCalled()
  })

  it("passes errors up through the chain", async () => {
    const seen: string[] = []
    const catcher: Middleware = async (_c, next) => {
      try {
        await next()
      } catch (e) {
        seen.push((e as Error).message)
      }
    }
    await compose([catcher])(ctx(), () => {
      throw new Error("boom")
    })
    expect(seen).toEqual(["boom"])
  })

  it("lets middleware share values through state", async () => {
    const c = ctx()
    await compose<Context>([async (x, next) => { x.state.user = "u1"; await next() }])(c, (x) => {
      expect(x.state.user).toBe("u1")
    })
  })

  it("refuses next() called twice", async () => {
    const twice: Middleware = async (_c, next) => {
      await next()
      await next()
    }
    await expect(compose([twice])(ctx(), () => undefined)).rejects.toThrow(/more than once/)
  })

  it("can run the same chain again", async () => {
    const run = compose<Context>([])
    const handler = vi.fn()
    await run(ctx(), handler)
    await run(ctx(), handler)
    expect(handler).toHaveBeenCalledTimes(2)
  })
})

describe("deadlines and timeouts", () => {
  it("computes remaining time and never goes negative", () => {
    expect(remainingMs({ deadline: 1500 }, 1000)).toBe(500)
    expect(remainingMs({ deadline: 500 }, 1000)).toBe(0)
    expect(remainingMs({}, 1000)).toBeUndefined()
  })

  it("only ever shortens a deadline", () => {
    expect(deadlineAfter(1000, undefined, 0)).toBe(1000)
    expect(deadlineAfter(1000, 400, 0)).toBe(400)
    expect(deadlineAfter(1000, 5000, 0)).toBe(1000)
  })

  it("returns the result when it is in time", async () => {
    await expect(withTimeout(1000, async () => 7)).resolves.toBe(7)
  })

  it("fails with Timeout and aborts the signal when time runs out", async () => {
    let signal: AbortSignal | undefined
    const slow = withTimeout(20, (s) => {
      signal = s
      return new Promise<never>(() => undefined)
    })
    await expect(slow).rejects.toMatchObject({ category: "Timeout" })
    expect(signal?.aborted).toBe(true)
  })

  it("fails with Unavailable when the parent aborts", async () => {
    const parent = new AbortController()
    const run = withTimeout(5000, () => new Promise<never>(() => undefined), parent.signal)
    parent.abort()
    await expect(run).rejects.toMatchObject({ category: "Unavailable" })
    await expect(withTimeout(5000, async () => 1, parent.signal)).rejects.toBeInstanceOf(WeftError)
  })

  it("passes the function's own error through", async () => {
    await expect(withTimeout(1000, async () => { throw WeftError.notFound() })).rejects.toMatchObject({ category: "NotFound" })
  })
})
