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

Detects the language and package manager: npm, pnpm, yarn, bun, deno, go modules, pip, uv, poetry, Maven, Gradle, dotnet, cargo, bundler, composer, mix, SwiftPM, pub. Only offers libraries that exist for that language ([05-languages.md](05-languages.md)).

```
weft init [--yes] [--name NAME] [--lang LANG] [--contracts PATH] [--http [LIBRARY]] [--kafka [LIBRARY]] ...
```

- **Non-interactive.** `weft init --yes --http express --kafka kafkajs`. Every transport has a flag (`--http`, `--gql`, `--grpc`, `--rpc`, `--ws`, `--sse`, `--kafka`, `--mq`, `--nats`, `--redis`, `--cloud`, `--mqtt`, `--pulsar`) that takes a library, or none for the default. With `--yes` and no transport flag it sets up HTTP. Without `--yes` it asks, and any flag you pass answers its question. Without a terminal it needs `--yes`.
- **The service name** is also its contract package: lower-case letters, digits and `_`, starting with a letter. It defaults to the folder name.
- **The example contract** uses the transports you chose: an event for the messaging ones, a route for HTTP, and so on. It is already formatted and checked, and `weft gen` runs on it before `weft init` finishes.
- **It never overwrites.** If `weft.yaml`, the example contract or the generated file exists, it stops before writing anything. It also checks the whole setup in memory first, so a name the language cannot hold fails with nothing left behind.
- **Languages.** TypeScript and Go. A folder with a `go.mod` is detected as Go (one with a `package.json` is TypeScript even if it also has a `go.mod`; `--lang go` overrides). For Go, `weft init` writes `weft.gen.go` and offers the Go libraries ([05-languages.md](05-languages.md)); only `nethttp` for `http` and `franz` for `kafka` have adapters yet, and the generator skips the rest and says so.
- **Installing packages.** It runs the project's own package manager (`pnpm add ...`; for Go, `go get github.com/weft-sdk/weft/go@latest`, one module that holds the core and every adapter). With no package manager, or `--no-install`, it prints the command. A failed install never fails `weft init`: the files are written and it says what to run. The Go module needs its release tag before `go get` works (see the [npm runbook](runbooks/publish-npm.md#not-covered-here)).

### `weft gen`

```
weft gen [--lang LANG] [--out PATH] [--check] [--watch]
```

Reads the contracts named in `weft.yaml`, checks them, and writes the typed code for one language.

- **Finding the config.** `weft.yaml` is looked for in the current folder and then each folder above it. Relative paths in it are relative to the file.
- **Language.** `--lang`, or `language:` in `weft.yaml`.
- **Contracts.** The `contracts:` entries (a folder, default `./contracts`). Every `.weft` file in them is checked together, so packages can import each other. A failing contract writes nothing and exits with 3.
- **Output.** `--out`, or `codegen.out` (default `src/weft.gen.ts`). The file is deterministic, and is left alone when it would not change.
- **Languages.** TypeScript and Go. `weft gen --lang go` writes `weft.gen.go` (default; `codegen.out` changes it), in the package named by `codegen.package` or, if that is not set, by the folder it is written to.
- **The typed `weft` object.** For the transports in `weft.yaml` that have a package for the language (TypeScript: `http` on `express`, `kafka` on `kafkajs`; Go: `http` on `nethttp`, `kafka` on `franz`), the file also wires them up and types them from the contracts, using `service:` to know which package's routes this service serves. [SDK design](04-sdk-design.md#typescript) has the details.
- `--check` writes nothing and exits with 1 if the file is missing or out of date (for CI).
- `--watch` runs once, then keeps running and regenerates whenever `weft.yaml` or a contract file changes (saved, added or removed). It waits until the files stop changing, so a half-written save does not trigger a run. A broken contract is reported and the watcher keeps going; the next good save regenerates. It looks at file times a few times a second rather than using the system's file events, so it behaves the same everywhere. Stop it with Ctrl-C. Not combined with `--check`; with no `weft.yaml` it exits with 2.

For TypeScript the file holds the types of every package (one `namespace` per package, so `orders.Order`), a per-package map from event name to payload type, and the checked contracts as a constant for the runtime to read ([ADR 0021](decisions/0021-generated-code-embeds-the-ir.md)).

### `weft check`

```
weft check [--baseline REF | --baseline-dir PATH] [--json]
```

Compares the current contracts with a baseline and lists every change as **safe**, **risky** or **breaking** ([the rules](02-contract-language.md#evolving-contracts-safely)). Each breaking change comes with what to do instead (usually: deprecate first, remove in the next major version).

```
$ weft check
Comparing with git v1

orders (version 1.0.0)
  breaking  Order.note  removed field `note` from `Order`
                        hint: mark it `@deprecated` first, and remove it in the next major version
  1 breaking, 0 risky, 0 safe
  ✗ breaking changes need a major version bump; the version is 1.0.0
```

- **Baseline.** `--baseline` takes a git ref (a tag, branch or commit). The default is the latest tag. The baseline's contract files are read with `git show`, so the working tree is never touched. `--baseline-dir` compares with a folder of older contracts instead. A contracts folder that did not exist at the baseline counts as all new.
- **Exit codes.** 0 when no breaking change is left uncovered by the version, 1 when one is, 3 if the current or the baseline contracts have errors, 2 for usage errors (no tag, unknown ref).
- **`--json`.** Writes the result as JSON on standard output, and nothing else there (errors stay on standard error). The exit codes are the same. Every package that was compared is listed, with its changes breaking first:

```json
{
  "baseline": "git v1",
  "passes": false,
  "packages": [
    {
      "package": "orders",
      "oldVersion": "1.0.0",
      "newVersion": "1.0.0",
      "required": "major",
      "actual": null,
      "passes": false,
      "counts": { "breaking": 1, "risky": 0, "safe": 0 },
      "changes": [
        { "severity": "breaking", "location": "Order.note", "message": "removed field `note` from `Order`", "hint": "mark it `@deprecated` first, and remove it in the next major version" }
      ]
    }
  ]
}
```

  `required` is the smallest bump the changes call for (`none`, `patch`, `minor` or `major`), `actual` the bump the versions show (or `null`), and `hint` is only there for breaking changes.
- Comparing with a published version (`weft pull`, the dashboard) is added with those commands.

### `weft fmt`

```
weft fmt [PATH...] [--check]
```

Formats `.weft` files in place, in the one style described in [spec/language.md](../spec/language.md#style). A path is a file, or a folder that is searched for `.weft` files (hidden folders, `node_modules` and `target` are skipped). With no path it formats the current folder. `--check` changes nothing, lists the files that would change and exits with 1 (for CI). A file with a syntax error is reported, the other files are still formatted, and the exit code is 3.

### `weft dashboard`

```
$ weft dashboard
Weft dashboard running at http://localhost:4747
Receiving telemetry on   http://localhost:4318 (OTLP)
```

Options: `--port`, `--contracts PATH`, `--data-dir`, `--open`. See [08-dashboard.md](08-dashboard.md).

## Config file: `weft.yaml`

One per service. The same keys in every language.

```yaml
service: orders
language: typescript

contracts:
  - ./contracts                          # this service's own packages
  - git: github.com/larder/contracts     # packages from other services
    packages:
      orders: ^2.3
      payments: ^1.0

codegen:
  out: src/weft.gen.ts
  int64: number            # or bigint, for the full 64-bit range

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

topics:                    # optional: map event names to real topic / queue names
  order.placed: prod.orders.placed.v1

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
