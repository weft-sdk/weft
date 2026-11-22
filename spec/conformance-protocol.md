# The conformance runner protocol

Version 0.1. How the [conformance harness](../docs/07-conformance.md) drives a language's **runner**: a small program that wraps that language's Weft package. The harness compiles the contracts to the [IR](ir.md) itself and hands the IR to the runner, so a runner needs no generated code and no compiler.

## Transport

The harness starts the runner as a process and talks over its standard input and output, one JSON document per line (UTF-8, no pretty-printing). The runner writes nothing else to standard output; logs go to standard error. A runner exits when its standard input closes.

Every request has an `id` (a number) and an `op`. Every response repeats the `id`:

```json
{ "id": 1, "ok": true, "result": { } }
{ "id": 1, "ok": false, "error": "what went wrong" }
```

`ok: false` is for a request the runner could not carry out (unknown op, bad arguments). A value that fails validation is **not** an error: it is a successful `decode` whose result says so.

## Operations

| `op` | Request fields | Result |
| --- | --- | --- |
| `hello` | none | `{ "language": "typescript", "version": "0.0.0", "protocol": "0.1" }` |
| `load` | `ir`: an array of IR documents | `{}`. Replaces the contracts the runner knows. Every later operation uses them |
| `decode` | `type`: a type reference as in the IR; `wire`: any JSON value | See below |
| `config` | `yaml`: the text of a weft.yaml; `env`: an object of environment variables | See below |
| `messaging` | A scenario: see below | What happened, see below |
| `http` | A scenario: see below | What was answered, see below |

### `decode`

The runner decodes `wire` as `type`, then, if it was valid, encodes the result again.

```json
{ "id": 3, "op": "decode", "type": { "kind": "named", "package": "conformance", "name": "Order" }, "wire": { } }
{ "id": 3, "ok": true, "result": { "valid": true, "canonical": { } } }
{ "id": 3, "ok": true, "result": { "valid": false, "issues": [ { "path": "lines[0].qty", "message": "..." } ] } }
```

- `canonical` is the wire form after the round trip: names are wire names, optional fields that were missing or `null` are left out, fields the contract does not have are gone, timestamps are in UTC with milliseconds, and so on ([shared behaviour](behaviour.md#wire-format-json)).
- `issues` lists every problem. A case compares the **paths** (using wire names, as in `behaviour.md`); the messages are for people and may differ between languages.

### `config`

The runner reads the file as it would at start-up, with `env` as the environment ([configuration](behaviour.md#configuration)).

```json
{ "id": 4, "op": "config", "yaml": "service: orders", "env": { } }
{ "id": 4, "ok": true, "result": { "valid": true, "config": { "service": "orders", "transports": { }, "topics": { }, "services": { }, "telemetry": { } } } }
{ "id": 4, "ok": true, "result": { "valid": false, "problems": [ "service is required" ] } }
```

A case compares `config` as a value, and `problems` by count and by text: each expected piece of text must be in a problem, and there must be no other problems. The wording of a problem may differ between languages.

### `messaging`

One scripted scenario on an **in-memory broker**, run inside the runner with its language's package. The jitter of the retry backoff is fixed at one half and the waits are recorded and skipped, so the backoff is exact (150, 300, 600, 1200 ms for the first four failures). The event is subscribed with a handler that fails as scripted, then published to.

| Request field | Meaning |
| --- | --- |
| `event` | The event name, such as `order.placed`. The service is called `conformance` |
| `payload` | The event's payload in wire form. If it does not decode, nothing is published and `publishError` is `InvalidInput` |
| `rawBody` | Instead of `payload`: text put straight on the broker, bypassing validation, as another service might |
| `topic` | The real topic name, as if `topics:` in weft.yaml said so |
| `handler.fail` | `{ times: 2 or "always", category: one of the eight, or "Plain" }`. `Plain` is an ordinary exception, not a Weft error |
| `subscribe` | `{ attempts, deadLetter }`: `deadLetter` is a name, or `false` to drop |
| `publish` | `{ key, idempotencyKey, headers, delayMs, times }`: `times` publishes that often, waiting for each to be handled |
| `dedupe` | Use the dedupe middleware |
| `stopDuringBackoff` | The waits block until shutdown begins, and the service is stopped as soon as the handler has failed once |

The result:

| Field | Meaning |
| --- | --- |
| `publishError` | The category of the error `publish` raised, or `null` |
| `calls`, `attempts` | How many times the handler ran, and the `attempt` it was given each time |
| `sleeps` | The waits between attempts, in milliseconds |
| `received` | The payload as the handler last handled it successfully, in wire form, or `null` |
| `destinations` | Every destination a message was sent to, in order (dead letters included) |
| `envelope` | The envelope of the first message sent: `type`, `source`, `datacontenttype`, `key` (or `null`), `hasId`, `hasTime`, `headers` |
| `dlq` | Each dead letter: `destination`, `category`, `attempts`, `originalDestination` (the `weft-*` headers), and `sameId`: it kept the message's id |
| `unacked` | Messages the broker never saw acknowledged |

A case lists only the result fields it cares about, and for objects and arrays only the parts it names (arrays must be the same length).

### `http`

One request to one route, run through the runner's HTTP handling with an adapter that captures the route instead of listening on a socket. The handler is scripted.

| Request field | Meaning |
| --- | --- |
| `route` | The operation name in the contracts, such as `createOrder` |
| `request` | `{ params, query, headers, body }`. `params` and `headers` map names to text; `query` maps a name to text or a list of text; `body` is the raw body text, or absent |
| `handler.returns` | The result, in wire form. `null` means "nothing" |
| `handler.returnsRaw` | A value returned as it is, without being checked against the contract (to try the server's refusal to send it) |
| `handler.throws` | `{ category }` (one of the eight), `{ plain: true }` (an ordinary exception whose message contains a secret), or `{ declared: { name, fields, message } }` |
| `handler.status`, `handler.headers` | Set on the response by the handler |

The result:

| Field | Meaning |
| --- | --- |
| `status`, `contentType`, `body` | The response. `contentType` is without parameters, `body` is parsed JSON or `null` |
| `issues` | The sorted `path`s of the problem body's `issues`, or `null` |
| `headers` | The response headers other than `content-type`, in lower case |
| `calls` | How many times the handler ran. 0 when the request was refused |
| `seen` | What the handler was given, in wire form: `{ params, query, headers, body }`, each `null` when the route has none |
| `leaksSecret` | Whether the response contains the secret that the `plain` exception carried |

New operations are added with the conformance cases that need them, and listed here.
