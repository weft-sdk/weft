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

New operations are added with the conformance cases that need them, and listed here.
