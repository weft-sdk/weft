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
├── contracts/        # .weft files the cases use
├── cases/            # YAML cases, language-neutral
├── runners.yaml      # how to start each language's runner
└── (the harness is the crate compiler/crates/weft-conformance, in the compiler workspace)
```

The **harness** (`weft-conformance run`, in Rust) compiles the contracts to the [IR](../spec/ir.md), starts each language's **runner**, hands it the IR, sends it every case, and compares the answers. A runner is a small program that wraps its language's package and speaks a JSON-lines protocol on standard input and output ([spec/conformance-protocol.md](../spec/conformance-protocol.md)). Because the harness gives the runner the IR, a runner needs no generated code.

A case (so far, encoding, validation, configuration and messaging):

```yaml
- id: encoding.timestamp.positive-offset-becomes-utc
  type: timestamp                 # or conformance.Order, int[], map<string, int>, conformance.Page<int>
  wire: "2026-11-23T10:00:00+02:00"
  expect: { canonical: "2026-11-23T08:00:00.000Z" }   # valid, and this is the wire form after a round trip
- id: validation.minmax.below
  type: conformance.MinMax
  wire: { n: 9 }
  expect: { issues: [n] }          # invalid, with exactly these issue paths
```

The runner decodes the wire value as the type, then encodes the result again; the case says what must come back. A configuration case gives the text of a `weft.yaml` and an environment, and expects the resulting config or the problems. A messaging case is a scripted scenario (a handler that fails twice, then a publish) on an in-memory broker, with the waits recorded, that expects exact numbers of calls, waits and dead letters. Messages may differ between languages, issue paths may not. Cases that involve more than one service or a broker (retries, dead letters, tracing across a transport) come with the messaging and HTTP work in P3.

```
make conformance          # builds the TypeScript runner, then runs every case against every runner
weft-conformance run --runner typescript --filter encoding.int --results results.json
```

Where the infrastructure is needed (Kafka for the messaging cases), it runs in Docker.

**What exists today.** The harness, the protocol, runners for TypeScript and Go, and 220 cases (encoding, validation, `weft.yaml` configuration, and messaging scenarios on an in-memory broker) that both pass, which is what "the same in every language" means so far. The harness itself is tested with runners that give wrong answers on purpose. No other language has a runner yet, and the messaging, HTTP, tracing and shutdown cases in the table above are not written.

## In CI

- Every pull request runs the cases for the languages and adapters it touches.
- Nightly runs the full matrix: every language × every adapter × every case.
- Results are published as a public compatibility table on the website, so users can see exactly what's verified.
