# Conformance suite

Supporting 13 languages and dozens of libraries only works if they all behave the same. The conformance suite is how we prove it. No package or adapter is released without passing it.

## What it tests

| Area | Example cases |
| --- | --- |
| Encoding | Every contract type round-trips exactly: int64 edges, decimals, timestamps with time zones, unicode, empty lists, optional vs null |
| Cross-language | A Go publisher and a Python consumer agree on every message. Every language pair, for every messaging transport |
| Envelope | CloudEvents fields present and identical across languages |
| Tracing | A trace started in TypeScript continues through Kafka into Go and over HTTP into Python as one trace |
| Retries | A handler failing twice then succeeding is called exactly 3 times with the right backoff |
| Dead letters | A message that always fails lands in the DLQ after the configured attempts |
| Errors | Each error category maps to the right HTTP status, gRPC code, and messaging behaviour |
| Shutdown | In-flight messages finish; no message is lost or acked twice |
| Validation | Invalid payloads are rejected with the same error in every language |
| Capabilities | Unsupported options fail at startup with a clear message |

## How it works

```
conformance/
├── cases/            # YAML test cases, language-neutral
├── contracts/        # .weft files the cases use
├── harness/          # Rust test runner
└── docker-compose.yml
```

A case:

```yaml
id: messaging.retry.then-succeed
transports: [kafka, mq, nats, redis, sqs, pubsub, servicebus, pulsar]
steps:
  - consumer: { lang: $B, on: order.placed, behaviour: fail-times(2) }
  - producer: { lang: $A, publish: order.placed, payload: fixtures/order.json }
expect:
  - handler-calls: 3
  - received-payload: fixtures/order.json
  - dlq: empty
```

Each language package ships a small **runner** program that the harness drives over a simple JSON protocol: "subscribe to this", "publish that", "report what you received". `$A` and `$B` expand to every language pair.

Real infrastructure runs in Docker via **Testcontainers**: Redpanda (Kafka), RabbitMQ, NATS, Redis, LocalStack (AWS), Google Pub/Sub emulator, Azurite / Service Bus emulator, Mosquitto (MQTT), Pulsar, and an OpenTelemetry collector for trace assertions.

## In CI

- Every pull request runs the cases for the languages and adapters it touches.
- Nightly runs the full matrix: every language × every adapter × every case.
- Results are published as a public compatibility table on the website, so users can see exactly what's verified.
