# Architecture

## The pieces

```
            .weft contracts
                   │
                   ▼
   ┌───────────────────────────────┐
   │  weft CLI (Rust)            │
   │  parser → checker → IR        │──► weft check (breaking changes)
   │  codegen per language         │──► weft.gen.ts / weft_gen.go / ...
   │  init, LSP, dashboard server  │
   └───────────────────────────────┘
                   │ generated code
                   ▼
   ┌───────────────────────────────────────────────┐
   │  Service (any language)                        │
   │    weft.* facade (generated)                 │
   │    Weft core (native package)                │
   │    adapters ──► KafkaJS / Express / Socket.io  │
   └───────────────────────────────────────────────┘
                   │ OpenTelemetry
                   ▼
            Weft dashboard (local / self-hosted / cloud)
```

1. **Contracts** describe data and operations once ([02-contract-language.md](02-contract-language.md)).
2. **The compiler and CLI** check contracts, catch breaking changes, and generate code for each language. Written once, in Rust.
3. **Language packages** give each language a native `weft.*` API that runs the real libraries underneath ([04-sdk-design.md](04-sdk-design.md)).
4. **The conformance suite** proves every package behaves the same ([07-conformance.md](07-conformance.md)).
5. **The dashboard** shows the map and live traffic ([08-dashboard.md](08-dashboard.md)).

## Principles

- **Wrap, don't replace.** Weft runs the libraries teams already use. It never reimplements a Kafka client or an HTTP server.
- **Contracts are the source of truth.** Every language's types come from the same file.
- **Native in every language.** A Go developer gets Go, not a Rust library with Go bindings.
- **Shared once, thin everywhere.** Everything that can be shared (contracts, codegen, checks, tests, dashboard) is written once. Language packages stay small.
- **Never trap the user.** The underlying library is always reachable through `raw`.
- **Not in the request path.** No proxy, no sidecar. The dashboard is optional and passive.

## Compiler internals

```
.weft files ─► lexer/parser ─► AST ─► name resolution + type checking ─► IR (JSON)
                                                                        ├─► codegen: typescript, go, python, ...
                                                                        ├─► check: diff two IRs
                                                                        ├─► map: static HTML
                                                                        └─► LSP: diagnostics, completion, hover
importers: OpenAPI, proto, GraphQL SDL, AsyncAPI, Avro, JSON Schema ─► IR
```

The **IR** (intermediate representation) is a stable, versioned JSON format. Code generators, the dashboard and third-party tools read the IR, never the `.weft` syntax. This lets the syntax evolve without breaking anything downstream.

Each code generator is a set of templates plus a small amount of Rust for naming and type mapping. Adding a language means adding a generator, a core package, and adapters.

## Repo layout

One monorepo.

```
weft/
├── compiler/            # Rust workspace
│   ├── weft-syntax/   # lexer, parser, AST, formatter
│   ├── weft-check/    # name resolution, type checking, IR
│   ├── weft-ir/       # IR types + JSON schema
│   ├── weft-compat/   # breaking-change detection
│   ├── weft-codegen/  # one module + templates per language
│   ├── weft-import/   # OpenAPI, proto, SDL, AsyncAPI, Avro, JSON Schema
│   ├── weft-lsp/      # language server
│   ├── weft-init/     # interactive setup, package-manager integration
│   └── weft-cli/      # the `weft` binary
├── spec/
│   ├── language.md      # contract language spec
│   ├── ir.schema.json
│   └── behaviour.md     # shared runtime behaviour (retries, envelopes, errors)
├── sdks/
│   ├── typescript/      # core + adapters (pnpm workspace)
│   ├── go/
│   ├── python/
│   ├── jvm/
│   ├── dotnet/
│   ├── rust/
│   ├── ruby/
│   ├── php/
│   ├── elixir/
│   ├── swift/
│   └── dart/
├── conformance/         # cases, harness, docker-compose
├── dashboard/
│   ├── server/          # Rust (axum)
│   └── ui/              # React + Vite
├── editors/
│   ├── vscode/
│   ├── jetbrains/
│   └── tree-sitter-weft/
├── website/             # Astro: landing page + docs
└── docs/                # these design docs
```

See [10-tech-stack.md](10-tech-stack.md) for the tools in each part.
