import {
  SpanKind,
  SpanStatusCode,
  context,
  metrics,
  propagation,
  trace,
  type Attributes,
  type Context as OtelContext,
  type Counter,
  type Histogram,
  type Span,
  type TextMapGetter,
  type TextMapSetter,
} from "@opentelemetry/api"
import { isWeftError } from "./errors.js"
import { version } from "./version.js"

/*
 * Weft emits OpenTelemetry through the API only. With no SDK installed every call here does nothing;
 * install and configure an SDK (and an exporter) in the application to see the spans and metrics.
 */

const NAME = "@weft-sdk/core"

export function tracer() {
  return trace.getTracer(NAME, version)
}

export function meter() {
  return metrics.getMeter(NAME, version)
}

const setter: TextMapSetter<Record<string, string>> = {
  set(carrier, key, value) {
    carrier[key] = value
  },
}

const getter: TextMapGetter<Readonly<Record<string, string>>> = {
  keys: (carrier) => Object.keys(carrier),
  get: (carrier, key) => carrier[key],
}

/** Writes the active trace context into `headers` as `traceparent` (and `tracestate`), W3C Trace Context. */
export function injectTraceContext(headers: Record<string, string>, from: OtelContext = context.active()): void {
  propagation.inject(from, headers, setter)
}

/** The trace context carried in `headers`, on top of the active one. Use it as the parent of a span. */
export function extractTraceContext(headers: Readonly<Record<string, string>>): OtelContext {
  return propagation.extract(context.active(), headers, getter)
}

export interface SpanOptions {
  readonly kind: SpanKind
  readonly attributes?: Attributes
  /** The parent context. Default: the active one. */
  readonly parent?: OtelContext
}

/** Marks a span as failed and records why. */
export function recordError(span: Span, error: unknown): void {
  span.recordException(error instanceof Error ? error : String(error))
  if (isWeftError(error)) span.setAttribute("weft.error.category", error.category)
  span.setStatus({ code: SpanStatusCode.ERROR, message: error instanceof Error ? error.message : String(error) })
}

/**
 * Runs `fn` inside a new span that is the active one, ends it afterwards, and records a thrown
 * error on it before rethrowing.
 */
export async function withSpan<T>(name: string, options: SpanOptions, fn: (span: Span) => Promise<T>): Promise<T> {
  const parent = options.parent ?? context.active()
  const span = tracer().startSpan(name, { kind: options.kind, ...(options.attributes ? { attributes: options.attributes } : {}) }, parent)
  try {
    return await context.with(trace.setSpan(parent, span), () => fn(span))
  } catch (error) {
    recordError(span, error)
    throw error
  } finally {
    span.end()
  }
}

export { SpanKind, SpanStatusCode }

interface Instruments {
  readonly sent: Counter
  readonly received: Counter
  readonly errors: Counter
  readonly duration: Histogram
  readonly size: Histogram
}

let instruments: Instruments | undefined

/** The metrics Weft records (spec/behaviour.md › Observability). Created once, on first use. */
export function messagingMetrics(): Instruments {
  instruments ??= {
    sent: meter().createCounter("weft.messaging.messages.sent", { description: "Messages sent, by event and outcome" }),
    received: meter().createCounter("weft.messaging.messages.received", {
      description: "Messages received, by event and outcome (ok, retry, dead_letter, dropped, not_acked)",
    }),
    errors: meter().createCounter("weft.messaging.errors", { description: "Failures, by event and error category" }),
    duration: meter().createHistogram("weft.messaging.process.duration", {
      unit: "ms",
      description: "Time a handler took, per attempt",
    }),
    size: meter().createHistogram("weft.messaging.message.size", { unit: "By", description: "Size of a message body" }),
  }
  return instruments
}

interface HttpInstruments {
  readonly serverDuration: Histogram
  readonly clientDuration: Histogram
}

let httpInstruments: HttpInstruments | undefined

/** The HTTP metrics (spec/behaviour.md › Observability). Created once, on first use. */
export function httpMetrics(): HttpInstruments {
  httpInstruments ??= {
    serverDuration: meter().createHistogram("weft.http.server.duration", { unit: "ms", description: "Time to answer a request, by method, route and status" }),
    clientDuration: meter().createHistogram("weft.http.client.duration", { unit: "ms", description: "Time for a call to another service, including retries" }),
  }
  return httpInstruments
}
