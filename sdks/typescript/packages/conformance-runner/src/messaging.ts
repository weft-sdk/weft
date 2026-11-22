/**
 * The `messaging` operation: one scripted scenario on an in-memory broker, with a fixed jitter and a
 * recorded sleep, so every language can be held to the same retries, dead letters and envelope.
 */
import {
  Contracts,
  MemoryAdapter,
  Messaging,
  Weft,
  WeftError,
  dedupe,
  parseConfig,
  type Envelope,
  type ErrorCategory,
} from "@weft-sdk/core"

interface Request {
  event: string
  payload?: unknown
  rawBody?: string
  topic?: string
  handler?: { fail?: { times: number | "always"; category: string } }
  subscribe?: { attempts?: number; deadLetter?: string | false }
  publish?: { key?: string; idempotencyKey?: string; headers?: Record<string, string>; delayMs?: number; times?: number }
  dedupe?: boolean
  stopDuringBackoff?: boolean
}

const quiet = { warn: () => undefined, error: () => undefined }

function failure(category: string): Error {
  if (category === "Plain") return new Error("a plain exception")
  return new WeftError(category as ErrorCategory, `failed with ${category}`)
}

export async function messaging(contracts: Contracts, req: Request): Promise<Record<string, unknown>> {
  const yaml = `service: conformance\n${req.topic ? `topics:\n  ${req.event}: ${req.topic}\n` : ""}`
  const weft = new Weft({ contracts, config: parseConfig(yaml, {}), shutdownGraceMs: 2000 })
  const adapter = new MemoryAdapter("kafka")
  const broker = adapter.raw
  const sleeps: number[] = []
  const messaging = new Messaging(weft, {
    adapter,
    logger: quiet,
    random: () => 0.5,
    sleep: async (ms, signal) => {
      sleeps.push(ms)
      if (req.stopDuringBackoff) await new Promise<void>((resolve) => signal.addEventListener("abort", () => resolve(), { once: true }))
    },
  })
  if (req.dedupe) messaging.use(dedupe())

  const { def } = contracts.event(req.event)
  const attempts: number[] = []
  let received: unknown = null
  let failures = 0
  messaging.on(
    req.event,
    (payload, ctx) => {
      attempts.push(ctx.attempt)
      const fail = req.handler?.fail
      if (fail !== undefined && (fail.times === "always" || failures < fail.times)) {
        failures += 1
        throw failure(fail.category)
      }
      received = contracts.encode(def.payload, payload)
    },
    {
      ...(req.subscribe?.attempts === undefined ? {} : { attempts: req.subscribe.attempts }),
      ...(req.subscribe?.deadLetter === undefined ? {} : { deadLetter: req.subscribe.deadLetter }),
    },
  )
  await weft.start()

  let publishError: string | null = null
  const destination = req.topic ?? req.event
  if (req.rawBody !== undefined) {
    const envelope: Envelope = { id: "raw-1", type: req.event, source: "elsewhere", time: new Date().toISOString(), datacontenttype: "application/json", headers: {}, body: new TextEncoder().encode(req.rawBody) }
    broker.publish(destination, envelope, {})
    await broker.idle()
  } else {
    const decoded = contracts.tryDecode(def.payload, req.payload)
    if (!decoded.ok) {
      publishError = "InvalidInput"
    } else {
      const p = req.publish ?? {}
      for (let i = 0; i < (p.times ?? 1); i++) {
        try {
          await messaging.publish(req.event, decoded.value, {
            ...(p.key === undefined ? {} : { key: p.key }),
            ...(p.idempotencyKey === undefined ? {} : { idempotencyKey: p.idempotencyKey }),
            ...(p.headers === undefined ? {} : { headers: p.headers }),
            ...(p.delayMs === undefined ? {} : { delayMs: p.delayMs }),
          })
        } catch (error) {
          publishError = error instanceof WeftError ? error.category : "Internal"
          break
        }
        if (req.stopDuringBackoff) {
          for (let wait = 0; attempts.length < 1 && wait < 400; wait++) await new Promise((r) => setTimeout(r, 5))
        } else {
          await broker.idle()
        }
      }
    }
  }
  if (req.stopDuringBackoff) await weft.stop()
  await broker.idle()
  if (!req.stopDuringBackoff) await weft.stop()

  const sent = broker.published.filter((m) => m.destination === destination && m.envelope.id !== "raw-1")[0]?.envelope
  return {
    publishError,
    calls: attempts.length,
    attempts,
    sleeps,
    received,
    destinations: broker.published.map((m) => m.destination),
    envelope:
      sent === undefined
        ? null
        : { type: sent.type, source: sent.source, datacontenttype: sent.datacontenttype, key: sent.key ?? null, hasId: sent.id.length > 0, hasTime: !Number.isNaN(Date.parse(sent.time)), headers: sent.headers },
    dlq: broker.published
      .filter((m) => m.envelope.headers["weft-error-category"] !== undefined)
      .map((m) => ({
        destination: m.destination,
        category: m.envelope.headers["weft-error-category"],
        attempts: m.envelope.headers["weft-attempts"],
        originalDestination: m.envelope.headers["weft-original-destination"],
        sameId: m.envelope.id === sent?.id || m.envelope.id === "raw-1",
      })),
    unacked: broker.unacked.length,
  }
}
