import type { Envelope } from "./types.js"

/** The CloudEvents attributes that are carried in headers (binary mode). `datacontenttype` is the content type. */
const ATTRIBUTES = ["id", "type", "source", "time"] as const

/**
 * The headers for an envelope in CloudEvents binary mode: `<prefix>specversion`, `<prefix>id`, ...
 * The prefix depends on the transport (`ce_` for Kafka, `ce-` for HTTP, `cloudEvents:` for AMQP).
 * The key and the content type are for the adapter to place natively; other headers are copied.
 */
export function toHeaders(envelope: Envelope, prefix: string): Record<string, string> {
  const headers: Record<string, string> = { ...envelope.headers }
  headers[`${prefix}specversion`] = "1.0"
  for (const name of ATTRIBUTES) headers[`${prefix}${name}`] = envelope[name]
  headers[`${prefix}datacontenttype`] = envelope.datacontenttype
  return headers
}

/** The reverse: an envelope from headers and a body. Returns what is missing when the message is not a CloudEvent. */
export function fromHeaders(
  headers: Readonly<Record<string, string>>,
  prefix: string,
  body: Uint8Array,
  key?: string,
): { ok: true; envelope: Envelope } | { ok: false; missing: string[] } {
  const missing = ATTRIBUTES.filter((name) => headers[`${prefix}${name}`] === undefined).map((n) => `${prefix}${n}`)
  if (missing.length > 0) return { ok: false, missing }
  const rest: Record<string, string> = {}
  for (const [k, v] of Object.entries(headers)) {
    if (!k.startsWith(prefix)) rest[k] = v
  }
  return {
    ok: true,
    envelope: {
      id: headers[`${prefix}id`] as string,
      type: headers[`${prefix}type`] as string,
      source: headers[`${prefix}source`] as string,
      time: headers[`${prefix}time`] as string,
      datacontenttype: headers[`${prefix}datacontenttype`] ?? "application/json",
      ...(key === undefined ? {} : { key }),
      headers: rest,
      body,
    },
  }
}
