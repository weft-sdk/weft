# Language package design

Each language gets a native Weft package. This doc defines what every package MUST contain and how it MUST behave, so that a developer moving between languages finds the same thing.

## Structure

```
core                       the weft.* object, lifecycle, config, middleware, codecs, errors, telemetry
 ├── adapter: http-<lib>    one per supported HTTP library
 ├── adapter: kafka-<lib>   one per supported Kafka library
 ├── adapter: ws-<lib>      ...
 └── ...
generated code             produced by `weft gen` inside the user's project
```

Package naming per ecosystem:

| Ecosystem | Core | Adapter example |
| --- | --- | --- |
| npm | `@weft-sdk/core` | `@weft-sdk/kafka-kafkajs` |
| Go | `github.com/weft-sdk/weft/go` | `github.com/weft-sdk/weft/go/kafka/franz` |
| PyPI | `weft-core` | `weft-core[kafka-confluent]` (extras) |
| Maven | `io.github.weft-sdk:weft-core` | `io.github.weft-sdk:weft-kafka-spring` |
| NuGet | `WeftSdk.Core` | `WeftSdk.Kafka.Confluent` |
| crates.io | `weft-sdk` | `weft-sdk` with feature `kafka-rdkafka` |
| RubyGems | `weft-core` | `weft-kafka-karafka` |
| Packagist | `weft-sdk/core` | `weft-sdk/kafka-rdkafka` |
| Hex | `:weft_sdk` | `:weft_sdk_kafka_brod` |
| SwiftPM | `Weft` | `WeftWebSocket` |
| pub.dev | `weft_sdk` | `weft_sdk_ws` |


## The adapter interface

Each transport defines a small interface in core. An adapter implements it on top of one library. For messaging, in pseudocode:

```
interface MessagingAdapter {
  connect(config) / close(graceful)
  publish(destination, envelope, options)
  subscribe(destination, handler(envelope, ack), options)
  raw()
  capabilities() -> { ordering, keys, headers, transactions, requestReply, delay, ... }
}
```

Core does everything that is the same across libraries: encoding, CloudEvents envelopes, validation, middleware, retries, tracing, errors. Adapters only translate to and from the library. This keeps adapters small (typically a few hundred lines) and behaviour consistent.

`capabilities()` lets core reject an option a library can't honour with a clear error, instead of silently ignoring it.

## Behaviour every package MUST implement

| Area | Requirement |
| --- | --- |
| Encoding | JSON by default. Protobuf, Avro, MessagePack via config. Wire names from contracts |
| Event envelope | CloudEvents 1.0 (binary mode: metadata in headers where the transport has headers) |
| Validation | Outgoing and incoming payloads validated against contracts. On by default in dynamic languages, opt-in in static ones |
| Tracing | OpenTelemetry spans for every send and receive. W3C Trace Context propagated through every transport, including Kafka headers, AMQP properties and WebSocket messages |
| Metrics | OpenTelemetry metrics: count, latency, errors, payload size, per operation |
| Retries | Exponential backoff with jitter. Defaults per transport, configurable per operation |
| Timeouts | Every outgoing call has a deadline. Deadlines propagate across services |
| Idempotency | Optional idempotency key on publish and HTTP calls; dedupe middleware on receive |
| Dead letters | Messages that exhaust retries go to `<destination>.dlq` (configurable) |
| Errors | The error categories in [03-api.md § Errors](03-api.md#errors) |
| Shutdown | `stop()` stops intake, waits for in-flight handlers (with timeout), commits offsets, closes connections |
| Config | `weft.yaml` + environment variables (`WEFT_KAFKA_BROKERS`), same keys in every language |
| Escape hatch | `raw` on every transport |

## Defaults

| Setting | Default |
| --- | --- |
| Messaging delivery | At-least-once. Ack after the handler succeeds |
| Ordering key | The event's declared `key`, else none |
| Retries (messaging) | 5 attempts, 200 ms → 30 s backoff, then dead-letter |
| Retries (HTTP, gRPC calls) | 3 attempts, only for `Unavailable` and `Timeout`, only for idempotent operations |
| Call timeout | 10 s |
| Shutdown grace period | 30 s |

## Idiomatic per language

The API shape is shared. The style is native:

- **Async model:** Promises in TS, `context.Context` + goroutines in Go, `async`/`await` in Python, coroutines in Kotlin, `Task` in C#, Tokio in Rust, fibers/threads in Ruby, Fibers in PHP 8.1+, processes in Elixir.
- **Errors:** exceptions where idiomatic, `(T, error)` in Go, `Result<T, E>` in Rust, `{:ok, v} | {:error, e}` in Elixir.
- **Registration style:** decorators in Python, annotations in Java/Kotlin, attributes in C#, plain function calls elsewhere. All MUST also support plain function calls.
- **Framework integration:** Spring Boot starter, ASP.NET Core extension, NestJS module, FastAPI plugin, Rails engine, Laravel service provider, Phoenix integration.

## Generated code

`weft gen` writes, per language:

- types for every contract type
- the checked contracts (the [IR](../spec/ir.md)) as a constant, so the runtime has the validation rules, wire names and operation metadata for the dashboard and tracing
- a typed `weft` facade that only accepts names and payloads defined in the contracts

Validation, encoding and routing run in the language's core package, reading the embedded contracts, so every language behaves the same by construction ([ADR 0021](decisions/0021-generated-code-embeds-the-ir.md)). Generated files carry a header comment, are deterministic, and are safe to commit. `weft gen --check` fails CI if they're out of date.

### TypeScript

One file, `weft.gen.ts`:

```ts
export type Uuid = string & { readonly __weft: "uuid" }       // uuids are branded

export namespace orders {                                      // types only, so the file works with type stripping
  export type Currency = "EUR" | "GBP" | "USD"
  export interface Money { amount: number; currency: Currency }
  export interface Order { id: Uuid; total: Money; note?: string | null; placedAt: Date; /* ... */ }
  export interface Events { "order.placed": Order; "order.cancelled": OrderCancelled }
}

export const contracts = { orders: { /* the IR of the orders package */ } } as const
```

Optional fields accept `null` as well as a missing value, as the contract language says. `int` is `number`, or `bigint` with `codegen.int64: bigint`. Names that TypeScript cannot hold (`Record`, `Date`, keywords) are errors that say which name to change. The typed facade is added by the TypeScript package (phase P2).

## Quality bar

A package is released only when it passes the [conformance suite](07-conformance.md) for every adapter it ships.
