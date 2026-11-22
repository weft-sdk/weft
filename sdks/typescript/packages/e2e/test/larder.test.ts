import { randomUUID } from "node:crypto"
import { createServer } from "node:net"
import { SpanKind, context, propagation, trace } from "@opentelemetry/api"
import { AsyncLocalStorageContextManager } from "@opentelemetry/context-async-hooks"
import { W3CTraceContextPropagator } from "@opentelemetry/core"
import { BasicTracerProvider, InMemorySpanExporter, SimpleSpanProcessor } from "@opentelemetry/sdk-trace-base"
import { DeclaredError, WeftError, parseConfig, withSpan } from "@weft-sdk/core"
import { Kafka } from "kafkajs"
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest"
import { notificationsService, ordersService } from "../src/larder.js"
import { createWeft } from "../src/weft.gen.js"

// The whole stack, from the generated code down: Express, KafkaJS, a real Kafka broker. Needs one:
// WEFT_TEST_KAFKA=127.0.0.1:9092 (see `make ts-kafka-up`).
const BROKERS = process.env.WEFT_TEST_KAFKA

const spans = new InMemorySpanExporter()
const provider = new BasicTracerProvider({ spanProcessors: [new SimpleSpanProcessor(spans)] })

async function freePort(): Promise<number> {
  return new Promise((resolve) => {
    const s = createServer()
    s.listen(0, () => {
      const port = (s.address() as { port: number }).port
      s.close(() => resolve(port))
    })
  })
}

describe.skipIf(BROKERS === undefined)("Larder: orders and notifications through one contract", () => {
  beforeAll(() => {
    context.setGlobalContextManager(new AsyncLocalStorageContextManager().enable())
    propagation.setGlobalPropagator(new W3CTraceContextPropagator())
    trace.setGlobalTracerProvider(provider)
  })
  afterAll(() => provider.shutdown())

  const running: { stop(): Promise<unknown> }[] = []
  afterEach(async () => {
    await Promise.all(running.splice(0).map((w) => w.stop()))
    spans.reset()
  })

  /** Each test gets its own topics and groups, so tests never see each other's messages. */
  async function setup() {
    const id = randomUUID().slice(0, 8)
    const ordersPort = await freePort()
    const base = (service: string, port: number, group: string) =>
      [
        `service: ${service}`,
        "transports:",
        "  http:",
        `    port: ${port}`,
        "  kafka:",
        `    brokers: ${BROKERS}`,
        `    group: ${group}`,
        "topics:",
        `  order.placed: placed-${id}`,
        "services:",
        `  orders: http://127.0.0.1:${ordersPort}`,
      ].join("\n")
    const orders = ordersService(parseConfig(base("orders", ordersPort, `orders-${id}`), {}))
    const quiet = { warn: () => undefined, error: () => undefined }
    void quiet
    let failing = false
    const notifications = notificationsService(parseConfig(base("notifications", await freePort(), `notifications-${id}`), {}), { fail: () => failing })
    // The client side: another service that only calls orders.
    const api = createWeft({ config: parseConfig(base("api", await freePort(), `api-${id}`), {}) })
    running.push(orders.weft, notifications.weft, api)
    await Promise.all([orders.weft.start(), notifications.weft.start(), api.start()])
    return { id, orders, notifications, api, topic: `placed-${id}`, failing: (v: boolean) => (failing = v) }
  }

  it("places an order over HTTP and the notification arrives over Kafka", async () => {
    const { orders, notifications, api } = await setup()
    const placed = await api.http.to("orders").post("/orders", { body: { customerId: "c-1", items: [{ sku: "apple", qty: 2 }] } })

    expect(placed.customerId).toBe("c-1")
    expect(placed.total).toEqual({ amount: 300, currency: "EUR" })
    expect(placed.placedAt).toBeInstanceOf(Date)
    await vi.waitFor(() => expect(notifications.sent).toHaveLength(1), { timeout: 40_000 })
    expect(notifications.sent[0]).toEqual(placed)

    // Read it back, and ask for one that is not there.
    expect(await api.http.to("orders").get("/orders/{id}", { params: { id: placed.id } })).toEqual(placed)
    expect(await api.http.to("orders").get("/orders/{id}", { params: { id: randomUUID() as typeof placed.id } })).toBeUndefined()
    expect(await api.http.to("orders").get("/orders")).toEqual([placed])
    expect(orders.store.size).toBe(1)
  }, 90_000)

  it("sends a declared error back as the same error, and publishes nothing", async () => {
    const { notifications, api } = await setup()
    const error = await api.http.to("orders").post("/orders", { body: { customerId: "c-1", items: [{ sku: "pear", qty: 1 }] } }).catch((e) => e)
    expect(error).toBeInstanceOf(DeclaredError)
    expect(error).toMatchObject({ package: "orders", errorName: "OutOfStock", fields: { sku: "pear", available: 0 } })
    await new Promise((r) => setTimeout(r, 1500))
    expect(notifications.sent).toHaveLength(0)
  }, 60_000)

  it("refuses a request that breaks the contract, naming every problem", async () => {
    const { api } = await setup()
    const rejected = await api.http
      .to("orders")
      // Skipping the types on purpose, as a caller in another language could.
      .post("/orders", { body: { customerId: 5, items: [{ sku: "apple" }] } as never })
      .catch((e) => e)
    expect(rejected).toBeInstanceOf(WeftError)
    expect(rejected.category).toBe("InvalidInput")
  }, 60_000)

  it("follows one trace from the HTTP call to the consumer", async () => {
    const { notifications, api } = await setup()
    let traceId = ""
    await withSpan("checkout", { kind: SpanKind.SERVER }, async (span) => {
      traceId = span.spanContext().traceId
      await api.http.to("orders").post("/orders", { body: { customerId: "c-1", items: [{ sku: "apple", qty: 1 }] } })
    })
    await vi.waitFor(() => expect(notifications.sent).toHaveLength(1), { timeout: 40_000 })
    await vi.waitFor(() => expect(spans.getFinishedSpans().some((s) => s.name.endsWith(" process"))).toBe(true), { timeout: 10_000 })
    const inTrace = spans.getFinishedSpans().filter((s) => s.spanContext().traceId === traceId)
    const kinds = inTrace.map((s) => `${SpanKind[s.kind]} ${s.name.replace(/placed-[0-9a-f]+/, "placed-ID")}`)
    expect(kinds).toEqual(
      expect.arrayContaining(["CLIENT POST /v1/orders", "SERVER POST /v1/orders", "PRODUCER placed-ID publish", "CONSUMER placed-ID process"]),
    )
  }, 90_000)

  it("dead-letters an order the notifications service keeps failing on", async () => {
    const { id, topic, notifications, api, failing } = await setup()
    failing(true)
    // Every attempt fails: the default is 5 attempts with backoff, so this takes a few seconds.
    await api.http.to("orders").post("/orders", { body: { customerId: "c-1", items: [{ sku: "apple", qty: 1 }] } })

    const kafka = new Kafka({ brokers: [BROKERS as string] })
    const admin = kafka.admin()
    await admin.connect()
    await admin.createTopics({ waitForLeaders: true, topics: [{ topic: `${topic}.dlq` }] })
    await admin.disconnect()
    const consumer = kafka.consumer({ groupId: `dlq-${id}` })
    await consumer.connect()
    await consumer.subscribe({ topic: `${topic}.dlq`, fromBeginning: true })
    const dead: Record<string, string>[] = []
    await consumer.run({
      eachMessage: async ({ message }) => {
        dead.push(Object.fromEntries(Object.entries(message.headers ?? {}).map(([k, v]) => [k, String(v)])))
      },
    })
    await vi.waitFor(() => expect(dead).toHaveLength(1), { timeout: 60_000 })
    await consumer.disconnect()
    expect(dead[0]).toMatchObject({ "weft-error-category": "Internal", "weft-attempts": "5", "weft-error-message": "the mail server is down" })
    expect(notifications.sent).toHaveLength(0)
  }, 120_000)

  it("shuts down cleanly: in-flight work finishes, then connections close", async () => {
    const { orders, notifications, api } = await setup()
    await api.http.to("orders").post("/orders", { body: { customerId: "c-1", items: [{ sku: "apple", qty: 1 }] } })
    await vi.waitFor(() => expect(notifications.sent).toHaveLength(1), { timeout: 40_000 })
    const results = await Promise.all([api.stop(), orders.weft.stop(), notifications.weft.stop()])
    for (const r of results) expect(r).toEqual({ drained: true, abandoned: 0 })
    expect(orders.weft.core.state).toBe("stopped")
  }, 90_000)
})
