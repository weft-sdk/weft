/**
 * The `service.*` operations: a real service, started inside the runner with its language's package,
 * on a real HTTP socket and a real Kafka broker, so the harness can make two languages talk.
 */
import { SpanKind, context, propagation, trace, type Span } from "@opentelemetry/api"
import { AsyncLocalStorageContextManager } from "@opentelemetry/context-async-hooks"
import { W3CTraceContextPropagator } from "@opentelemetry/core"
import { BasicTracerProvider, InMemorySpanExporter, SimpleSpanProcessor, type ReadableSpan } from "@opentelemetry/sdk-trace-base"
import {
  Contracts,
  DeclaredError,
  Http,
  Messaging,
  Weft,
  WeftError,
  fullPath,
  isWeftError,
  parseConfig,
  withSpan,
  type ErrorCategory,
} from "@weft-sdk/core"
import { ExpressAdapter } from "@weft-sdk/http-express"
import { encodeInput } from "./http.js"
import { KafkaJsAdapter } from "@weft-sdk/kafka-kafkajs"

const exporter = new InMemorySpanExporter()
let tracing = false

function installTracing(): void {
  if (tracing) return
  tracing = true
  context.setGlobalContextManager(new AsyncLocalStorageContextManager().enable())
  propagation.setGlobalPropagator(new W3CTraceContextPropagator())
  trace.setGlobalTracerProvider(new BasicTracerProvider({ spanProcessors: [new SimpleSpanProcessor(exporter)] }))
}

interface Script {
  returns?: unknown
  returnsRaw?: unknown
  throws?: { category?: string; declared?: { package?: string; name: string; fields: unknown; message?: string } }
  status?: number
  headers?: Record<string, string>
}

interface StartArgs {
  svc: string
  service: string
  kafka?: { brokers: string; group: string; topics?: Record<string, string> }
  consumes?: { event: string; fromBeginning?: boolean; attempts?: number; fail?: { times: number | "always"; category: string } }[]
  serves?: { route: string; handler: Script }[]
  services?: Record<string, string>
}

interface Running {
  weft: Weft
  http: Http
  adapter: ExpressAdapter
  messaging?: Messaging
  received: Record<string, unknown>[]
  served: Record<string, unknown>[]
}

const running = new Map<string, Running>()
const quiet = { warn: () => undefined, error: () => undefined }

function yamlFor(a: StartArgs): string {
  const lines = [`service: ${a.service}`, "transports:", "  http:", "    port: 0"]
  if (a.kafka) lines.push("  kafka:", `    brokers: ${a.kafka.brokers}`, `    group: ${a.kafka.group}`)
  if (a.kafka?.topics) lines.push("topics:", ...Object.entries(a.kafka.topics).map(([e, t]) => `  ${e}: ${t}`))
  if (a.services) lines.push("services:", ...Object.entries(a.services).map(([n, u]) => `  ${n}: ${u}`))
  return lines.join("\n")
}

function thrown(c: Contracts, pkg: string, t: NonNullable<Script["throws"]>): Error {
  if (t.declared !== undefined) {
    const d = t.declared
    const fields = c.decodeError(d.package ?? pkg, d.name, d.fields) as Record<string, unknown>
    return new DeclaredError(d.package ?? pkg, d.name, fields, d.message)
  }
  return new WeftError(t.category as ErrorCategory, `failed with ${t.category}`)
}

export async function start(c: Contracts, a: StartArgs): Promise<Record<string, unknown>> {
  installTracing()
  const weft = new Weft({ contracts: c, config: parseConfig(yamlFor(a), {}), shutdownGraceMs: 3000 })
  const adapter = new ExpressAdapter()
  const http = new Http(weft, { adapter, logger: quiet })
  const svc: Running = { weft, http, adapter, received: [], served: [] }

  for (const s of a.serves ?? []) {
    const { package: pkg, def: route } = c.route(s.route)
    http.route(route.method, fullPath(route), (input) => {
      const span = trace.getActiveSpan()
      svc.served.push({
        route: route.name,
        traceId: span?.spanContext().traceId ?? null,
        hasDeadline: input.ctx.headers["weft-deadline"] !== undefined,
        hasTraceparent: input.ctx.headers.traceparent !== undefined,
        seen: encodeInput(c, route, input as unknown as Record<string, unknown>),
      })
      if (s.handler.status !== undefined) input.ctx.response.status = s.handler.status
      Object.assign(input.ctx.response.headers, s.handler.headers ?? {})
      if (s.handler.throws !== undefined) throw thrown(c, pkg, s.handler.throws)
      if (s.handler.returnsRaw !== undefined) return s.handler.returnsRaw
      if (s.handler.returns === undefined || s.handler.returns === null) return undefined
      return c.decode(route.result.value, s.handler.returns)
    }, { package: pkg })
  }

  if (a.kafka !== undefined) {
    svc.messaging = new Messaging(weft, { adapter: new KafkaJsAdapter(), name: "kafka", logger: quiet })
    for (const sub of a.consumes ?? []) {
      const { def } = c.event(sub.event)
      let failures = 0
      svc.messaging.on(
        sub.event,
        (payload, ctx) => {
          const active = trace.getActiveSpan() as (Span & Partial<ReadableSpan>) | undefined
          if (sub.fail !== undefined && (sub.fail.times === "always" || failures < sub.fail.times)) {
            failures += 1
            throw new WeftError(sub.fail.category as ErrorCategory, `failed with ${sub.fail.category}`)
          }
          svc.received.push({
            payload: c.encode(def.payload, payload),
            key: ctx.key ?? null,
            attempt: ctx.attempt,
            envelope: {
              type: ctx.envelope.type,
              source: ctx.envelope.source,
              datacontenttype: ctx.envelope.datacontenttype,
              hasId: ctx.envelope.id.length > 0,
              hasTime: !Number.isNaN(Date.parse(ctx.envelope.time)),
            },
            headers: ctx.envelope.headers,
            traceId: active?.spanContext().traceId ?? null,
            parentSpanId: active?.parentSpanContext?.spanId ?? null,
          })
        },
        { fromBeginning: sub.fromBeginning ?? true, ...(sub.attempts === undefined ? {} : { attempts: sub.attempts }) },
      )
    }
  }
  await weft.start()
  running.set(a.svc, svc)
  return { httpPort: adapter.port ?? null }
}

function service(svc: string): Running {
  const s = running.get(svc)
  if (s === undefined) throw new Error(`no service ${svc}: send service.start first`)
  return s
}

export async function publish(c: Contracts, a: { svc: string; event: string; payload: unknown; key?: string; headers?: Record<string, string>; inSpan?: boolean }) {
  const s = service(a.svc)
  const { def } = c.event(a.event)
  const value = c.decode(def.payload, a.payload)
  let traceId: string | null = null
  const send = async () => {
    try {
      await s.messaging!.publish(a.event, value, { ...(a.key === undefined ? {} : { key: a.key }), ...(a.headers === undefined ? {} : { headers: a.headers }) })
      return { ok: true }
    } catch (error) {
      return { ok: false, error: describe(error) }
    }
  }
  const result = a.inSpan
    ? await withSpan("test", { kind: SpanKind.INTERNAL }, async (span) => {
        traceId = span.spanContext().traceId
        return send()
      })
    : await send()
  return { ...result, traceId }
}

function describe(error: unknown): Record<string, unknown> {
  if (error instanceof DeclaredError) return { declared: { package: error.package, name: error.errorName, fields: error.fields }, message: error.message }
  if (isWeftError(error)) return { category: error.category, message: error.message, issues: error.issues.map((i) => i.path).sort() }
  return { category: "Internal", message: String(error) }
}

export async function call(
  c: Contracts,
  a: { svc: string; service: string; route: string; request?: { params?: Record<string, unknown>; query?: unknown; headers?: unknown; body?: unknown }; inSpan?: boolean; timeoutMs?: number; idempotencyKey?: string },
) {
  const s = service(a.svc)
  const { package: pkg, def: route } = c.route(a.route)
  const r = a.request ?? {}
  const options = {
    package: pkg,
    ...(route.body === undefined ? {} : { body: c.decode(route.body, r.body) }),
    ...(route.pathParams === undefined ? {} : { params: Object.fromEntries(route.pathParams.map((p) => [p.name, c.decode(p.type, r.params?.[p.name])])) }),
    ...(route.query === undefined ? {} : { query: c.decode(route.query, r.query ?? {}) as Record<string, unknown> }),
    ...(route.headers === undefined ? {} : { headers: c.decode(route.headers, r.headers ?? {}) as Record<string, unknown> }),
    ...(a.timeoutMs === undefined ? {} : { timeoutMs: a.timeoutMs }),
    ...(a.idempotencyKey === undefined ? {} : { idempotencyKey: a.idempotencyKey }),
  }
  const client = s.http.to(a.service)
  const verb = route.method.toLowerCase() as "get" | "post" | "put" | "patch" | "delete"
  let traceId: string | null = null
  const send = async () => {
    try {
      const result = await client[verb](fullPath(route), options as never)
      return { ok: true, result: result === undefined ? null : c.encode(route.result.value, result) }
    } catch (error) {
      if (error instanceof DeclaredError) {
        return { ok: false, error: { declared: { package: error.package, name: error.errorName, fields: c.encodeError(error.package, error.errorName, error.fields) }, message: error.message } }
      }
      return { ok: false, error: describe(error) }
    }
  }
  const result = a.inSpan
    ? await withSpan("test", { kind: SpanKind.INTERNAL }, async (span) => {
        traceId = span.spanContext().traceId
        return send()
      })
    : await send()
  return { ...result, traceId }
}

export async function received(a: { svc: string; count: number; timeoutMs?: number }) {
  const s = service(a.svc)
  const until = Date.now() + (a.timeoutMs ?? 30_000)
  while (s.received.length < a.count && Date.now() < until) await new Promise((r) => setTimeout(r, 25))
  return { count: s.received.length, messages: s.received }
}

export function served(a: { svc: string }) {
  return { calls: service(a.svc).served.length, requests: service(a.svc).served }
}

export function spans() {
  return {
    spans: exporter.getFinishedSpans().map((s) => ({
      kind: SpanKind[s.kind],
      name: s.name,
      traceId: s.spanContext().traceId,
      spanId: s.spanContext().spanId,
      parentSpanId: s.parentSpanContext?.spanId ?? null,
    })),
  }
}

export async function stop(a: { svc: string }) {
  const s = service(a.svc)
  const result = await s.weft.stop()
  running.delete(a.svc)
  return { ...result }
}
