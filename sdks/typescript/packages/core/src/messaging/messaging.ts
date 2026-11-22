import { topicName } from "../config.js"
import { DEFAULTS } from "../defaults.js"
import { WeftError, isWeftError } from "../errors.js"
import { SpanKind, extractTraceContext, injectTraceContext, messagingMetrics, withSpan } from "../telemetry.js"
import type { Middleware } from "../middleware.js"
import type { Transport, Weft } from "../weft.js"
import { backoffMs, sleep as realSleep } from "./backoff.js"
import { IDEMPOTENCY_HEADER } from "./dedupe.js"
import type {
  Delivery,
  Envelope,
  MessageContext,
  MessageHandler,
  MessagingAdapter,
  PublishOptions,
  SubscribeOptions,
} from "./types.js"

export interface Logger {
  warn(message: string, fields?: Record<string, unknown>): void
  error(message: string, fields?: Record<string, unknown>): void
}

export const consoleLogger: Logger = {
  warn: (message, fields) => console.warn(message, fields ?? ""),
  error: (message, fields) => console.error(message, fields ?? ""),
}

export interface MessagingOptions {
  readonly adapter: MessagingAdapter
  /** The name of this transport in `weft.yaml` and in contexts, such as `kafka`. Defaults to the adapter's. */
  readonly name?: string
  readonly logger?: Logger
  /** For tests: how to wait between attempts. */
  readonly sleep?: (ms: number, signal: AbortSignal) => Promise<void>
  /** For tests: the source of randomness for backoff jitter. */
  readonly random?: () => number
}

interface Subscription {
  readonly event: string
  readonly options: SubscribeOptions
  readonly handler: MessageHandler
  readonly package: string
  readonly def: { readonly payload: import("../ir.js").TypeRef }
}

const OWN_OPTIONS = ["attempts", "deadLetter", "backoff", "package"] as const

function toWeftError(cause: unknown): WeftError {
  if (isWeftError(cause)) return cause
  return WeftError.internal(cause instanceof Error ? cause.message : String(cause), { cause })
}

function keyOf(fields: readonly string[] | undefined, payload: unknown): string | undefined {
  if (fields === undefined || fields.length === 0 || typeof payload !== "object" || payload === null) return undefined
  const parts: string[] = []
  for (const f of fields) {
    const v = (payload as Record<string, unknown>)[f]
    if (v === undefined || v === null) return undefined
    parts.push(v instanceof Date ? v.toISOString() : String(v))
  }
  return parts.join(":")
}

/**
 * Messaging over one adapter: `publish` encodes and sends an event, `on` receives it. Everything
 * the libraries have in common is here: encoding, CloudEvents envelopes, validation, middleware,
 * retries with backoff, dead letters. Register it with the `Weft` it is built on; it is a transport.
 */
export class Messaging implements Transport {
  readonly name: string
  readonly adapter: MessagingAdapter
  private readonly weft: Weft
  private readonly logger: Logger
  private readonly sleep: (ms: number, signal: AbortSignal) => Promise<void>
  private readonly random: () => number
  private readonly subscriptions: Subscription[] = []
  private readonly middleware: Middleware<MessageContext>[] = []
  private readonly stopController = new AbortController()
  private started = false

  constructor(weft: Weft, options: MessagingOptions) {
    this.weft = weft
    this.adapter = options.adapter
    this.name = options.name ?? options.adapter.name
    this.logger = options.logger ?? consoleLogger
    this.sleep = options.sleep ?? realSleep
    this.random = options.random ?? Math.random
    weft.register(this)
  }

  /** The library underneath. Weft's tracing and middleware do not apply to calls made through it. */
  get raw(): unknown {
    return this.adapter.raw
  }

  /** Middleware for this transport only. It runs after the middleware added to `Weft`. */
  use(middleware: Middleware<MessageContext>): this {
    this.middleware.push(middleware)
    return this
  }

  /** Receives an event. Register before `weft.start()`. */
  on<P = unknown>(event: string, handler: MessageHandler<P>, options: SubscribeOptions = {}): void {
    if (this.started) throw WeftError.internal(`Register the handler for ${event} before start()`)
    const found = this.weft.contracts.event(event, options.package)
    if (options.group !== undefined && !this.adapter.capabilities().consumerGroups) {
      throw WeftError.internal(`${this.name} does not support consumer groups, so the group option is not allowed`)
    }
    this.subscriptions.push({
      event,
      options,
      handler: handler as MessageHandler,
      package: found.package,
      def: found.def,
    })
  }

  /** Validates, encodes and sends an event. Retries `Unavailable` and `Timeout` failures with backoff. */
  async publish(event: string, payload: unknown, options: PublishOptions = {}): Promise<void> {
    if (this.weft.state !== "running" && this.weft.state !== "stopping") {
      throw WeftError.unavailable("Weft is not running: call start() before publishing")
    }
    const { package: pkg, key: keyOption, headers: extra, idempotencyKey, delayMs, ...adapterOptions } = options
    const { def } = this.weft.contracts.event(event, pkg)
    const caps = this.adapter.capabilities()
    if (delayMs !== undefined && !caps.delay) throw WeftError.internal(`${this.name} does not support delayed delivery (delayMs)`)
    if (extra !== undefined && !caps.headers) throw WeftError.internal(`${this.name} does not support message headers`)

    const wire = this.weft.contracts.encode(def.payload, payload)
    const key = keyOption ?? keyOf(def.key, payload)
    const body = new TextEncoder().encode(JSON.stringify(wire))
    const id = crypto.randomUUID()
    const destination = topicName(this.weft.config, event)
    const sent = delayMs === undefined ? adapterOptions : { ...adapterOptions, delayMs }
    const m = messagingMetrics()
    const attributes = { event, transport: this.name }
    try {
      await withSpan(
        `${destination} publish`,
        { kind: SpanKind.PRODUCER, attributes: this.attributes("publish", destination, id, body.byteLength, event) },
        async () => {
          const headers: Record<string, string> = { ...extra }
          if (idempotencyKey !== undefined) headers[IDEMPOTENCY_HEADER] = idempotencyKey
          injectTraceContext(headers)
          const envelope: Envelope = {
            id,
            type: event,
            source: this.weft.config.service,
            time: new Date().toISOString(),
            datacontenttype: "application/json",
            ...(key === undefined ? {} : { key }),
            headers,
            body,
          }
          await this.send(destination, envelope, sent)
        },
      )
      m.sent.add(1, { ...attributes, outcome: "ok" })
      m.size.record(body.byteLength, { ...attributes, direction: "send" })
    } catch (error) {
      m.sent.add(1, { ...attributes, outcome: "error" })
      m.errors.add(1, { ...attributes, category: isWeftError(error) ? error.category : "Internal" })
      throw error
    }
  }

  // ---- Transport ----

  async start(): Promise<void> {
    await this.adapter.start()
    for (const sub of this.subscriptions) {
      const destination = topicName(this.weft.config, sub.event)
      const adapterOptions = Object.fromEntries(
        Object.entries(sub.options).filter(([k]) => !(OWN_OPTIONS as readonly string[]).includes(k)),
      )
      await this.adapter.subscribe(destination, (delivery) => this.receive(sub, destination, delivery), adapterOptions)
    }
    this.started = true
  }

  async stopIntake(): Promise<void> {
    this.stopController.abort()
    await this.adapter.stopIntake()
  }

  async close(): Promise<void> {
    await this.adapter.close()
  }

  // ---- internals ----

  private get stopping(): boolean {
    return this.stopController.signal.aborted || this.weft.state === "stopping" || this.weft.state === "stopped"
  }

  private async send(destination: string, envelope: Envelope, options: Readonly<Record<string, unknown>>): Promise<void> {
    const max = DEFAULTS.messaging.attempts
    for (let attempt = 1; ; attempt++) {
      try {
        await this.adapter.publish(destination, envelope, options)
        return
      } catch (cause) {
        const error = isWeftError(cause) ? cause : WeftError.unavailable(`${this.name} could not send to ${destination}`, { cause })
        const retryable = error.category === "Unavailable" || error.category === "Timeout"
        if (!retryable || attempt >= max) throw error
        await this.sleep(backoffMs(attempt, {}, this.random), this.stopController.signal)
        if (this.stopController.signal.aborted && this.weft.state !== "running") throw error
      }
    }
  }

  private async receive(sub: Subscription, destination: string, delivery: Delivery): Promise<void> {
    const { envelope } = delivery
    const max = sub.options.attempts ?? DEFAULTS.messaging.attempts
    let payload: unknown
    try {
      payload = this.weft.contracts.decodeJson(sub.def.payload, new TextDecoder().decode(envelope.body))
    } catch (cause) {
      await this.deadLetter(sub, destination, envelope, toWeftError(cause), 1)
      return
    }

    for (let attempt = 1; ; attempt++) {
      const ctx = this.context(sub, envelope, attempt)
      const m = messagingMetrics()
      const attributes = { event: sub.event, transport: this.name }
      const started = performance.now()
      try {
        await withSpan(
          `${destination} process`,
          {
            kind: SpanKind.CONSUMER,
            parent: extractTraceContext(envelope.headers),
            attributes: { ...this.attributes("process", destination, envelope.id, envelope.body.byteLength, sub.event), "weft.attempt": attempt },
          },
          () => this.weft.dispatch(ctx, (c) => sub.handler(payload, c as MessageContext), this.middleware),
        )
        m.duration.record(performance.now() - started, { ...attributes, outcome: "ok" })
        m.received.add(1, { ...attributes, outcome: "ok" })
        m.size.record(envelope.body.byteLength, { ...attributes, direction: "receive" })
        return
      } catch (cause) {
        const error = toWeftError(cause)
        m.duration.record(performance.now() - started, { ...attributes, outcome: "error" })
        m.errors.add(1, { ...attributes, category: error.category })
        // Shutting down: do not acknowledge, so the broker delivers it again after a restart.
        if (this.stopping) {
          m.received.add(1, { ...attributes, outcome: "not_acked" })
          throw WeftError.unavailable("Shutting down before the message was handled", { cause })
        }
        const policy = error.messagingPolicy
        if (policy === "none") {
          m.received.add(1, { ...attributes, outcome: "dropped" })
          this.logger.warn(`${sub.event} handler failed with ${error.category}; not retried`, { id: envelope.id, message: error.message })
          return
        }
        if (policy === "dead-letter" || attempt >= max) {
          m.received.add(1, { ...attributes, outcome: "dead_letter" })
          await this.deadLetter(sub, destination, envelope, error, attempt)
          return
        }
        m.received.add(1, { ...attributes, outcome: "retry" })
        await this.sleep(backoffMs(attempt, sub.options.backoff, this.random), this.stopController.signal)
        if (this.stopping) throw WeftError.unavailable("Shutting down before the message was handled", { cause })
      }
    }
  }

  private async deadLetter(
    sub: Subscription,
    destination: string,
    envelope: Envelope,
    error: WeftError,
    attempts: number,
  ): Promise<void> {
    const target = sub.options.deadLetter === undefined ? `${destination}.dlq` : sub.options.deadLetter
    if (target === false) {
      this.logger.error(`${sub.event} message dropped after ${attempts} attempt(s): ${error.message}`, { id: envelope.id })
      return
    }
    const dead: Envelope = {
      ...envelope,
      headers: {
        ...envelope.headers,
        "weft-original-destination": destination,
        "weft-error-category": error.category,
        "weft-error-message": error.message,
        "weft-attempts": String(attempts),
      },
    }
    this.logger.error(`${sub.event} message sent to ${target} after ${attempts} attempt(s): ${error.message}`, { id: envelope.id })
    await withSpan(
      `${target} publish`,
      {
        kind: SpanKind.PRODUCER,
        parent: extractTraceContext(envelope.headers),
        attributes: this.attributes("publish", target, envelope.id, envelope.body.byteLength, sub.event),
      },
      () => this.send(target, dead, {}),
    )
  }

  private attributes(operation: "publish" | "process", destination: string, id: string, bytes: number, event: string) {
    return {
      "messaging.system": this.name,
      "messaging.destination.name": destination,
      "messaging.operation.type": operation,
      "messaging.message.id": id,
      "messaging.message.body.size": bytes,
      "weft.event": event,
    }
  }

  private context(sub: Subscription, envelope: Envelope, attempt: number): MessageContext {
    return {
      transport: this.name,
      operation: sub.event,
      signal: this.weft.shutdownSignal,
      headers: { ...envelope.headers },
      state: {},
      event: sub.event,
      envelope,
      ...(envelope.key === undefined ? {} : { key: envelope.key }),
      attempt,
      timestamp: new Date(envelope.time),
    }
  }
}
