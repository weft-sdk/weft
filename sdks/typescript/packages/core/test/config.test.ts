import { describe, expect, it } from "vitest"
import { ConfigError, parseConfig, serviceUrl, topicName } from "../src/index.js"

const FILE = `
service: orders
language: typescript
contracts:
  - ./contracts
transports:
  http:
    library: express
    port: 3000
  kafka:
    library: kafkajs
    brokers: \${KAFKA_BROKERS}
    group: orders
topics:
  order.placed: prod.orders.placed.v1
services:
  payments: \${PAYMENTS_URL}
  api: http://api-gateway:4000
telemetry:
  otlp: \${OTEL_EXPORTER_OTLP_ENDPOINT}
`
const ENV = { KAFKA_BROKERS: "k1:9092,k2:9092", PAYMENTS_URL: "http://payments", OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel:4318" }

function problems(text: string, env = {}): readonly string[] {
  try {
    parseConfig(text, env)
  } catch (e) {
    expect(e).toBeInstanceOf(ConfigError)
    return (e as ConfigError).problems
  }
  throw new Error("expected a ConfigError")
}

describe("parseConfig", () => {
  it("reads the documented file and fills in ${VAR}", () => {
    const c = parseConfig(FILE, ENV)
    expect(c.service).toBe("orders")
    expect(c.transports.http).toEqual({ library: "express", port: 3000 })
    expect(c.transports.kafka).toEqual({ library: "kafkajs", brokers: "k1:9092,k2:9092", group: "orders" })
    expect(c.services).toEqual({ payments: "http://payments", api: "http://api-gateway:4000" })
    expect(c.telemetry).toEqual({ otlp: "http://otel:4318" })
  })

  it("lists every missing variable at once", () => {
    const p = problems(FILE, {})
    expect(p).toHaveLength(3)
    expect(p.join("\n")).toMatch(/KAFKA_BROKERS .*transports\.kafka\.brokers/)
    expect(p.join("\n")).toMatch(/PAYMENTS_URL .*services\.payments/)
  })

  it("lets WEFT_<TRANSPORT>_<KEY> override the file, keeping the setting's type", () => {
    const c = parseConfig(FILE, { ...ENV, WEFT_KAFKA_BROKERS: "other:9092", WEFT_HTTP_PORT: "8080", WEFT_KAFKA_CLIENT_ID: "x" })
    expect(c.transports.kafka?.brokers).toBe("other:9092")
    expect(c.transports.http?.port).toBe(8080)
    expect(c.transports.kafka?.client_id).toBe("x")
  })

  it("still reports a missing ${VAR} when a WEFT_ override replaces it", () => {
    expect(problems(FILE, { WEFT_KAFKA_BROKERS: "o:1" }).join("\n")).toMatch(/KAFKA_BROKERS/)
  })

  it("overrides the service, other services and telemetry", () => {
    const c = parseConfig(FILE, { ...ENV, WEFT_SERVICE: "billing", WEFT_SERVICES_PAYMENTS: "http://p2", WEFT_SERVICES_NEW: "http://n", WEFT_TELEMETRY_OTLP: "http://t" })
    expect(c.service).toBe("billing")
    expect(c.services).toMatchObject({ payments: "http://p2", new: "http://n" })
    expect(c.telemetry.otlp).toBe("http://t")
  })

  it("ignores WEFT_ variables that name no transport", () => {
    const c = parseConfig(FILE, { ...ENV, WEFT_MQ_HOST: "x", WEFT_LOG_LEVEL: "debug" })
    expect(c.transports.mq).toBeUndefined()
  })

  it("requires a service and sound sections", () => {
    expect(problems("transports: {}")).toEqual(["service is required"])
    expect(problems("service: a\ntransports: [1]\ntopics: 3\nservices:\n  x: 1")).toEqual([
      "transports must be a map",
      "topics must be a map",
      "services.x must be a string",
    ])
  })

  it("rejects bad YAML and non-maps", () => {
    expect(problems("service: [")[0]).toMatch(/not valid YAML/)
    expect(problems("- a\n- b")).toEqual(["weft.yaml must be a map at the top level"])
    expect(problems("")).toEqual(["service is required"])
  })

  it("accepts an empty transport", () => {
    expect(parseConfig("service: a\ntransports:\n  http:\n", {}).transports.http).toEqual({})
  })

  it("maps events to topics and services to URLs", () => {
    const c = parseConfig(FILE, ENV)
    expect(topicName(c, "order.placed")).toBe("prod.orders.placed.v1")
    expect(topicName(c, "order.cancelled")).toBe("order.cancelled")
    expect(serviceUrl(c, "api")).toBe("http://api-gateway:4000")
    expect(serviceUrl(c, "nope")).toBeUndefined()
  })
})
