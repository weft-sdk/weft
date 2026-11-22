# Shared runtime behaviour

Version 0.1. What every language package does the same way, so a Weft service in one language and a Weft service in another agree on the wire. The packages implement this over the [IR](ir.md); the [conformance suite](../docs/07-conformance.md) checks it. This file grows as the packages do. It covers what the TypeScript package (phase P2) has built.

## Wire format (JSON)

A value has two forms: the **code form** a handler sees, and the **wire form** that travels. The runtime converts between them from the IR.

| Contract type | Code form (TypeScript) | Wire form |
| --- | --- | --- |
| `string` | `string` | JSON string |
| `bool` | `boolean` | JSON boolean |
| `int`, `int64` | `number`, or `bigint` with `codegen.int64: bigint` | JSON number when the value is a safe integer (±2^53 − 1), otherwise a decimal string. A receiver accepts both |
| `int32` | `number` | JSON number |
| `float`, `float32`, `float64` | `number` (finite) | JSON number |
| `decimal` | `string` | JSON string, such as `"12.50"` |
| `uuid` | `string` | JSON string in the 8-4-4-4-12 form |
| `timestamp` | `Date` | RFC 3339 string in UTC with milliseconds, such as `"2026-11-23T10:00:00.000Z"`. A receiver also accepts any offset and any number of fraction digits |
| `bytes` | `Uint8Array` | Base64 (RFC 4648, with padding) |
| enum | the variant name | JSON string, the variant name |
| record | object, keys are field names | object, keys are each field's `wireName` |
| `T[]` | array | array |
| map | object | object. Keys are strings; `int` keys are written as decimal strings |

Records:

- An optional field may be missing or `null`. On the wire a missing optional field is left out, never written as `null`. A receiver accepts both.
- A field that is not in the contract is ignored by a receiver and not passed on.
- Required fields must be present and not `null`.

## Validation

The runtime validates a value when it encodes one to send and when it decodes one it received. Failures are collected, not stopped at the first. Each issue has a path (`items[0].qty`, using wire names on decode and code names on encode) and a message. A failed validation is an `InvalidInput` error carrying all issues.

| Rule | Meaning |
| --- | --- |
| `@min`, `@max` | Inclusive, on the numeric value (a `decimal` is compared as a number) |
| `@length` | Inclusive. Strings count Unicode code points, `bytes` count bytes, lists count items, maps count keys |
| `@pattern` | Not anchored: it matches if the pattern is found anywhere. Write `^` and `$` to match the whole value |
| `@format` | `email`, `url` (absolute, `http` or `https`), `uri` (any absolute URI), `hostname` (RFC 1123), `ipv4`, `ipv6`, `date` (`YYYY-MM-DD`, a real day), `time` (`HH:MM:SS`, optional fraction, optional `Z` or offset) |

Sized integers are range checked: `int32` is −2^31 to 2^31 − 1, `int` and `int64` are −2^63 to 2^63 − 1. An enum value that is not a variant of the receiver's contract is an issue.

## Errors

Eight categories, in every language. Contract-declared errors carry their own status and fields; the categories below are the built-in ones.

| Category | HTTP | gRPC | In messaging |
| --- | --- | --- | --- |
| `NotFound` | 404 | `NOT_FOUND` | not retried |
| `InvalidInput` | 400 | `INVALID_ARGUMENT` | sent to the dead-letter destination |
| `Unauthorized` | 401 | `UNAUTHENTICATED` | not retried |
| `Forbidden` | 403 | `PERMISSION_DENIED` | not retried |
| `Conflict` | 409 | `ALREADY_EXISTS` | not retried |
| `Unavailable` | 503 | `UNAVAILABLE` | retried |
| `Timeout` | 504 | `DEADLINE_EXCEEDED` | retried |
| `Internal` | 500 | `INTERNAL` | retried, then sent to the dead-letter destination |

## Configuration

Every package reads `weft.yaml` ([the CLI doc](../docs/06-cli.md#config-file-weftyaml)) the same way:

- `${NAME}` in a value reads the environment variable. A variable that is not set is a configuration error, and every such problem is reported together.
- These variables override the file, and only for settings that exist or are named this way:

| Variable | Overrides |
| --- | --- |
| `WEFT_SERVICE` | `service` |
| `WEFT_<TRANSPORT>_<KEY>`, such as `WEFT_KAFKA_BROKERS` | `transports.<transport>.<key>`. The transport must be in the file. A key matches an existing one ignoring case and underscores. The value is converted to the type of the setting it replaces (number, boolean) |
| `WEFT_SERVICES_<NAME>` | `services.<name>` |
| `WEFT_TELEMETRY_<KEY>` | `telemetry.<key>` |

- `topics:` maps an event name to the real topic or queue name; an event with no entry uses its own name.
- `service` is required.

## Lifecycle

- Handlers and transports are registered first. `start()` then connects each transport in the order registered; if one fails, the ones already started are closed and `start()` fails with `Unavailable`.
- `stop()` stops taking new work in every transport, waits for in-flight handlers up to the grace period (30 s by default), then closes the transports in reverse order. A handler still running when the grace period ends is abandoned: its signal is aborted and the transports are closed anyway. `stop()` reports how many were abandoned and is safe to call more than once.
- New work after `stop()` begins is refused with `Unavailable`.
- Middleware runs outermost first: the middleware added to Weft, then the transport's own, then the handler. A middleware continues by calling `next()` once; not calling it stops the chain.
- A deadline only ever gets shorter as it is passed on. A timed-out call fails with `Timeout`; a cancelled one with `Unavailable`.

## Messaging

Core does everything that is the same across libraries; an adapter only translates to and from one library ([SDK design](../docs/04-sdk-design.md#the-adapter-interface)).

**Envelope.** Every message is a CloudEvents 1.0 envelope in binary mode: `id` (a UUID), `type` (the event name), `source` (the service name from `weft.yaml`), `time` (RFC 3339) and the content type travel as headers; the body is the encoded payload, JSON by default. The attribute prefix depends on the transport (`ce_` for Kafka). The ordering key is the event's declared `key:` (its fields joined with `:`) unless the call gives one. Other headers, such as the idempotency key (`weft-idempotency-key`), are copied as they are.

**Topics.** The destination is the `topics:` entry for the event, else the event name.

**Publishing.** The payload is validated and encoded first; nothing is sent if it is invalid. A send that fails with `Unavailable` or `Timeout`, or with an error the library raised, is retried with the backoff below, up to 5 attempts. Other categories fail at once. An option the library cannot honour (`delayMs`, headers, a consumer group) is an `Internal` error that names it, not silently ignored.

**Receiving.** The body is decoded and validated against the event's contract; a message that does not match goes straight to the dead-letter destination. A handler that returns normally acknowledges the message (at-least-once). When it throws, the category decides ([Errors](#errors)):

| Category | What happens |
| --- | --- |
| `NotFound`, `Unauthorized`, `Forbidden`, `Conflict` | Acknowledged and logged. Not retried, not dead-lettered |
| `InvalidInput` | Dead-lettered at once |
| `Unavailable`, `Timeout`, `Internal`, or any other exception (treated as `Internal`) | Retried in place, in order, until the attempts run out, then dead-lettered |

**Retries.** 5 attempts by default (`attempts`), waiting between them `min(30 s, 200 ms × 2^(n−1))`, with a random amount between half and all of that. The handler is retried before the next message is taken, so order within a partition holds.

**Dead letters.** A failed message is sent, unchanged, to `<destination>.dlq` (or the `deadLetter` option; `false` drops it with a log line), with the headers `weft-original-destination`, `weft-error-category`, `weft-error-message` and `weft-attempts`.

**Shutdown.** Once `stop()` begins, a message that is mid-retry is not acknowledged and not dead-lettered, so the broker delivers it again after a restart.

**Duplicates.** The `dedupe()` middleware skips a message whose idempotency key (else its `id`) was already handled successfully. It remembers keys in memory only.

## Observability

Weft emits OpenTelemetry through the OpenTelemetry **API** only. With no SDK installed nothing is recorded and nothing is slowed; the application installs an SDK and an exporter (the `telemetry.otlp` setting is for the application's own setup and the dashboard).

**Trace context.** W3C Trace Context (`traceparent`, `tracestate`) is written into the message headers on publish and read from them on receive, so one trace runs from the publisher through every consumer. HTTP and other adapters do the same with their own headers.

**Spans.**

| Span | Kind | When |
| --- | --- | --- |
| `<destination> publish` | producer | Each publish, parent is the active span |
| `<destination> process` | consumer | Each attempt to handle a message, parent is the publisher's span. A failed attempt is marked as an error with the exception and `weft.error.category` |
| `<destination> publish` | producer | A dead-letter send, in the original message's trace |

Attributes: `messaging.system` (the transport, such as `kafka`), `messaging.destination.name`, `messaging.operation.type` (`publish` or `process`), `messaging.message.id`, `messaging.message.body.size`, `weft.event`, and on `process` spans `weft.attempt`.

**Metrics** (meter `@weft-sdk/core`), each with `event` and `transport` attributes:

| Name | Kind | What |
| --- | --- | --- |
| `weft.messaging.messages.sent` | counter | `outcome`: `ok` or `error` |
| `weft.messaging.messages.received` | counter | `outcome`: `ok`, `retry`, `dead_letter`, `dropped` (not retried) or `not_acked` (shutdown) |
| `weft.messaging.errors` | counter | `category`: one of the eight |
| `weft.messaging.process.duration` | histogram, ms | One per handler attempt, with `outcome` |
| `weft.messaging.message.size` | histogram, bytes | `direction`: `send` or `receive` |

## HTTP

Core serves and calls HTTP; an adapter only owns the socket and the router.

**Routes.** A handler is registered by method and path and must match a route of the contracts, by its full path (`/v1/orders`) or its path without the base (`/orders`) when that is not ambiguous. Anything else is an error at registration, not at the first request.

**Reading a request.** Path parameters, the query string, headers and the JSON body are decoded and validated against the route. Text values are converted to the type the contract names (`true`/`false` for `bool`, numbers for `float` and `int`). Every problem in all four parts is returned at once, with a path starting `params.`, `query.`, `headers.` or `body.`. A missing or unreadable body is `body is required` or `body is not valid JSON`. Header names are matched without regard to case, by the field's `wireName`.

**Writing a response.** The returned value is validated and encoded. An optional result (`-> Order?`) that is missing is a 404. A value that does not match the contract is a server bug: `Internal`, 500, never sent. Handlers may set `ctx.response.status` and `ctx.response.headers`. Success is `application/json`.

**Errors** are [RFC 9457](https://www.rfc-editor.org/rfc/rfc9457) problem details (`application/problem+json`):

```json
{ "type": "urn:weft:error:NotFound", "title": "NotFound", "status": 404, "detail": "No such order" }
{ "type": "urn:weft:error:InvalidInput", "title": "InvalidInput", "status": 400, "detail": "Invalid input: ...", "issues": [ { "path": "body.qty", "message": "..." } ] }
{ "type": "urn:weft:error:orders.OutOfStock", "title": "OutOfStock", "status": 422, "detail": "...", "fields": { "sku": "apple", "available": 0 } }
```

The status of a category is in [Errors](#errors); a declared error has its contract's status (422 unless `@status`). A declared error a route does not list, or any other exception, is `Internal` with the detail "Internal server error": the real message is logged, never sent. The client turns a problem back into the same error: a `WeftError` with its category, or a `DeclaredError` with its fields.

**Calling.** `to("orders")` reads the base URL from `services:`. The request is built from the contract (path parameters, query, headers, body), so a call that breaks it fails with `InvalidInput` before anything is sent. Every call has a timeout (10 s by default) and sends `weft-deadline` (milliseconds since the epoch) and `traceparent`. A call is retried, up to 3 attempts with the backoff above, only when it failed with `Unavailable` or `Timeout` **and** it is safe to repeat: `GET`, `HEAD`, `PUT`, `DELETE`, `OPTIONS`, or any call given an `idempotencyKey` (sent as `Idempotency-Key`). A 404 for an optional result is `undefined`.

**Shutdown.** A request in flight when `stop()` begins finishes and its response is written; connections are closed as they go idle, and the rest after a short grace. A request that arrives after `stop()` began is refused with 503.

**Observability.** A server span `<METHOD> <route>` (parent from `traceparent`) and a client span per attempt, with `http.request.method`, `http.route`, `http.response.status_code`; histograms `weft.http.server.duration` and `weft.http.client.duration` (ms).

## Kafka

The Kafka adapter (`@weft-sdk/kafka-kafkajs`, on KafkaJS) follows [Messaging](#messaging) and adds:

- **Attributes.** CloudEvents attributes are `ce_` headers (`ce_id`, `ce_type`, `ce_source`, `ce_time`, `ce_datacontenttype`, `ce_specversion`); the ordering key is the Kafka key; other headers are Kafka headers. A message that was not sent by Weft (no `ce_` headers) is still delivered: its `id` is `topic:partition:offset` and its `type` is the topic.
- **Settings.** `transports.kafka`: `brokers` (a list, or a comma-separated string), `group` (the default consumer group), `clientId` (default: the service name). An option on `on()` overrides the group. `fromBeginning` applies when the group has no committed offset yet.
- **Acknowledging.** A message's offset is committed when its handler returns. A handler that throws (because shutdown began mid-retry) leaves it uncommitted, and the group's next consumer receives it again.
- **Topics.** Before subscribing, the adapter creates the topics it will consume if they do not exist (a consumer that subscribes to a missing topic fails). Setting `createTopics: false` skips this. Topics it only publishes to, such as dead-letter topics, are created by the broker on first send, which needs `auto.create.topics.enable` (the default).
- **Retries hold up the partition.** A message being retried is retried before the next one is taken, so keep the total retry time well under the consumer's session timeout (30 s by default) or the group will rebalance.
- **Not supported:** `delayMs`, transactions, request-reply. They fail with a clear `Internal` error.
- **Known noise.** KafkaJS 2.2.4 prints Node's `TimeoutNegativeWarning` on recent Node versions. It comes from KafkaJS's request queue and is harmless.

The Go adapter (`go/kafka/franz`, on franz-go) behaves the same, and **sends uncompressed** unless told otherwise (`ExtraOptions`): franz-go compresses with Snappy by default, which a KafkaJS consumer cannot read without a plugin, so Go-to-TypeScript messages were unreadable until the cross-language cases found it. It also: the same `ce_` headers and key, so a Go service and a TypeScript service read each other's messages; a group that has committed nothing starts at the end unless `fromBeginning` is set; the offset is committed after the handler returns, and a message that shutdown interrupted is left uncommitted.
