import { SpanKind, SpanStatusCode, context, metrics, propagation, trace } from "@opentelemetry/api"
import { AsyncLocalStorageContextManager } from "@opentelemetry/context-async-hooks"
import { W3CTraceContextPropagator } from "@opentelemetry/core"
import { InMemoryMetricExporter, AggregationTemporality, MeterProvider, PeriodicExportingMetricReader } from "@opentelemetry/sdk-metrics"
import { BasicTracerProvider, InMemorySpanExporter, SimpleSpanProcessor } from "@opentelemetry/sdk-trace-base"
import { afterAll, beforeAll, beforeEach, describe, expect, it } from "vitest"
import {
  Contracts,
  MemoryAdapter,
  Messaging,
  Weft,
  WeftError,
  extractTraceContext,
  injectTraceContext,
  parseConfig,
  withSpan,
} from "../src/index.js"
import { contracts } from "./fixtures/examples.gen.js"

const spans = new InMemorySpanExporter()
const metricExporter = new InMemoryMetricExporter(AggregationTemporality.CUMULATIVE)
const reader = new PeriodicExportingMetricReader({ exporter: metricExporter, exportIntervalMillis: 3_600_000 })
const tracerProvider = new BasicTracerProvider({ spanProcessors: [new SimpleSpanProcessor(spans)] })
const meterProvider = new MeterProvider({ readers: [reader] })

beforeAll(() => {
  context.setGlobalContextManager(new AsyncLocalStorageContextManager().enable())
  propagation.setGlobalPropagator(new W3CTraceContextPropagator())
  trace.setGlobalTracerProvider(tracerProvider)
  metrics.setGlobalMeterProvider(meterProvider)
})
afterAll(async () => {
  await tracerProvider.shutdown()
  await meterProvider.shutdown()
})
beforeEach(() => {
  spans.reset()
})

const ID = "123e4567-e89b-42d3-a456-426614174000"
const order = {
  id: ID,
  customerId: "c1",
  items: [],
  total: { amount: 1, currency: "EUR" },
  placedAt: new Date("2026-11-23T10:00:00.000Z"),
}

function setup() {
  const weft = new Weft({
    contracts: new Contracts({ orders: contracts.orders }),
    config: parseConfig("service: svc", {}),
    shutdownGraceMs: 200,
  })
  const adapter = new MemoryAdapter("kafka")
  const quiet = { warn: () => undefined, error: () => undefined }
  const messaging = new Messaging(weft, { adapter, logger: quiet, sleep: async () => undefined, random: () => 0.5 })
  return { weft, messaging, broker: adapter.raw }
}

async function metricValue(name: string): Promise<{ value: number; attributes: Record<string, unknown> }[]> {
  await reader.forceFlush()
  const out: { value: number; attributes: Record<string, unknown> }[] = []
  for (const rm of metricExporter.getMetrics()) {
    for (const sm of rm.scopeMetrics) {
      for (const m of sm.metrics) {
        if (m.descriptor.name !== name) continue
        for (const dp of m.dataPoints) {
          const v = dp.value as number | { count: number }
          out.push({ value: typeof v === "number" ? v : v.count, attributes: dp.attributes })
        }
      }
    }
  }
  return out
}

describe("tracing", () => {
  it("puts a publish and its consume in one trace, the consumer a child of the publisher", async () => {
    const { weft, messaging, broker } = setup()
    messaging.on("order.placed", () => undefined)
    await weft.start()
    await messaging.publish("order.placed", order)
    await broker.idle()
    await weft.stop()

    const finished = spans.getFinishedSpans()
    const publish = finished.find((s) => s.name === "order.placed publish")
    const process = finished.find((s) => s.name === "order.placed process")
    expect(publish?.kind).toBe(SpanKind.PRODUCER)
    expect(process?.kind).toBe(SpanKind.CONSUMER)
    expect(process?.spanContext().traceId).toBe(publish?.spanContext().traceId)
    expect(process?.parentSpanContext?.spanId).toBe(publish?.spanContext().spanId)
    expect(publish?.attributes).toMatchObject({
      "messaging.system": "kafka",
      "messaging.destination.name": "order.placed",
      "messaging.operation.type": "publish",
      "weft.event": "order.placed",
    })
    expect(process?.attributes).toMatchObject({ "messaging.operation.type": "process", "weft.attempt": 1 })
    expect(process?.attributes["messaging.message.id"]).toBe(publish?.attributes["messaging.message.id"])
  })

  it("continues the caller's trace", async () => {
    const { weft, messaging, broker } = setup()
    messaging.on("order.placed", () => undefined)
    await weft.start()
    let rootTrace = ""
    await withSpan("request", { kind: SpanKind.SERVER }, async (span) => {
      rootTrace = span.spanContext().traceId
      await messaging.publish("order.placed", order)
    })
    await broker.idle()
    await weft.stop()
    const finished = spans.getFinishedSpans()
    expect(new Set(finished.map((s) => s.spanContext().traceId))).toEqual(new Set([rootTrace]))
  })

  it("writes a span per attempt and marks failures", async () => {
    const { weft, messaging, broker } = setup()
    let n = 0
    messaging.on("order.placed", () => {
      if (++n < 3) throw WeftError.unavailable("down")
    })
    await weft.start()
    await messaging.publish("order.placed", order)
    await broker.idle()
    await weft.stop()
    const attempts = spans.getFinishedSpans().filter((s) => s.name === "order.placed process")
    expect(attempts.map((s) => s.attributes["weft.attempt"])).toEqual([1, 2, 3])
    expect(attempts.map((s) => s.status.code)).toEqual([SpanStatusCode.ERROR, SpanStatusCode.ERROR, SpanStatusCode.UNSET])
    expect(attempts[0]?.attributes["weft.error.category"]).toBe("Unavailable")
    expect(attempts[0]?.events.some((e) => e.name === "exception")).toBe(true)
    expect(new Set(attempts.map((s) => s.spanContext().traceId)).size).toBe(1)
  })

  it("keeps the original trace on a dead letter", async () => {
    const { weft, messaging, broker } = setup()
    messaging.on("order.placed", () => {
      throw WeftError.invalidInput("bad")
    })
    await weft.start()
    await messaging.publish("order.placed", order)
    await broker.idle()
    await weft.stop()
    const original = broker.to("order.placed")[0]?.envelope.headers.traceparent
    const dead = broker.to("order.placed.dlq")[0]?.envelope.headers.traceparent
    expect(original).toMatch(/^00-[0-9a-f]{32}-[0-9a-f]{16}-0[01]$/)
    expect(dead).toBe(original)
    const dlqSpan = spans.getFinishedSpans().find((s) => s.name === "order.placed.dlq publish")
    expect(dlqSpan?.spanContext().traceId).toBe(original?.split("-")[1])
  })

  it("injects and extracts W3C trace context for any carrier", async () => {
    await withSpan("outer", { kind: SpanKind.INTERNAL }, async (span) => {
      const headers: Record<string, string> = {}
      injectTraceContext(headers)
      expect(headers.traceparent).toBe(`00-${span.spanContext().traceId}-${span.spanContext().spanId}-01`)
      const extracted = trace.getSpanContext(extractTraceContext(headers))
      expect(extracted).toMatchObject({ traceId: span.spanContext().traceId, spanId: span.spanContext().spanId, isRemote: true })
    })
    expect(trace.getSpanContext(extractTraceContext({}))).toBeUndefined()
  })
})

describe("metrics", () => {
  it("counts sends and receives, errors by category, and records durations and sizes", async () => {
    const { weft, messaging, broker } = setup()
    let n = 0
    messaging.on("order.placed", () => {
      if (++n === 1) throw WeftError.timeout()
    })
    await weft.start()
    await messaging.publish("order.placed", order)
    await broker.idle()
    await weft.stop()

    const total = (rows: { value: number; attributes: Record<string, unknown> }[], where: Record<string, unknown>) =>
      rows.filter((r) => Object.entries(where).every(([k, v]) => r.attributes[k] === v)).reduce((a, r) => a + r.value, 0)

    const sent = await metricValue("weft.messaging.messages.sent")
    expect(total(sent, { event: "order.placed", transport: "kafka", outcome: "ok" })).toBeGreaterThanOrEqual(1)
    const received = await metricValue("weft.messaging.messages.received")
    expect(total(received, { outcome: "retry" })).toBeGreaterThanOrEqual(1)
    expect(total(received, { outcome: "ok" })).toBeGreaterThanOrEqual(1)
    const errors = await metricValue("weft.messaging.errors")
    expect(total(errors, { category: "Timeout" })).toBeGreaterThanOrEqual(1)
    expect(total(await metricValue("weft.messaging.process.duration"), { event: "order.placed" })).toBeGreaterThanOrEqual(2)
    expect(total(await metricValue("weft.messaging.message.size"), { direction: "send" })).toBeGreaterThanOrEqual(1)
  })
})
