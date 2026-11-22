import { createServer } from "node:net"
import { SpanKind, context, propagation, trace } from "@opentelemetry/api"
import { AsyncLocalStorageContextManager } from "@opentelemetry/context-async-hooks"
import { W3CTraceContextPropagator } from "@opentelemetry/core"
import { BasicTracerProvider, InMemorySpanExporter, SimpleSpanProcessor } from "@opentelemetry/sdk-trace-base"
import { Contracts, DeclaredError, Http, Weft, WeftError, parseConfig, withSpan } from "@weft-sdk/core"
import express from "express"
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest"
import { ExpressAdapter } from "../src/index.js"
import { contracts } from "../../core/test/fixtures/examples.gen.js"

const spans = new InMemorySpanExporter()
const provider = new BasicTracerProvider({ spanProcessors: [new SimpleSpanProcessor(spans)] })
beforeAll(() => {
  context.setGlobalContextManager(new AsyncLocalStorageContextManager().enable())
  propagation.setGlobalPropagator(new W3CTraceContextPropagator())
  trace.setGlobalTracerProvider(provider)
})
afterAll(() => provider.shutdown())

const ID = "123e4567-e89b-42d3-a456-426614174000"
const order = {
  id: ID,
  customerId: "c1",
  items: [{ sku: "apple", qty: 2 }],
  total: { amount: 450, currency: "EUR" },
  placedAt: new Date("2026-11-23T10:00:00.000Z"),
}

async function freePort(): Promise<number> {
  return new Promise((resolve) => {
    const s = createServer()
    s.listen(0, () => {
      const port = (s.address() as { port: number }).port
      s.close(() => resolve(port))
    })
  })
}

const running: Weft[] = []
afterEach(async () => {
  await Promise.all(running.splice(0).map((w) => w.stop()))
  spans.reset()
})

async function setup(options: { fetch?: typeof fetch; app?: express.Express } = {}) {
  const port = await freePort()
  const weft = new Weft({
    contracts: new Contracts({ orders: contracts.orders }),
    config: parseConfig(`service: svc\ntransports:\n  http:\n    port: ${port}\nservices:\n  self: http://127.0.0.1:${port}\n  nobody: http://127.0.0.1:1`, {}),
    shutdownGraceMs: 500,
  })
  running.push(weft)
  const adapter = new ExpressAdapter(options.app === undefined ? {} : { app: options.app })
  const quiet = { warn: () => undefined, error: () => undefined }
  const http = new Http(weft, {
    adapter,
    logger: quiet,
    sleep: async () => undefined,
    random: () => 0.5,
    ...(options.fetch === undefined ? {} : { fetch: options.fetch }),
  })
  return { weft, http, adapter, port, url: `http://127.0.0.1:${port}` }
}

const raw = async (url: string, init: RequestInit = {}) => {
  const res = await fetch(url, init)
  const text = await res.text()
  return { status: res.status, type: res.headers.get("content-type") ?? "", body: text === "" ? undefined : JSON.parse(text) }
}

describe("serving and calling", () => {
  it("serves a route and a client calls it, both from the same contract", async () => {
    const { weft, http } = await setup()
    http.post("/orders", ({ body }) => ({ ...order, customerId: (body as { customerId: string }).customerId }))
    await weft.start()
    const result = await http.to("self").post("/orders", { body: { customerId: "zed", items: [{ sku: "a", qty: 1 }] } })
    expect(result).toEqual({ ...order, customerId: "zed" })
  })

  it("gives the handler decoded params, query and body", async () => {
    const { weft, http } = await setup()
    const seen: unknown[] = []
    http.get("/orders/{id}", ({ params }) => {
      seen.push(params)
      return order
    })
    http.get("/orders", ({ query }) => {
      seen.push(query)
      return []
    })
    await weft.start()
    await http.to("self").get("/orders/{id}", { params: { id: ID } })
    await http.to("self").get("/orders", { query: { page: 2 } })
    expect(seen).toEqual([{ id: ID }, { page: 2 }])
  })

  it("matches a route by its path with or without the base", async () => {
    const { weft, http } = await setup()
    http.get("/v1/orders/{id}", () => order)
    await weft.start()
    expect(await http.to("self").get("/v1/orders/{id}", { params: { id: ID } })).toEqual(order)
  })

  it("answers a request that breaks the contract with a 400 problem body listing every issue", async () => {
    const { weft, http, url } = await setup()
    http.post("/orders", () => order)
    await weft.start()
    const r = await raw(`${url}/v1/orders`, { method: "POST", body: JSON.stringify({ customerId: 5, items: [{ sku: "a" }] }) })
    expect(r.status).toBe(400)
    expect(r.type).toMatch(/^application\/problem\+json/)
    expect(r.body).toMatchObject({ type: "urn:weft:error:InvalidInput", title: "InvalidInput", status: 400 })
    expect(r.body.issues.map((i: { path: string }) => i.path).sort()).toEqual(["body.customerId", "body.items[0].qty"])
  })

  it("rejects a missing body, bad JSON, a bad path parameter and a bad query value", async () => {
    const { weft, http, url } = await setup()
    http.post("/orders", () => order)
    http.get("/orders/{id}", () => order)
    http.get("/orders", () => [])
    await weft.start()
    const post = (body?: string) => raw(`${url}/v1/orders`, { method: "POST", ...(body === undefined ? {} : { body }) })
    expect((await post()).body.issues).toEqual([{ path: "body", message: "is required" }])
    expect((await post("{nope")).body.issues).toEqual([{ path: "body", message: "is not valid JSON" }])
    expect((await raw(`${url}/v1/orders/nope`)).body.issues[0].path).toBe("params.id")
    expect((await raw(`${url}/v1/orders?page=abc`)).body.issues[0].path).toBe("query.page")
    expect((await raw(`${url}/v1/orders?page=3`)).status).toBe(200)
  })

  it("answers an unknown path with a 404 problem", async () => {
    const { weft, http, url } = await setup()
    http.get("/orders", () => [])
    await weft.start()
    const r = await raw(`${url}/nothing`)
    expect(r.status).toBe(404)
    expect(r.body).toMatchObject({ title: "NotFound", status: 404 })
  })

  it("refuses a request body over the limit", async () => {
    const { weft, http, url } = await setup()
    http.post("/orders", () => order)
    await weft.start()
    const r = await raw(`${url}/v1/orders`, { method: "POST", body: "x".repeat(2 * 1024 * 1024) })
    expect(r.status).toBe(413)
    expect(r.type).toMatch(/problem\+json/)
  })

  it("lets a handler set the status and headers, and run middleware around it", async () => {
    const { weft, http, url } = await setup()
    const log: string[] = []
    weft.use(async (_c, next) => { log.push("weft"); await next() })
    http.use(async (c, next) => { log.push(`http ${c.operation}`); await next(); c.response.headers["x-seen"] = "yes" })
    http.post("/orders", ({ ctx }) => {
      ctx.response.status = 201
      ctx.response.headers.location = "/v1/orders/1"
      return order
    })
    await weft.start()
    const res = await fetch(`${url}/v1/orders`, { method: "POST", body: JSON.stringify({ customerId: "c", items: [] }) })
    expect(res.status).toBe(201)
    expect(res.headers.get("location")).toBe("/v1/orders/1")
    expect(res.headers.get("x-seen")).toBe("yes")
    expect(log).toEqual(["weft", "http createOrder"])
  })

  it("exposes the Express app as raw, and serves on a given app", async () => {
    const app = express()
    app.get("/health", (_req, res) => void res.json({ ok: true }))
    const { weft, http, adapter, url } = await setup({ app })
    http.get("/orders", () => [])
    await weft.start()
    expect(http.raw).toBe(app)
    expect(adapter.raw).toBe(app)
    expect((await raw(`${url}/health`)).body).toEqual({ ok: true })
    expect((await raw(`${url}/v1/orders`)).status).toBe(200)
  })

  it("refuses a handler for a route the contracts do not have, or one registered twice", async () => {
    const { http } = await setup()
    expect(() => http.get("/nope", () => undefined)).toThrow(/no route/)
    http.get("/orders", () => [])
    expect(() => http.get("/orders", () => [])).toThrow(/already has a handler/)
  })
})

describe("errors", () => {
  const table = [
    ["notFound", 404],
    ["unauthorized", 401],
    ["forbidden", 403],
    ["conflict", 409],
    ["unavailable", 503],
    ["timeout", 504],
    ["internal", 500],
    ["invalidInput", 400],
  ] as const

  it.each(table)("a thrown %s is HTTP %i, and the client raises it again", async (name, status) => {
    const { weft, http, url } = await setup()
    http.post("/orders", () => {
      throw WeftError[name]("because")
    })
    await weft.start()
    const r = await raw(`${url}/v1/orders`, { method: "POST", body: JSON.stringify({ customerId: "c", items: [] }) })
    expect(r.status).toBe(status)
    expect(r.body.status).toBe(status)
    const category = name[0]?.toUpperCase() + name.slice(1)
    expect(r.body.type).toBe(`urn:weft:error:${category}`)
    await expect(http.to("self").post("/orders", { body: { customerId: "c", items: [] }, timeoutMs: 2000 })).rejects.toMatchObject({ category })
  })

  it("does not leak the message of an internal error, or of an unexpected one", async () => {
    const { weft, http, url } = await setup()
    http.get("/orders", () => {
      throw new Error("database password is hunter2")
    })
    await weft.start()
    const r = await raw(`${url}/v1/orders`)
    expect(r.status).toBe(500)
    expect(JSON.stringify(r.body)).not.toContain("hunter2")
    expect(r.body.detail).toBe("Internal server error")
  })

  it("sends a declared error with its status and fields, and the client gets the same error back", async () => {
    const { weft, http, url } = await setup()
    http.post("/orders", () => {
      throw new DeclaredError("orders", "OutOfStock", { sku: "apple", available: 0 }, "No apples")
    })
    await weft.start()
    const r = await raw(`${url}/v1/orders`, { method: "POST", body: JSON.stringify({ customerId: "c", items: [] }) })
    expect(r.status).toBe(422)
    expect(r.body).toMatchObject({ type: "urn:weft:error:orders.OutOfStock", title: "OutOfStock", detail: "No apples", fields: { sku: "apple", available: 0 } })
    const error = await http.to("self").post("/orders", { body: { customerId: "c", items: [] } }).catch((e) => e)
    expect(error).toBeInstanceOf(DeclaredError)
    expect(error).toMatchObject({ package: "orders", errorName: "OutOfStock", fields: { sku: "apple", available: 0 } })
  })

  it("treats an undeclared declared-error as a server bug", async () => {
    const { weft, http, url } = await setup()
    http.get("/orders", () => {
      throw new DeclaredError("orders", "OutOfStock", { sku: "a", available: 1 })
    })
    await weft.start()
    expect((await raw(`${url}/v1/orders`)).status).toBe(500)
  })

  it("an optional result that is missing is a 404, and the client returns undefined", async () => {
    const { weft, http, url } = await setup()
    http.get("/orders/{id}", () => undefined)
    await weft.start()
    expect((await raw(`${url}/v1/orders/${ID}`)).status).toBe(404)
    expect(await http.to("self").get("/orders/{id}", { params: { id: ID } })).toBeUndefined()
  })

  it("`weft.http.notFound()` returns a 404", async () => {
    const { weft, http, url } = await setup()
    http.get("/orders/{id}", () => http.notFound("no such order"))
    await weft.start()
    const r = await raw(`${url}/v1/orders/${ID}`)
    expect(r.status).toBe(404)
    expect(r.body.detail).toBe("no such order")
  })

  it("a handler that returns the wrong shape is a 500, not bad data on the wire", async () => {
    const { weft, http, url } = await setup()
    http.get("/orders", () => [{ nope: true }])
    await weft.start()
    expect((await raw(`${url}/v1/orders`)).status).toBe(500)
  })
})

describe("calling other services", () => {
  it("fails with Internal when the service has no URL, and Unavailable when it cannot be reached", async () => {
    const { weft, http } = await setup()
    await weft.start()
    await expect(http.to("ghost").get("/orders")).rejects.toThrow(/no URL for the service "ghost"/)
    await expect(http.to("nobody").get("/orders")).rejects.toMatchObject({ category: "Unavailable" })
  })

  it("rejects a call that breaks the contract before sending it", async () => {
    const { weft, http } = await setup()
    await weft.start()
    await expect(http.to("self").post("/orders", { body: { customerId: 1 } })).rejects.toMatchObject({ category: "InvalidInput" })
    await expect(http.to("self").get("/orders/{id}")).rejects.toMatchObject({ category: "InvalidInput" })
  })

  it("retries a GET on Unavailable, but not a POST unless it has an idempotency key", async () => {
    const calls: string[] = []
    const spy: typeof fetch = async (input, init) => {
      calls.push(`${init?.method} ${String(input)}`)
      return real(input, init)
    }
    const real = fetch
    const { weft, http } = await setup({ fetch: spy })
    let n = 0
    http.get("/orders", () => {
      if (++n < 3) throw WeftError.unavailable("warming up")
      return []
    })
    let posts = 0
    http.post("/orders", () => {
      if (++posts < 3) throw WeftError.unavailable()
      return order
    })
    await weft.start()
    expect(await http.to("self").get("/orders")).toEqual([])
    expect(calls).toHaveLength(3)
    calls.length = 0
    await expect(http.to("self").post("/orders", { body: { customerId: "c", items: [] } })).rejects.toMatchObject({ category: "Unavailable" })
    expect(calls).toHaveLength(1)
    expect(await http.to("self").post("/orders", { body: { customerId: "c", items: [] }, idempotencyKey: "k" })).toEqual(order)
    expect(calls).toHaveLength(3)
  })

  it("gives up with Timeout, and retries a GET that times out", async () => {
    const { weft, http } = await setup()
    let n = 0
    http.get("/orders", async () => {
      n += 1
      await new Promise((r) => setTimeout(r, 400))
      return []
    })
    await weft.start()
    await expect(http.to("self").get("/orders", { timeoutMs: 30 })).rejects.toMatchObject({ category: "Timeout" })
    expect(n).toBe(3)
  })

  it("sends the deadline and the trace context, so one trace spans both services", async () => {
    const { weft, http } = await setup()
    let seen: Record<string, string> = {}
    http.get("/orders", ({ ctx }) => {
      seen = ctx.headers
      expect(ctx.deadline).toBeGreaterThan(Date.now())
      return []
    })
    await weft.start()
    let traceId = ""
    await withSpan("outer", { kind: SpanKind.INTERNAL }, async (span) => {
      traceId = span.spanContext().traceId
      await http.to("self").get("/orders")
    })
    expect(seen.traceparent).toContain(traceId)
    expect(seen["weft-deadline"]).toBeDefined()
    const finished = spans.getFinishedSpans()
    const client = finished.find((s) => s.kind === SpanKind.CLIENT)
    const server = finished.find((s) => s.kind === SpanKind.SERVER)
    expect(client?.spanContext().traceId).toBe(traceId)
    expect(server?.spanContext().traceId).toBe(traceId)
    expect(server?.parentSpanContext?.spanId).toBe(client?.spanContext().spanId)
    expect(server?.attributes).toMatchObject({ "http.route": "/v1/orders", "http.response.status_code": 200 })
  })
})

describe("shutdown", () => {
  it("finishes a request in flight, then stops listening", async () => {
    const { weft, http, url } = await setup()
    let release!: () => void
    let entered = false
    const gate = new Promise<void>((r) => (release = r))
    http.get("/orders", async () => {
      entered = true
      await gate
      return []
    })
    await weft.start()
    const inflight = raw(`${url}/v1/orders`)
    await vi.waitFor(() => expect(entered).toBe(true))
    const stopping = weft.stop()
    await new Promise((r) => setTimeout(r, 50))
    release()
    expect((await inflight).status).toBe(200)
    expect(await stopping).toEqual({ drained: true, abandoned: 0 })
    await expect(raw(`${url}/v1/orders`)).rejects.toBeDefined()
  })

  it("answers 503 for a request that arrives after stop began", async () => {
    const { weft, http } = await setup()
    http.get("/orders", () => [])
    await weft.start()
    await weft.stop()
    await expect(http.to("self").get("/orders", { timeoutMs: 500 })).rejects.toMatchObject({ category: "Unavailable" })
  })
})
