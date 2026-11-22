import { describe, expect, it } from "vitest"
import { Contracts, Weft, parseConfig, type Context, type Transport } from "../src/index.js"
import { contracts } from "./fixtures/examples.gen.js"

const config = parseConfig("service: test", {})

function ctx(): Context {
  return { transport: "fake", operation: "op", signal: new AbortController().signal, headers: {}, state: {} }
}

class Fake implements Transport {
  readonly events: string[]
  constructor(readonly name: string, events: string[], private readonly failStart = false) {
    this.events = events
  }
  async start() {
    this.events.push(`${this.name} start`)
    if (this.failStart) throw new Error("no broker")
  }
  async stopIntake() {
    this.events.push(`${this.name} stopIntake`)
  }
  async close() {
    this.events.push(`${this.name} close`)
  }
}

const make = (graceMs = 1000) => new Weft({ contracts: new Contracts(contracts), config, shutdownGraceMs: graceMs })
const deferred = () => {
  let resolve!: () => void
  const promise = new Promise<void>((r) => (resolve = r))
  return { promise, resolve }
}

describe("Weft lifecycle", () => {
  it("starts transports in order and stops them in reverse", async () => {
    const events: string[] = []
    const weft = make().register(new Fake("http", events)).register(new Fake("kafka", events))
    await weft.start()
    expect(weft.state).toBe("running")
    await weft.stop()
    expect(weft.state).toBe("stopped")
    expect(events).toEqual(["http start", "kafka start", "kafka stopIntake", "http stopIntake", "kafka close", "http close"])
  })

  it("runs handlers registered before start through the middleware", async () => {
    const weft = make()
    const log: string[] = []
    weft.use(async (_c, next) => {
      log.push("global")
      await next()
    })
    await weft.start()
    await weft.dispatch(ctx(), () => { log.push("handler") }, [async (_c, next) => { log.push("transport"); await next() }])
    expect(log).toEqual(["global", "transport", "handler"])
    await weft.stop()
  })

  it("waits for in-flight handlers before closing, and refuses new work once stopping", async () => {
    const events: string[] = []
    const weft = make().register(new Fake("http", events))
    await weft.start()
    const gate = deferred()
    const handler = weft.dispatch(ctx(), async () => {
      await gate.promise
      events.push("handler done")
    })
    const stopping = weft.stop()
    await expect(weft.dispatch(ctx(), () => undefined)).rejects.toMatchObject({ category: "Unavailable" })
    await new Promise((r) => setTimeout(r, 20))
    expect(events).not.toContain("http close")
    gate.resolve()
    await handler
    await expect(stopping).resolves.toEqual({ drained: true, abandoned: 0 })
    expect(events.slice(-2)).toEqual(["handler done", "http close"])
  })

  it("gives up after the grace period, aborts the shutdown signal, and still closes", async () => {
    const events: string[] = []
    const weft = make(30).register(new Fake("http", events))
    await weft.start()
    void weft.dispatch(ctx(), () => new Promise<void>(() => undefined)).catch(() => undefined)
    const result = await weft.stop()
    expect(result).toEqual({ drained: false, abandoned: 1 })
    expect(weft.shutdownSignal.aborted).toBe(true)
    expect(events).toContain("http close")
  })

  it("counts a failing handler as finished", async () => {
    const weft = make()
    await weft.start()
    await expect(weft.dispatch(ctx(), () => { throw new Error("x") })).rejects.toThrow("x")
    await expect(weft.stop()).resolves.toEqual({ drained: true, abandoned: 0 })
  })

  it("closes what already started when a transport fails to start", async () => {
    const events: string[] = []
    const weft = make().register(new Fake("http", events)).register(new Fake("kafka", events, true))
    await expect(weft.start()).rejects.toMatchObject({ category: "Unavailable", message: expect.stringContaining("kafka: no broker") })
    expect(events).toEqual(["http start", "kafka start", "http stopIntake", "http close"])
    expect(weft.state).toBe("stopped")
  })

  it("keeps closing the others when one transport fails to close", async () => {
    const events: string[] = []
    const bad = new Fake("kafka", events)
    bad.close = async () => { throw new Error("stuck") }
    const weft = make().register(new Fake("http", events)).register(bad)
    await weft.start()
    await weft.stop()
    expect(events).toContain("http close")
  })

  it("stop is safe to call twice, or before start", async () => {
    const weft = make()
    await expect(weft.stop()).resolves.toEqual({ drained: true, abandoned: 0 })
    const second = make()
    await second.start()
    const [a, b] = await Promise.all([second.stop(), second.stop()])
    expect(a).toBe(b)
  })

  it("refuses to start twice, or to register after start, or to register a name twice", async () => {
    const weft = make().register(new Fake("http", []))
    expect(() => weft.register(new Fake("http", []))).toThrow(/already registered/)
    await weft.start()
    await expect(weft.start()).rejects.toThrow(/Cannot start/)
    expect(() => weft.register(new Fake("kafka", []))).toThrow(/already started/)
    await weft.stop()
  })
})
