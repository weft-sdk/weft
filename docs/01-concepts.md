# Concepts

Every other doc uses these terms with exactly these meanings.

## Transport

A **transport** is a way services communicate. Each has a namespace on the `weft` object:

| Namespace | Transport | Style |
| --- | --- | --- |
| `weft.http` | HTTP / REST | request–response |
| `weft.gql` | GraphQL | request–response, subscriptions |
| `weft.grpc` | gRPC, gRPC-Web, Connect | request–response, streaming |
| `weft.rpc` | Typed function calls (tRPC-style) | request–response |
| `weft.ws` | WebSockets, Socket.io-style events | two-way events |
| `weft.sse` | Server-sent events | server → client stream |
| `weft.kafka` | Kafka and Kafka-compatible (Redpanda, MSK, Confluent) | messaging |
| `weft.mq` | RabbitMQ / AMQP, ActiveMQ | messaging |
| `weft.nats` | NATS, JetStream | messaging |
| `weft.redis` | Redis Pub/Sub and Streams | messaging |
| `weft.cloud` | AWS SQS/SNS/EventBridge, Google Pub/Sub, Azure Service Bus | messaging |
| `weft.mqtt` | MQTT | messaging (IoT) |
| `weft.pulsar` | Apache Pulsar | messaging |

## Library and adapter

A **library** is the real client or server Weft runs underneath: KafkaJS, Express, Spring Kafka, FastAPI, Socket.io.

An **adapter** is the small piece of Weft code that connects one transport namespace to one library in one language. `@weft-sdk/kafka-kafkajs` is the adapter that makes `weft.kafka` run on KafkaJS in TypeScript.

A project installs one adapter per transport it uses. `weft init` picks them.

## Contract

A **contract** describes what is sent and received, independent of language. It is written in a `.weft` file ([02-contract-language.md](02-contract-language.md)) and contains:

- **types**: the shape of data (`Order`, `Money`)
- **operations**: the named things you can call or listen to (an event, an HTTP route, an RPC method, a GraphQL field, a socket event)

Contracts are the single source of truth. Generated code in every language comes from them.

### Operations are mostly transport-neutral

An `event` contract says *what* a message looks like, not *how* it travels. The same `order.placed` event can be published over Kafka, RabbitMQ, NATS or SQS. The call site picks the transport:

```ts
weft.kafka.publish("order.placed", order)
weft.mq.publish("order.placed", order)
```

Likewise, an `rpc` contract can be served over gRPC or over `weft.rpc`. HTTP routes, GraphQL fields and socket events have their own contract kinds because their shape is tied to the transport.

## Generated code

`weft gen` turns contracts into code for one language: types, plus a typed `weft` object that only accepts names and payloads that exist in the contracts.

```ts
import { weft } from "./weft.gen"

weft.kafka.publish("order.placed", order)   // ✓ name and type checked
weft.kafka.publish("order.plcaed", order)   // ✗ compile error: unknown event
```

In languages without static types (JavaScript, Ruby, Python without type checking), the generated code validates payloads at runtime instead.

## Package

A **package** is Weft for one language: the core plus its adapters, published to that language's registry (npm, PyPI, Maven Central, ...). See [04-sdk-design.md](04-sdk-design.md).

## Service

A **service** is one deployable program using Weft, with a name set in `weft.yaml`. Services **own** the contracts they provide (the publisher of an event, the server of a route) and **use** contracts owned by others.

## Escape hatch

Weft never traps a user. Every transport exposes the underlying library's client:

```ts
const kafka = weft.kafka.raw    // the real KafkaJS instance
```

If Weft doesn't support a feature yet, users drop down to the library for that one call.

## Dashboard

The **dashboard** shows the service map built from contracts and live traffic reported by Weft packages. It runs locally on a port, self-hosted, or in Weft Cloud. See [08-dashboard.md](08-dashboard.md).
