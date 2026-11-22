import { randomUUID } from "node:crypto"
import { Contracts, Messaging, Weft, WeftError, parseConfig, type MessageContext } from "@weft-sdk/core"
import { Kafka } from "kafkajs"
import { afterEach, describe, expect, it, vi } from "vitest"
import { KafkaJsAdapter, parseBrokers } from "../src/index.js"
import { contracts } from "../../core/test/fixtures/examples.gen.js"

const BROKERS = process.env.WEFT_TEST_KAFKA
const ID = "123e4567-e89b-42d3-a456-426614174000"
const order = {
  id: ID,
  customerId: "c1",
  items: [{ sku: "apple", qty: 2 }],
  total: { amount: 450, currency: "EUR" },
  placedAt: new Date("2026-11-23T10:00:00.000Z"),
}

describe("parseBrokers", () => {
  it("reads a list or a comma-separated string", () => {
    expect(parseBrokers("a:9092, b:9092,")).toEqual(["a:9092", "b:9092"])
    expect(parseBrokers(["a:9092"])).toEqual(["a:9092"])
    expect(parseBrokers(undefined)).toEqual([])
  })
})

describe("KafkaJsAdapter without a broker", () => {
  it("says which setting is missing", async () => {
    const weft = new Weft({ contracts: new Contracts({ orders: contracts.orders }), config: parseConfig("service: s", {}) })
    const messaging = new Messaging(weft, { adapter: new KafkaJsAdapter(), name: "kafka" })
    expect(messaging.adapter.capabilities()).toMatchObject({ keys: true, headers: true, consumerGroups: true, delay: false })
    await expect(weft.start()).rejects.toThrow(/no Kafka brokers.*transports\.kafka/)
  })

  it("rejects what Kafka cannot do", async () => {
    const weft = new Weft({ contracts: new Contracts({ orders: contracts.orders }), config: parseConfig("service: s\ntransports:\n  kafka:\n    brokers: x:1", {}) })
    const messaging = new Messaging(weft, { adapter: new KafkaJsAdapter({ kafka: new Kafka({ brokers: ["x:1"] }) }), name: "kafka" })
    ;(weft as unknown as { current: string }).current = "running"
    await expect(messaging.publish("order.placed", order, { delayMs: 5 })).rejects.toThrow(/does not support delayed delivery/)
  })
})

describe.skipIf(BROKERS === undefined)("KafkaJsAdapter against a real broker", () => {
  const running: Weft[] = []
  afterEach(async () => {
    await Promise.all(running.splice(0).map((w) => w.stop()))
  })

  /** Topics and groups are unique per test, so tests never see each other's messages. */
  function names() {
    const id = randomUUID().slice(0, 8)
    return { placed: `placed-${id}`, cancelled: `cancelled-${id}`, group: `g-${id}` }
  }

  function node(n: ReturnType<typeof names>, options: { group?: string; sleep?: (ms: number, signal: AbortSignal) => Promise<void> } = {}) {
    const yaml = [
      "service: orders-svc",
      "transports:",
      "  kafka:",
      `    brokers: ${BROKERS}`,
      `    group: ${options.group ?? n.group}`,
      "topics:",
      `  order.placed: ${n.placed}`,
      `  order.cancelled: ${n.cancelled}`,
    ].join("\n")
    const weft = new Weft({ contracts: new Contracts({ orders: contracts.orders }), config: parseConfig(yaml, {}), shutdownGraceMs: 5000 })
    running.push(weft)
    const adapter = new KafkaJsAdapter()
    const quiet = { warn: () => undefined, error: () => undefined }
    const messaging = new Messaging(weft, { adapter, name: "kafka", logger: quiet, random: () => 0.5, ...(options.sleep ? { sleep: options.sleep } : { sleep: async () => undefined }) })
    return { weft, messaging, adapter }
  }

  it("publishes and consumes through the generated contract", async () => {
    const n = names()
    const { weft, messaging } = node(n)
    const seen: { payload: unknown; ctx: MessageContext }[] = []
    messaging.on("order.placed", (payload, ctx) => void seen.push({ payload, ctx }), { fromBeginning: true })
    await weft.start()
    await messaging.publish("order.placed", order, { headers: { tenant: "t1" } })
    await vi.waitFor(() => expect(seen).toHaveLength(1), { timeout: 30_000 })
    expect(seen[0]?.payload).toEqual(order)
    const ctx = seen[0]?.ctx as MessageContext
    expect(ctx.key).toBe(ID)
    expect(ctx.envelope).toMatchObject({ type: "order.placed", source: "orders-svc", datacontenttype: "application/json" })
    expect(ctx.headers.tenant).toBe("t1")
    expect(ctx.headers.traceparent ?? "").toBeDefined()
  }, 60_000)

  it("gives each consumer group every message once", async () => {
    const n = names()
    const a = node(n)
    const b = node(n)
    const other = node(n, { group: `${n.group}-other` })
    const got: string[] = []
    a.messaging.on("order.placed", () => void got.push("a"), { fromBeginning: true })
    b.messaging.on("order.placed", () => void got.push("b"), { fromBeginning: true })
    other.messaging.on("order.placed", () => void got.push("other"), { fromBeginning: true })
    await Promise.all([a.weft.start(), b.weft.start(), other.weft.start()])
    for (let i = 0; i < 4; i++) await a.messaging.publish("order.placed", order)
    await vi.waitFor(() => expect(got.length).toBe(8), { timeout: 40_000 })
    expect(got.filter((g) => g === "other")).toHaveLength(4)
    expect(got.filter((g) => g !== "other")).toHaveLength(4)
  }, 90_000)

  it("dead-letters a message that keeps failing, with the reason in headers", async () => {
    const n = names()
    const { weft, messaging } = node(n)
    let calls = 0
    messaging.on("order.placed", () => {
      calls += 1
      throw WeftError.unavailable("down")
    }, { fromBeginning: true, attempts: 3 })
    await weft.start()
    await messaging.publish("order.placed", order)

    const kafka = new Kafka({ brokers: [BROKERS as string] })
    const admin = kafka.admin()
    await admin.connect()
    await admin.createTopics({ waitForLeaders: true, topics: [{ topic: `${n.placed}.dlq` }] })
    await admin.disconnect()
    const consumer = kafka.consumer({ groupId: `dlq-${n.group}` })
    await consumer.connect()
    await consumer.subscribe({ topic: `${n.placed}.dlq`, fromBeginning: true })
    const dead: Record<string, string>[] = []
    await consumer.run({
      eachMessage: async ({ message }) => {
        dead.push(Object.fromEntries(Object.entries(message.headers ?? {}).map(([k, v]) => [k, String(v)])))
      },
    })
    await vi.waitFor(() => expect(dead).toHaveLength(1), { timeout: 40_000 })
    await consumer.disconnect()
    expect(calls).toBe(3)
    expect(dead[0]).toMatchObject({
      "weft-original-destination": n.placed,
      "weft-error-category": "Unavailable",
      "weft-attempts": "3",
      ce_type: "order.placed",
    })
  }, 90_000)

  it("does not acknowledge a message that was being retried at shutdown: the next consumer gets it", async () => {
    const n = names()
    const waitForStop = (_ms: number, signal: AbortSignal) => new Promise<void>((resolve) => signal.addEventListener("abort", () => resolve(), { once: true }))
    const first = node(n, { sleep: waitForStop })
    let tries = 0
    first.messaging.on("order.placed", () => {
      tries += 1
      throw WeftError.unavailable()
    }, { fromBeginning: true })
    await first.weft.start()
    await first.messaging.publish("order.placed", order)
    await vi.waitFor(() => expect(tries).toBe(1), { timeout: 40_000 })
    await first.weft.stop()

    const second = node(n)
    const received: unknown[] = []
    // The group has committed nothing (the first consumer never acknowledged), so read from the start.
    second.messaging.on("order.placed", (payload) => void received.push(payload), { fromBeginning: true })
    await second.weft.start()
    await vi.waitFor(() => expect(received).toHaveLength(1), { timeout: 40_000 })
    expect(received[0]).toEqual(order)
  }, 120_000)

  it("exposes the KafkaJS instance as raw", async () => {
    const n = names()
    const { weft, messaging } = node(n)
    await weft.start()
    expect(messaging.raw).toBeInstanceOf(Kafka)
  }, 30_000)
})
