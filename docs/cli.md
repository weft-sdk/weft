# CLI

One binary, `weft`, written in Rust. It works the same in every project, whatever language.

Install: Homebrew (`brew install weft-sdk/tap/weft`), `npx @weft-sdk/cli`, Scoop, Docker, or a curl script. Each language package can also pin and download the binary for its users.

## Commands

| Command | Purpose |
| --- | --- |
| `weft init` | Pick transports and libraries, install packages, write `weft.yaml` |
| `weft add <transport>` | Add a transport to an existing project |
| `weft gen` | Generate typed code from contracts |
| `weft check` | Detect breaking contract changes |
| `weft fmt` | Format `.weft` files |
| `weft import <file>` | Convert OpenAPI, `.proto`, GraphQL SDL, AsyncAPI, Avro or JSON Schema to `.weft` |
| `weft publish` | Push contracts to the dashboard / Weft Cloud |
| `weft pull <package>` | Fetch another service's published contracts |
| `weft map` | Write a static HTML map of services from contracts |
| `weft dashboard` | Run the dashboard locally on a port |
| `weft doctor` | Check config, connections to brokers, and package versions |

### `weft init`

```
$ weft init
Detected: TypeScript (pnpm)
? Service name               › orders
? Which communication do you use?
  ◉ HTTP   ◉ GraphQL   ◉ Kafka   ◉ WebSockets   ◯ gRPC   ◯ RabbitMQ   ◯ NATS   ◯ more…
? HTTP library?              › Express        (Fastify, Hono, NestJS)
? GraphQL library?           › Apollo Server  (GraphQL Yoga)
? Kafka library?             › KafkaJS        (confluent-kafka-javascript)
? WebSocket library?         › Socket.io      (ws)
? Where do contracts live?   › ./contracts    (git repo, Weft Cloud)

✓ pnpm add @weft-sdk/core @weft-sdk/http-express @weft-sdk/gql-apollo @weft-sdk/kafka-kafkajs @weft-sdk/ws-socketio
✓ Wrote weft.yaml
✓ Wrote contracts/orders.weft (example)
✓ Wrote src/weft.gen.ts

Next: edit contracts/orders.weft, then run `weft gen`.
```

Detects the language and package manager: npm, pnpm, yarn, bun, deno, go modules, pip, uv, poetry, Maven, Gradle, dotnet, cargo, bundler, composer, mix, SwiftPM, pub. Only offers libraries that exist for that language ([languages.md](languages.md)). Non-interactive: `weft init --yes --http express --kafka kafkajs`.

### `weft gen`

```
weft gen [--lang LANG] [--out PATH] [--check] [--watch]
```

Reads contracts, writes typed code. `--watch` regenerates on save. `--check` exits non-zero if generated code is out of date (for CI).

### `weft check`

```
weft check [--against REF|published] [--format text|json|markdown|github|sarif]
```

Compares contracts with a baseline using the rules in [contract-language.md § Evolving contracts](contract-language.md#evolving-contracts-safely). With the dashboard connected, it also lists which services use each affected operation.

```
weft check: 1 breaking change

  orders.order.placed
  ✗ field `total` removed
      used by payments (Go), analytics (Python)
  Suggested: keep `total` with @deprecated, add the new field alongside.
```

### `weft dashboard`

```
$ weft dashboard
Weft dashboard running at http://localhost:4747
Receiving telemetry on   http://localhost:4318 (OTLP)
```

Options: `--port`, `--contracts PATH`, `--data-dir`, `--open`. See [dashboard.md](dashboard.md).

## Config file: `weft.yaml`

One per service. The same keys in every language.

```yaml
service: orders
language: typescript

contracts:
  - ./contracts
  - git: github.com/larder/contracts
    ref: v3

codegen:
  out: src/weft.gen.ts
  int64: bigint            # or number

transports:
  http:
    library: express
    port: 3000
  gql:
    library: apollo-server
    path: /graphql
  kafka:
    library: kafkajs
    brokers: ${KAFKA_BROKERS}
    group: orders
  ws:
    library: socket.io
    path: /ws

services:                  # where other services live, for http/gql/grpc calls
  payments: ${PAYMENTS_URL}
  api: http://api-gateway:4000

telemetry:
  otlp: ${OTEL_EXPORTER_OTLP_ENDPOINT}
  dashboard: http://localhost:4318
```

`${VAR}` reads environment variables. Every key can also be overridden with `WEFT_<SECTION>_<KEY>`, e.g. `WEFT_KAFKA_BROKERS`.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 1 | `check` found breaking changes, or `gen --check` found stale code |
| 2 | Usage or config error |
| 3 | Contract error (syntax or type) |
| 4 | Network error (registry, git, dashboard) |
