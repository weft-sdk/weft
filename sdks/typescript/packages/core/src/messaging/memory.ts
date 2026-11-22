import { WeftError } from "../errors.js"
import type { Delivery, Envelope, MessagingAdapter, MessagingCapabilities } from "./types.js"

type Handler = (delivery: Delivery) => Promise<void>

interface Subscriber {
  readonly group: string
  readonly handler: Handler
}

/** A published message, as the broker saw it. */
export interface Published {
  readonly destination: string
  readonly envelope: Envelope
  readonly options: Readonly<Record<string, unknown>>
}

/**
 * An in-process broker for tests and examples. Each consumer group gets every message once, handed
 * to one of its subscribers in turn; a subscriber that rejects leaves the message in `unacked`.
 */
export class MemoryBroker {
  readonly published: Published[] = []
  readonly unacked: { destination: string; envelope: Envelope }[] = []
  private readonly subscribers = new Map<string, Subscriber[]>()
  private readonly turn = new Map<string, number>()
  private readonly pending = new Set<Promise<void>>()

  subscribe(destination: string, group: string, handler: Handler): void {
    const list = this.subscribers.get(destination) ?? []
    list.push({ group, handler })
    this.subscribers.set(destination, list)
  }

  unsubscribeAll(handlers: ReadonlySet<Handler>): void {
    for (const [destination, list] of this.subscribers) {
      this.subscribers.set(destination, list.filter((s) => !handlers.has(s.handler)))
    }
  }

  publish(destination: string, envelope: Envelope, options: Readonly<Record<string, unknown>>): void {
    this.published.push({ destination, envelope, options })
    const byGroup = new Map<string, Subscriber[]>()
    for (const s of this.subscribers.get(destination) ?? []) byGroup.set(s.group, [...(byGroup.get(s.group) ?? []), s])
    for (const [group, members] of byGroup) {
      const k = `${destination}/${group}`
      const i = this.turn.get(k) ?? 0
      this.turn.set(k, i + 1)
      const member = members[i % members.length] as Subscriber
      const run = member.handler({ envelope, deliveryCount: 1 }).catch(() => {
        this.unacked.push({ destination, envelope })
      })
      this.pending.add(run)
      void run.finally(() => this.pending.delete(run))
    }
  }

  /** Resolves when every delivery started so far has finished. */
  async idle(): Promise<void> {
    while (this.pending.size > 0) await Promise.all([...this.pending])
  }

  /** The messages published to one destination. */
  to(destination: string): Published[] {
    return this.published.filter((p) => p.destination === destination)
  }
}

const CAPABILITIES: MessagingCapabilities = {
  ordering: true,
  keys: true,
  headers: true,
  transactions: false,
  requestReply: false,
  delay: false,
  consumerGroups: true,
}

/** The messaging adapter over a `MemoryBroker`. */
export class MemoryAdapter implements MessagingAdapter<MemoryBroker> {
  readonly name: string
  readonly raw: MemoryBroker
  private readonly handlers = new Set<Handler>()
  private intake = true
  failPublish = 0

  constructor(name = "memory", broker = new MemoryBroker()) {
    this.name = name
    this.raw = broker
  }

  capabilities(): MessagingCapabilities {
    return CAPABILITIES
  }

  async start(): Promise<void> {
    this.intake = true
  }

  async stopIntake(): Promise<void> {
    this.intake = false
    this.raw.unsubscribeAll(this.handlers)
  }

  async close(): Promise<void> {
    await this.raw.idle()
  }

  async publish(destination: string, envelope: Envelope, options: Readonly<Record<string, unknown>>): Promise<void> {
    if (this.failPublish > 0) {
      this.failPublish -= 1
      throw WeftError.unavailable("the memory broker is pretending to be down")
    }
    this.raw.publish(destination, envelope, options)
  }

  async subscribe(destination: string, handler: Handler, options: Readonly<Record<string, unknown>>): Promise<void> {
    const group = typeof options.group === "string" ? options.group : "default"
    const guarded: Handler = (delivery) => (this.intake ? handler(delivery) : Promise.reject(new Error("not taking new work")))
    this.handlers.add(guarded)
    this.raw.subscribe(destination, group, guarded)
  }
}
