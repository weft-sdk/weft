import type { Context } from "../middleware.js"
import type { Transport } from "../weft.js"

/**
 * One message as it travels: a CloudEvents 1.0 envelope around the encoded payload. Adapters carry
 * the attributes in the transport's own headers (binary mode) and the body as the message value.
 */
export interface Envelope {
  /** CloudEvents `id`: unique per message. */
  readonly id: string
  /** CloudEvents `type`: the event name, such as `order.placed`. */
  readonly type: string
  /** CloudEvents `source`: the service that sent it. */
  readonly source: string
  /** CloudEvents `time`: RFC 3339. */
  readonly time: string
  readonly datacontenttype: string
  /** The ordering and partition key, if the event declares one. */
  readonly key?: string
  /** Other headers: trace context, the idempotency key, and anything the sender added. */
  readonly headers: Readonly<Record<string, string>>
  /** The encoded payload (JSON by default). */
  readonly body: Uint8Array
}

/** What a library can do. Core rejects an option the adapter cannot honour, with a clear error. */
export interface MessagingCapabilities {
  readonly ordering: boolean
  readonly keys: boolean
  readonly headers: boolean
  readonly transactions: boolean
  readonly requestReply: boolean
  readonly delay: boolean
  readonly consumerGroups: boolean
}

/** Options for `publish`. Beyond these, an adapter may accept options of its own. */
export interface PublishOptions {
  /** Overrides the key taken from the event's `key:`. */
  readonly key?: string
  readonly headers?: Readonly<Record<string, string>>
  /** Sets `weft-idempotency-key`, which `dedupe()` uses on the receiving side. */
  readonly idempotencyKey?: string
  /** Delay delivery. Needs the `delay` capability. */
  readonly delayMs?: number
  /** Another package, if two declare the same event name. */
  readonly package?: string
  readonly [option: string]: unknown
}

/** Options for `on`. */
export interface SubscribeOptions {
  /** Consumer group (Kafka), or the equivalent. Needs the `consumerGroups` capability. */
  readonly group?: string
  readonly fromBeginning?: boolean
  /** Attempts before the message is dead-lettered. Default 5. */
  readonly attempts?: number
  /** Where failed messages go. Default `<destination>.dlq`; `false` drops them. */
  readonly deadLetter?: string | false
  readonly backoff?: { readonly minMs?: number; readonly maxMs?: number }
  readonly package?: string
  readonly [option: string]: unknown
}

/** What an adapter is given to deliver. */
export interface Delivery {
  readonly envelope: Envelope
  /** How many times the broker has delivered this message, when it knows. */
  readonly deliveryCount?: number
}

/**
 * The messaging library underneath, as core needs it. Adapters only translate: encoding, envelopes,
 * validation, middleware, retries and dead letters are core's. A `handler` that resolves means
 * "acknowledge"; one that rejects means "do not acknowledge" (shutdown, or a crash), and the broker
 * will deliver the message again.
 */
export interface MessagingAdapter<Raw = unknown> extends Transport {
  readonly raw: Raw
  capabilities(): MessagingCapabilities
  publish(destination: string, envelope: Envelope, options: Readonly<Record<string, unknown>>): Promise<void>
  subscribe(
    destination: string,
    handler: (delivery: Delivery) => Promise<void>,
    options: Readonly<Record<string, unknown>>,
  ): Promise<void>
}

/** What a messaging handler sees besides the payload. */
export interface MessageContext extends Context {
  readonly event: string
  readonly envelope: Envelope
  readonly key?: string
  /** 1 for the first try, then 2, and so on. */
  readonly attempt: number
  readonly timestamp: Date
}

export type MessageHandler<P = unknown> = (payload: P, ctx: MessageContext) => Promise<void> | void
