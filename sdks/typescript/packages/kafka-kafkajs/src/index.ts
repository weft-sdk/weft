import { Kafka, logLevel, type Consumer, type KafkaConfig, type Producer } from "kafkajs"
import {
  WeftError,
  fromHeaders,
  isWeftError,
  toHeaders,
  type Config,
  type Delivery,
  type Envelope,
  type MessagingAdapter,
  type MessagingCapabilities,
} from "@weft-sdk/core"

/** CloudEvents binary mode on Kafka puts the attributes in `ce_` headers. */
const PREFIX = "ce_"

export interface KafkaJsAdapterOptions {
  /** Your own KafkaJS instance. Without one, the adapter makes its own from the brokers. */
  readonly kafka?: Kafka
  /** Or `transports.kafka.brokers` in weft.yaml: a list, or a comma-separated string. */
  readonly brokers?: readonly string[]
  /** Default: the service name. */
  readonly clientId?: string
  /** The consumer group used when `on()` gives none. Or `transports.kafka.group`. Default: the service name. */
  readonly group?: string
  /** Other KafkaJS settings (ssl, sasl, ...), used when the adapter makes its own instance. */
  readonly config?: Omit<KafkaConfig, "brokers" | "clientId">
}

interface Subscription {
  readonly topic: string
  readonly group: string
  readonly fromBeginning: boolean
  readonly handler: (delivery: Delivery) => Promise<void>
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

/** `"a:9092, b:9092"` or `["a:9092"]` to a list. */
export function parseBrokers(value: unknown): string[] {
  const list = Array.isArray(value) ? value : typeof value === "string" ? value.split(",") : []
  return list.map((b) => String(b).trim()).filter((b) => b !== "")
}

function toWeftError(error: unknown, what: string): WeftError {
  if (isWeftError(error)) return error
  const retriable = (error as { retriable?: boolean } | null)?.retriable
  const message = `${what}: ${error instanceof Error ? error.message : String(error)}`
  return retriable === false ? WeftError.internal(message, { cause: error }) : WeftError.unavailable(message, { cause: error })
}

function textHeaders(raw: Record<string, Buffer | string | (Buffer | string)[] | undefined> | undefined): Record<string, string> {
  const out: Record<string, string> = {}
  for (const [k, v] of Object.entries(raw ?? {})) {
    const first = Array.isArray(v) ? v[0] : v
    if (first !== undefined) out[k] = first.toString()
  }
  return out
}

/**
 * Weft's messaging on KafkaJS. KafkaJS stays in charge of the connection, the partitions and the
 * rebalancing; Weft does the envelopes, validation, retries and dead letters. `raw` is the `Kafka` instance.
 *
 * A message is acknowledged (its offset committed) when the handler returns. Retries happen inside the
 * handler, so a message that is being retried holds up its partition, and its consumer must keep
 * heartbeating: keep the retry time well under the session timeout (30 s by default).
 */
export class KafkaJsAdapter implements MessagingAdapter<Kafka> {
  readonly name = "kafka"
  private kafka: Kafka | undefined
  private readonly options: KafkaJsAdapterOptions
  private settings: Readonly<Record<string, unknown>> = {}
  private service = "weft"
  private producer: Producer | undefined
  private readonly consumers: Consumer[] = []
  private readonly subscriptions: Subscription[] = []

  constructor(options: KafkaJsAdapterOptions = {}) {
    this.options = options
    this.kafka = options.kafka
  }

  /** The `Kafka` instance. Available once started (or at once, if you passed your own). */
  get raw(): Kafka {
    if (this.kafka === undefined) throw WeftError.internal("The Kafka client is created by start()")
    return this.kafka
  }

  configure(settings: Readonly<Record<string, unknown>>, config: Config): void {
    this.settings = settings
    this.service = config.service
  }

  capabilities(): MessagingCapabilities {
    return CAPABILITIES
  }

  async start(): Promise<void> {
    if (this.kafka === undefined) {
      const brokers = this.options.brokers !== undefined ? [...this.options.brokers] : parseBrokers(this.settings.brokers)
      if (brokers.length === 0) {
        throw WeftError.internal("There are no Kafka brokers: set brokers under transports.kafka in weft.yaml (or WEFT_KAFKA_BROKERS)")
      }
      const clientId = this.options.clientId ?? (typeof this.settings.clientId === "string" ? this.settings.clientId : this.service)
      this.kafka = new Kafka({ ...this.options.config, clientId, brokers, logLevel: logLevel.NOTHING })
    }
    this.producer = this.kafka.producer({ allowAutoTopicCreation: true })
    await this.producer.connect()
  }

  async subscribe(
    destination: string,
    handler: (delivery: Delivery) => Promise<void>,
    options: Readonly<Record<string, unknown>>,
  ): Promise<void> {
    const group =
      typeof options.group === "string"
        ? options.group
        : (this.options.group ?? (typeof this.settings.group === "string" ? this.settings.group : this.service))
    this.subscriptions.push({ topic: destination, group, fromBeginning: options.fromBeginning === true, handler })
  }

  /** KafkaJS must know every topic of a consumer before it runs, so consuming starts here. */
  async ready(): Promise<void> {
    const kafka = this.raw
    const groups = new Map<string, Subscription[]>()
    for (const s of this.subscriptions) groups.set(s.group, [...(groups.get(s.group) ?? []), s])
    for (const [groupId, subs] of groups) {
      const consumer = kafka.consumer({ groupId, allowAutoTopicCreation: true })
      this.consumers.push(consumer)
      await consumer.connect()
      for (const s of subs) await consumer.subscribe({ topic: s.topic, fromBeginning: s.fromBeginning })
      const byTopic = new Map(subs.map((s) => [s.topic, s]))
      await consumer.run({
        autoCommit: true,
        eachMessage: async ({ topic, partition, message }) => {
          const sub = byTopic.get(topic)
          if (sub === undefined) return
          const body = new Uint8Array(message.value ?? Buffer.alloc(0))
          const key = message.key?.toString()
          const headers = textHeaders(message.headers)
          const parsed = fromHeaders(headers, PREFIX, body, key)
          const envelope: Envelope = parsed.ok
            ? parsed.envelope
            : {
                // Not sent by Weft: give it an identity of its own.
                id: `${topic}:${partition}:${message.offset}`,
                type: topic,
                source: "unknown",
                time: new Date(Number(message.timestamp)).toISOString(),
                datacontenttype: "application/json",
                ...(key === undefined ? {} : { key }),
                headers,
                body,
              }
          await sub.handler({ envelope })
        },
      })
    }
  }

  async publish(destination: string, envelope: Envelope, options: Readonly<Record<string, unknown>>): Promise<void> {
    if (this.producer === undefined) throw WeftError.unavailable("The Kafka producer is not connected")
    const { acks, timeout, compression, partition } = options as { acks?: number; timeout?: number; compression?: number; partition?: number }
    try {
      await this.producer.send({
        topic: destination,
        ...(acks === undefined ? {} : { acks }),
        ...(timeout === undefined ? {} : { timeout }),
        ...(compression === undefined ? {} : { compression }),
        messages: [
          {
            ...(envelope.key === undefined ? {} : { key: envelope.key }),
            ...(partition === undefined ? {} : { partition }),
            value: Buffer.from(envelope.body),
            headers: toHeaders(envelope, PREFIX),
            timestamp: String(Date.parse(envelope.time) || Date.now()),
          },
        ],
      })
    } catch (error) {
      throw toWeftError(error, `Kafka could not send to ${destination}`)
    }
  }

  async stopIntake(): Promise<void> {
    await Promise.allSettled(this.consumers.map((c) => c.stop()))
  }

  async close(): Promise<void> {
    await Promise.allSettled(this.consumers.map((c) => c.disconnect()))
    await this.producer?.disconnect().catch(() => undefined)
    this.consumers.length = 0
    this.producer = undefined
  }
}

export function kafkaJsAdapter(options: KafkaJsAdapterOptions = {}): KafkaJsAdapter {
  return new KafkaJsAdapter(options)
}
