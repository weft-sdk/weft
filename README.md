# Weft

> **One interface for every way your services talk.**

Weft is a thin layer over the communication libraries you already use. Instead of learning and wiring up a different library for HTTP, GraphQL, gRPC, tRPC, Kafka, RabbitMQ, WebSockets and the rest, you call one consistent API, and Weft runs the real library underneath.

```ts
weft.http.post("/orders", createOrder)          // Express underneath
weft.gql.query("order", { id })                  // Apollo underneath
weft.grpc.call("orders.CreateOrder", req)        // grpc-js underneath
weft.kafka.publish("order.placed", order)        // KafkaJS underneath
weft.kafka.on("order.placed", handleOrder)
weft.mq.send("emails", message)                  // RabbitMQ underneath
weft.ws.emit("courier.moved", position)          // Socket.io underneath
```

The same API exists in 13 languages, so a Go service and a Python service talk to a TypeScript service through the same contracts.

---

## The problem

A typical microservice system uses several communication styles at once:

- REST for public APIs and the frontend
- GraphQL in front of the frontend
- gRPC between internal services
- Kafka or RabbitMQ for events
- WebSockets for live updates

Each one comes with its own library, its own setup, its own way of defining types, its own error handling, retries and tracing. A developer moving between them relearns everything. Types drift between the service that sends data and the services that receive it, and nobody notices until production.

## How Weft works

**1. Set up.** `weft init` asks which communication styles you use and which library you want for each, then installs and configures them.

```text
$ npx @weft-sdk/cli init
? Which communication do you use?   ◉ HTTP  ◉ GraphQL  ◉ Kafka  ◉ WebSockets  ◯ gRPC  ◯ RabbitMQ
? HTTP library?        › Express
? GraphQL library?     › Apollo
? Kafka library?       › KafkaJS
? WebSocket library?   › Socket.io
✓ Installed @weft-sdk/core and 4 adapters
✓ Wrote weft.yaml
```

**2. Write contracts.** A contract is a name plus a type, written once in a small language-neutral file.

```weft
type Order {
  id: uuid
  items: list<OrderItem>
  total: Money
}

event order.placed: Order
```

**3. Generate and call.** `weft gen` produces typed code for your language. Then you call `weft.*` and get autocomplete and type checking on every name and payload.

## What you get

- **One API** for every transport, in every language.
- **Typed end to end.** The sender and every receiver use the same generated types, whatever language they're written in.
- **Your stack stays.** Weft runs KafkaJS, Express, Spring Kafka, FastAPI and the rest underneath. The original client is always reachable if you need it.
- **Shared behaviour everywhere.** Tracing (OpenTelemetry), retries, timeouts, error formats and event metadata work the same on every transport.
- **Breaking changes caught early.** `weft check` compares contract changes against the last version before you merge.
- **A live map.** The Weft dashboard shows which services talk to which, over what, with live traffic. Run it locally on a port, self-host it, or use it in the cloud.

## Supported

**Languages:** TypeScript/JavaScript, Go, Python, Java/Kotlin/Scala (JVM), C#/F# (.NET), Rust, Ruby, PHP, Elixir/Erlang, Swift, Dart, plus browser and mobile clients.

**Communication:** HTTP/REST, GraphQL, gRPC, typed RPC (tRPC-style), WebSockets, server-sent events, Kafka, RabbitMQ/AMQP, NATS, Redis Pub/Sub and Streams, AWS SQS/SNS/EventBridge, Google Pub/Sub, Azure Service Bus, MQTT, Pulsar.

Full matrix of languages and the libraries Weft runs underneath: [docs/languages.md](docs/languages.md).

## Documentation

Design docs live in [docs/](docs/README.md).

## Status

Design stage. Nothing is implemented yet.
