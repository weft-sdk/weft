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
