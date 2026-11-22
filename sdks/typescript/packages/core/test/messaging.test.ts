import { describe, expect, it, vi } from "vitest"
import {
  Contracts,
  MemoryAdapter,
  MemoryBroker,
  Messaging,
  WeftError,
  Weft,
  backoffMs,
  dedupe,
  fromHeaders,
  parseConfig,
  toHeaders,
  type Envelope,
  type Logger,
  type MessageContext,
} from "../src/index.js"
import { contracts } from "./fixtures/examples.gen.js"

const ID = "123e4567-e89b-42d3-a456-426614174000"
const order = {
  id: ID,
  customerId: "c1",
  items: [{ sku: "apple", qty: 2 }],
  total: { amount: 450, currency: "EUR" },
  placedAt: new Date("2026-11-23T10:00:00.000Z"),
}

const logs: string[] = []
const logger: Logger = { warn: (m) => logs.push(`warn ${m}`), error: (m) => logs.push(`error ${m}`) }

function setup(options: { yaml?: string; broker?: MemoryBroker; sleep?: (ms: number, s: AbortSignal) => Promise<void> } = {}) {
  logs.length = 0
  const weft = new Weft({
    contracts: new Contracts({ orders: contracts.orders }),
    config: parseConfig(options.yaml ?? "service: orders-svc", {}),
    shutdownGraceMs: 300,
  })
  const adapter = new MemoryAdapter("kafka", options.broker)
  const sleeps: number[] = []
  const messaging = new Messaging(weft, {
    adapter,
    logger,
    random: () => 0.5,
    sleep: options.sleep ?? (async (ms) => void sleeps.push(ms)),
  })
  return { weft, adapter, messaging, broker: adapter.raw, sleeps }
}

describe("publish and receive", () => {
  it("delivers a decoded payload with its context", async () => {
    const { weft, messaging, broker } = setup()
    const seen: { payload: unknown; ctx: MessageContext }[] = []
    messaging.on("order.placed", (payload, ctx) => void seen.push({ payload, ctx }))
    await weft.start()
    await messaging.publish("order.placed", order)
    await broker.idle()
    expect(seen).toHaveLength(1)
    expect(seen[0]?.payload).toEqual(order)
    const { ctx } = seen[0] as { ctx: MessageContext }
    expect(ctx).toMatchObject({ transport: "kafka", operation: "order.placed", event: "order.placed", key: ID, attempt: 1 })
    expect(ctx.envelope).toMatchObject({ type: "order.placed", source: "orders-svc", datacontenttype: "application/json", key: ID })
    expect(ctx.envelope.id).toMatch(/^[0-9a-f-]{36}$/)
    expect(ctx.timestamp).toBeInstanceOf(Date)
    await weft.stop()
  })

  it("sends the wire form as JSON", async () => {
    const { weft, messaging, broker } = setup()
    await weft.start()
    await messaging.publish("order.placed", order, { headers: { tenant: "t1" }, idempotencyKey: "k1" })
    const sent = broker.to("order.placed")[0]?.envelope as Envelope
    expect(JSON.parse(new TextDecoder().decode(sent.body))).toMatchObject({ id: ID, placedAt: "2026-11-23T10:00:00.000Z" })
    expect(sent.headers).toEqual({ tenant: "t1", "weft-idempotency-key": "k1" })
    await weft.stop()
  })

  it("uses the topic name from weft.yaml and the key option", async () => {
    const { weft, messaging, broker } = setup({ yaml: "service: s\ntopics:\n  order.placed: prod.orders.placed.v1" })
    await weft.start()
    await messaging.publish("order.placed", order, { key: "custom" })
    expect(broker.to("prod.orders.placed.v1")[0]?.envelope.key).toBe("custom")
    expect(broker.to("order.placed")).toHaveLength(0)
    await weft.stop()
  })

  it("has no key for an event that declares none", async () => {
    const { weft, messaging, broker } = setup()
    await weft.start()
    await messaging.publish("order.cancelled", { id: ID, reason: "x" })
    expect(broker.to("order.cancelled")[0]?.envelope.key).toBeUndefined()
    await weft.stop()
  })

  it("rejects an invalid payload before sending anything", async () => {
    const { weft, messaging, broker } = setup()
    await weft.start()
    await expect(messaging.publish("order.placed", { ...order, id: "nope" })).rejects.toMatchObject({ category: "InvalidInput" })
    expect(broker.published).toHaveLength(0)
    await weft.stop()
  })

  it("refuses to publish before start, and unknown events", async () => {
    const { weft, messaging } = setup()
    await expect(messaging.publish("order.placed", order)).rejects.toMatchObject({ category: "Unavailable" })
    await weft.start()
    await expect(messaging.publish("nope", order)).rejects.toMatchObject({ category: "Internal" })
    await weft.stop()
  })

  it("rejects options the library cannot honour", async () => {
    const { weft, messaging } = setup()
    await weft.start()
    await expect(messaging.publish("order.placed", order, { delayMs: 1000 })).rejects.toThrow(/does not support delayed delivery/)
    await weft.stop()
  })

  it("rejects a group when the library has no consumer groups", () => {
    const { messaging, adapter } = setup()
    adapter.capabilities = () => ({ ordering: true, keys: true, headers: true, transactions: false, requestReply: false, delay: false, consumerGroups: false })
    expect(() => messaging.on("order.placed", () => undefined, { group: "g" })).toThrow(/consumer groups/)
  })

  it("registers handlers only before start", async () => {
    const { weft, messaging } = setup()
    await weft.start()
    expect(() => messaging.on("order.placed", () => undefined)).toThrow(/before start/)
    await weft.stop()
  })

  it("gives each consumer group every message once", async () => {
    const broker = new MemoryBroker()
    const a = setup({ broker })
    const b = setup({ broker })
    const c = setup({ broker })
    const got: string[] = []
    a.messaging.on("order.placed", () => void got.push("a1"), { group: "payments" })
    b.messaging.on("order.placed", () => void got.push("b1"), { group: "payments" })
    c.messaging.on("order.placed", () => void got.push("c1"), { group: "emails" })
    await Promise.all([a.weft.start(), b.weft.start(), c.weft.start()])
    await a.messaging.publish("order.placed", order)
    await a.messaging.publish("order.placed", order)
    await broker.idle()
    expect(got.filter((g) => g === "c1")).toHaveLength(2)
    expect(got.filter((g) => g !== "c1")).toHaveLength(2)
    expect(new Set(got.filter((g) => g !== "c1")).size).toBe(2)
    await Promise.all([a.weft.stop(), b.weft.stop(), c.weft.stop()])
  })
})

describe("retries and dead letters", () => {
  async function run(handler: () => Promise<void> | void, options = {}, env = setup()) {
    let calls = 0
    const attempts: number[] = []
    env.messaging.on("order.placed", (_p, ctx) => {
      calls += 1
      attempts.push(ctx.attempt)
      return handler()
    }, options)
    await env.weft.start()
    await env.messaging.publish("order.placed", order)
    await env.broker.idle()
    return { ...env, calls: () => calls, attempts }
  }

  it("retries with backoff until the handler succeeds", async () => {
    let n = 0
    const r = await run(() => {
      if (++n < 3) throw WeftError.unavailable()
    })
    expect(r.attempts).toEqual([1, 2, 3])
    expect(r.sleeps).toEqual([150, 300])
    expect(r.broker.to("order.placed.dlq")).toHaveLength(0)
    await r.weft.stop()
  })

  it("dead-letters after five attempts, with the reason in headers", async () => {
    const r = await run(() => {
      throw WeftError.timeout("slow")
    })
    expect(r.calls()).toBe(5)
    expect(r.sleeps).toHaveLength(4)
    const dead = r.broker.to("order.placed.dlq")
    expect(dead).toHaveLength(1)
    expect(dead[0]?.envelope.headers).toMatchObject({
      "weft-original-destination": "order.placed",
      "weft-error-category": "Timeout",
      "weft-error-message": "slow",
      "weft-attempts": "5",
    })
    expect(dead[0]?.envelope.id).toBe(r.broker.to("order.placed")[0]?.envelope.id)
    await r.weft.stop()
  })

  it("treats an ordinary exception as Internal: retried, then dead-lettered", async () => {
    const r = await run(() => {
      throw new Error("bug")
    })
    expect(r.calls()).toBe(5)
    expect(r.broker.to("order.placed.dlq")[0]?.envelope.headers["weft-error-category"]).toBe("Internal")
    await r.weft.stop()
  })

  it("dead-letters InvalidInput at once, without retrying", async () => {
    const r = await run(() => {
      throw WeftError.invalidInput("bad")
    })
    expect(r.calls()).toBe(1)
    expect(r.sleeps).toEqual([])
    expect(r.broker.to("order.placed.dlq")).toHaveLength(1)
    await r.weft.stop()
  })

  it.each([["notFound"], ["unauthorized"], ["forbidden"], ["conflict"]] as const)("%s is neither retried nor dead-lettered", async (name) => {
    const r = await run(() => {
      throw WeftError[name]()
    })
    expect(r.calls()).toBe(1)
    expect(r.broker.to("order.placed.dlq")).toHaveLength(0)
    expect(logs.some((l) => l.startsWith("warn") && l.includes("not retried"))).toBe(true)
    await r.weft.stop()
  })

  it("honours attempts, a custom dead-letter destination, and false", async () => {
    const a = await run(() => { throw WeftError.unavailable() }, { attempts: 2, deadLetter: "failed.orders" })
    expect(a.calls()).toBe(2)
    expect(a.broker.to("failed.orders")).toHaveLength(1)
    await a.weft.stop()
    const b = await run(() => { throw WeftError.unavailable() }, { attempts: 1, deadLetter: false })
    expect(b.broker.published).toHaveLength(1)
    expect(logs.some((l) => l.startsWith("error") && l.includes("dropped"))).toBe(true)
    await b.weft.stop()
  })

  it("dead-letters a body that does not match the contract, without calling the handler", async () => {
    const env = setup()
    const handler = vi.fn()
    env.messaging.on("order.placed", handler)
    await env.weft.start()
    const bad: Envelope = { id: "1", type: "order.placed", source: "x", time: new Date().toISOString(), datacontenttype: "application/json", headers: {}, body: new TextEncoder().encode('{"id": 5}') }
    env.broker.publish("order.placed", bad, {})
    await env.broker.idle()
    expect(handler).not.toHaveBeenCalled()
    expect(env.broker.to("order.placed.dlq")[0]?.envelope.headers["weft-error-category"]).toBe("InvalidInput")
    await env.weft.stop()
  })

  it("does not acknowledge a message that is mid-retry when shutdown starts", async () => {
    const env = setup({ sleep: (_ms, signal) => new Promise((resolve) => signal.addEventListener("abort", () => resolve(), { once: true })) })
    let calls = 0
    env.messaging.on("order.placed", () => {
      calls += 1
      throw WeftError.unavailable()
    })
    await env.weft.start()
    await env.messaging.publish("order.placed", order)
    await vi.waitFor(() => expect(calls).toBe(1))
    await env.weft.stop()
    expect(env.broker.unacked).toHaveLength(1)
    expect(env.broker.to("order.placed.dlq")).toHaveLength(0)
    expect(calls).toBe(1)
  })
})

describe("publishing failures", () => {
  it("retries Unavailable with backoff", async () => {
    const { weft, messaging, adapter, broker, sleeps } = setup()
    await weft.start()
    adapter.failPublish = 2
    await messaging.publish("order.placed", order)
    expect(sleeps).toEqual([150, 300])
    expect(broker.to("order.placed")).toHaveLength(1)
    await weft.stop()
  })

  it("gives up after five attempts", async () => {
    const { weft, messaging, adapter, sleeps } = setup()
    await weft.start()
    adapter.failPublish = 99
    await expect(messaging.publish("order.placed", order)).rejects.toMatchObject({ category: "Unavailable" })
    expect(sleeps).toHaveLength(4)
    await weft.stop()
  })

  it("does not retry other errors, and wraps library errors as Unavailable", async () => {
    const { weft, messaging, adapter, sleeps } = setup()
    await weft.start()
    adapter.publish = async () => {
      throw WeftError.forbidden("no")
    }
    await expect(messaging.publish("order.placed", order)).rejects.toMatchObject({ category: "Forbidden" })
    adapter.publish = async () => {
      throw new Error("ECONNRESET")
    }
    await expect(messaging.publish("order.placed", order)).rejects.toMatchObject({ category: "Unavailable" })
    expect(sleeps).toHaveLength(4)
    await weft.stop()
  })
})

describe("middleware", () => {
  it("runs Weft's middleware, then the transport's, then the handler", async () => {
    const { weft, messaging, broker } = setup()
    const log: string[] = []
    weft.use(async (_c, next) => { log.push("weft"); await next() })
    messaging.use(async (c, next) => { log.push(`kafka ${c.event}`); await next() })
    messaging.on("order.placed", () => void log.push("handler"))
    await weft.start()
    await messaging.publish("order.placed", order)
    await broker.idle()
    expect(log).toEqual(["weft", "kafka order.placed", "handler"])
    await weft.stop()
  })

  it("dedupe skips a repeated idempotency key and retries a failure", async () => {
    const { weft, messaging, broker } = setup()
    messaging.use(dedupe())
    const handled: number[] = []
    let first = true
    messaging.on("order.placed", (_p, ctx) => {
      if (first) {
        first = false
        throw WeftError.unavailable()
      }
      handled.push(ctx.attempt)
    })
    await weft.start()
    for (const key of ["same", "same", "other"]) {
      await messaging.publish("order.placed", order, { idempotencyKey: key })
      await broker.idle()
    }
    expect(handled).toEqual([2, 1])
    await weft.stop()
  })

  it("dedupe forgets the oldest key past its limit", async () => {
    const mw = dedupe({ max: 2 })
    const run = async (id: string): Promise<boolean> => {
      let ran = false
      await mw({ envelope: { id, headers: {} } } as MessageContext, async () => void (ran = true))
      return ran
    }
    expect(await run("a")).toBe(true)
    expect(await run("a")).toBe(false)
    await run("b")
    await run("c")
    expect(await run("a")).toBe(true)
  })
})

describe("helpers", () => {
  it("backoff doubles from the minimum, caps at the maximum, and stays within half to full", () => {
    expect(backoffMs(1, {}, () => 0)).toBe(100)
    expect(backoffMs(1, {}, () => 1)).toBe(200)
    expect(backoffMs(2, {}, () => 1)).toBe(400)
    expect(backoffMs(3, {}, () => 1)).toBe(800)
    expect(backoffMs(30, {}, () => 1)).toBe(30_000)
    expect(backoffMs(30, {}, () => 0)).toBe(15_000)
    expect(backoffMs(2, { minMs: 10, maxMs: 15 }, () => 1)).toBe(15)
  })

  it("carries an envelope in CloudEvents binary-mode headers and back", () => {
    const envelope: Envelope = { id: "1", type: "order.placed", source: "svc", time: "2026-11-23T10:00:00.000Z", datacontenttype: "application/json", key: "k", headers: { tenant: "t" }, body: new Uint8Array([1]) }
    const headers = toHeaders(envelope, "ce_")
    expect(headers).toMatchObject({ ce_specversion: "1.0", ce_id: "1", ce_type: "order.placed", ce_source: "svc", ce_datacontenttype: "application/json", tenant: "t" })
    const back = fromHeaders(headers, "ce_", envelope.body, "k")
    expect(back).toEqual({ ok: true, envelope })
    expect(fromHeaders({ tenant: "t" }, "ce_", envelope.body)).toEqual({ ok: false, missing: ["ce_id", "ce_type", "ce_source", "ce_time"] })
  })
})
