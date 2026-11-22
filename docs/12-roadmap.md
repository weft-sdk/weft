# Roadmap

Work is split into phases, and each phase into tasks sized for one pull request. Every task has an id (`W3`, `P1.2`) and lists what it needs. How to pick up a task and hand it off: [14-workflow.md](14-workflow.md).

## Current status

_Describes `main`. Updated in the pull request that finishes a task, for what `main` will be after the merge. Work in progress lives in each branch's work file in [work/](work/)._

| Field | Value |
| --- | --- |
| Phase | P1 (compiler and CLI) done. P2 (TypeScript package) is next. W (website) paused until the backend exists |
| Built | **The website** (`website/`: the landing page and the docs layout with placeholder pages, light and dark themes, Lighthouse 100). **A parser, checker and compatibility checker** (`weft-syntax`, `weft-ir`, `weft-check`, `weft-compat`: they turn contracts into a checked IR, or into located errors, and judge changes between two versions) inside a Rust workspace (`compiler/`) whose CLI has four commands: `weft init`, `weft fmt`, `weft check` and `weft gen` (TypeScript only). No docs content, working compiler, language package or dashboard exists |
| Next task | **P2.6, the Express adapter** (P2.7 and P2.8 can run beside it), then P2.9 ([P2](#p2-typescript-package)). It builds on [ADR 0021](decisions/0021-generated-code-embeds-the-ir.md), accepted 2026-11-23. W6, W7 and the docs content wait for the backend |
| Last done | 2026-11-23: P2.5, core observability; P2.4, core messaging (envelopes, retries, dead letters); P2.3, the core lifecycle, middleware and config; P2.2, the core contracts runtime; and P2.1, the TypeScript workspace skeleton (`sdks/typescript`, CI on three systems). Before that, 2026-11-21: P1.8, `weft init`, which finishes phase P1. `cargo fmt --check`, `clippy` and `test` pass in CI on Linux, macOS and Windows (the Windows run needed the `.gitattributes` line-ending fix), and the website workflow passes |
| Waiting on the maintainer | Nothing blocks P2. Not blocking: trademark search for "Weft", domain (Q3), talking to 5–10 developers about the idea |

## P0: Design documents — done

- [x] **P0.1** The product, contract language, API, language package design, supported languages, CLI, conformance suite, dashboard, architecture, tech stack and website plan are written in [docs/](README.md). Decisions 0001–0016 are accepted.

Done when: the maintainer has read them and they are on `main`. Done.

## W: Website — in progress

The landing page and docs site, built before any product code ([ADR 0016](decisions/0016-website-first.md)). Plan: [11-website.md](11-website.md). The site is written as the launch page of the finished product, and is not deployed publicly until its claims are true ([ADR 0019](decisions/0019-site-written-as-launch-page.md)).

- [x] **W1** Visual direction: references the maintainer likes, palette, type, motion, layout principles. Written into `11-website.md` and a decision record. Needs Maintainer. Done 2026-11-21. Approved direction in [11-website.md](11-website.md#visual-direction-the-terminal).
- [x] **W2** Site skeleton: Astro + TypeScript + pnpm in `website/`, design tokens from W1, base layout, light and dark themes, CI build. Needs W1. Done when: `pnpm --dir website build` passes in CI; an empty styled page renders.
- [x] **W3** Landing page: hero, the problem, how it works (init → contract → call), transports, languages, dashboard preview, call to action (GitHub, follow updates). Needs W2. Done when: All sections render with real copy, work at phone width, and pass a Lighthouse check (performance and accessibility ≥ 90) Done 2026-11-21. Lighthouse is 100 in all four categories, in both themes.
- [x] **W4** Interactive pieces: transport switcher in the hero, persistent language picker, before/after comparison. The before/after comparison is built (W3). The docs language tabs already remember the chosen language. The transport switcher and the site-wide picker are deferred until the docs content is written (maintainer, 2026-11-21). Needs W3. Done 2026-11-21 for what is built; the two deferred pieces are not scheduled.
- [x] **W5** Docs section: custom layout, navigation, search, language tabs, and a page for every planned entry. Page content is written later, once the backend exists. Needs W2. Done 2026-11-21. Layout chosen by the maintainer ([11-website.md](11-website.md#docs-layout-w5)); Lighthouse 100 on both docs pages.
- [ ] **W6** Playground preview: a contract editor showing pre-generated output for a few example contracts in several languages. Needs W4. Done when: Switching examples and languages works without a server.
- [ ] **W7** Launch prep: social images, metadata, sitemap, analytics decision, hosting, domain. Check every claim on the site against shipped code first ([ADR 0019](decisions/0019-site-written-as-launch-page.md)). Needs W3–W6, Q3, and the product claims being true. Done when: The maintainer has deployed it to the chosen domain.

Done when: every task is checked and the maintainer has deployed the site to the chosen domain. W3 to W5 are done; W6 and W7 wait for the backend.

## P1: Compiler and CLI — done

Rust workspace in `compiler/` ([architecture](09-architecture.md#repo-layout), [tech-stack](10-tech-stack.md#compiler-and-cli-rust)). The chain that decides the length is P1.1, P1.2, P1.4, P1.6. P1.3 and P1.5 can run beside it. `weft dashboard` waits for P5; importers, the language server and the WebAssembly build wait for later phases.

- [x] **P1.1** Rust workspace skeleton: `weft-syntax`, `weft-ir`, `weft-check` and `weft-cli` as near-empty crates, a `weft --version` binary, rustfmt and clippy config, a CI job. Rust commands added to CLAUDE.md. Done 2026-11-21. `cargo build`, `test`, `clippy` and `fmt --check` pass locally; the CI workflow passes on Linux, macOS and Windows.
- [x] **P1.2** Parser: lexer, grammar and AST for the whole contract language, `miette` errors. The language spec moves to `spec/language.md`. Needs P1.1. Done 2026-11-21. The complete example parses (`spec/examples/orders.weft`), and bad input gives located errors (snapshot tests).
- [x] **P1.3** IR: versioned JSON types and `spec/ir.schema.json`. Needs P1.1. Done 2026-11-21. `weft-ir` types, `spec/ir.schema.json` generated from them (a test keeps it current), and a sample using every part validates. The checker's output is validated from P1.4 on.
- [x] **P1.4** Checker: name resolution, type checking, operation rules, errors, generics, validation rules, imports; produces the IR. Needs P1.2, P1.3. Done 2026-11-21. The complete example compiles to IR that validates against the schema, packages import each other, and every rule has a failing test (about 100 tests, a few snapshots of the messages). Rules are in [spec/language.md](../spec/language.md#meaning-rules).
- [x] **P1.5** Formatter and `weft fmt`. Needs P1.2. Done 2026-11-21. Formatting is idempotent, keeps every comment, and leaves the IR unchanged. `weft fmt [PATH...] [--check]` works. Style is in [spec/language.md](../spec/language.md#style).
- [x] **P1.6** `weft gen` for TypeScript: type mapping and naming from the docs, reads `weft.yaml`. Needs P1.4. Done 2026-11-21. The generated `weft.gen.ts` type-checks with `tsc --strict` (also with `erasableSyntaxOnly`, and with `int64: bigint`) for the examples, and tests prove the types reject wrong values. The typed facade waits for the TypeScript package (P2). Not built: `--watch`, `--lang js`, contract sources other than local folders. [ADR 0021](decisions/0021-generated-code-embeds-the-ir.md) was accepted 2026-11-23.
- [x] **P1.7** `weft check` (`weft-compat`): diff two IRs into safe, risky and breaking, with the documented exit codes. Needs P1.4. Done 2026-11-21. Every row of the evolving-contracts table has a test, plus the cases the table does not list (about 40 tests in `weft-compat`, 13 through the CLI against real git repositories). Direction-aware rules and the version rule are written into [02-contract-language.md](02-contract-language.md#evolving-contracts-safely). Not built: `--json` output, comparing with a published version.
- [x] **P1.8** `weft init`, non-interactive and interactive. Needs P1.6. Done 2026-11-21. In an empty folder it writes a working `weft.yaml`, a first contract matched to the chosen transports, and the generated code; every transport alone and all together are tested, and the result passes `gen --check`, `fmt --check` and `check`. The interactive prompts were driven by hand through a pseudo-terminal, with no automated test. Package installation waits for P2.

Done when: every task is checked and `cargo fmt`, `clippy` and `test` pass in CI on Linux, macOS and Windows. Done 2026-11-21.

## P2: TypeScript package

The first language package: `@weft-sdk/core`, the Express and KafkaJS adapters, and the typed `weft` facade that `weft gen` writes ([sdk-design](04-sdk-design.md), [api](03-api.md), [ADR 0021](decisions/0021-generated-code-embeds-the-ir.md)). It lives in `sdks/typescript/` as a pnpm workspace ([architecture](09-architecture.md#repo-layout)). Nothing is published: reserving and publishing packages is the maintainer's. The chain that decides the length is P2.1, P2.2, P2.3, P2.4, P2.7, P2.9. P2.5, P2.6 and P2.8 run beside it.

- [x] **P2.1** Workspace skeleton: `sdks/typescript` as a pnpm workspace with an empty `@weft-sdk/core`, strict TypeScript, tsup (ESM and CJS), Vitest, a CI workflow, and `make ts-*` targets. Needs P1. Done when: `make ts-check` builds, type-checks and tests the empty package in CI on Linux, macOS and Windows. Done 2026-11-23 (TypeScript is pinned to 6.0.3 because tsup's declaration build needs the compiler API that TypeScript 7 does not ship).
- [x] **P2.2** Core contracts runtime: reads the embedded IR; validates values against it (types, validation rules, optional and null), maps wire names, encodes and decodes JSON; the eight error categories. Needs P2.1. Done when: values from `spec/examples/orders.weft` round-trip, every validation rule has a passing and a failing test, and errors carry the documented categories. Done 2026-11-23. `Contracts` in `@weft-sdk/core` encodes, decodes and validates (generics, maps, wire names, every rule and format), against a fixture the real compiler generates (`make ts-fixtures`). The wire format and the rules' meaning are written in [spec/behaviour.md](../spec/behaviour.md). 56 tests.
- [x] **P2.3** Core lifecycle: the `weft` object, `start()` and `stop()` with the shutdown grace period, `weft.yaml` and environment configuration with the documented keys, the middleware chain, the request context, call timeouts. Needs P2.2. Done when: handlers registered before `start()` run, `stop()` waits for in-flight work, and config from the file and from `WEFT_*` variables agree with [cli](06-cli.md#config-file-weftyaml). Done 2026-11-23. `Weft` (register, `start`, `stop`, `dispatch`, `track`), `parseConfig` and `loadConfig` (adds the `yaml` dependency), `compose` middleware, `withTimeout` and deadlines. Rules in [spec/behaviour.md](../spec/behaviour.md#lifecycle). 87 tests in all.
- [x] **P2.4** Core messaging: the `MessagingAdapter` interface and `capabilities()`, CloudEvents envelopes, retries with backoff and jitter, dead letters, idempotency keys, an in-memory adapter for tests. Needs P2.3. Done when: publish, consume, retry, dead-letter and dedupe behave as the defaults table says, against the in-memory adapter. Done 2026-11-23. `Messaging` (publish, on, middleware), `MessagingAdapter` with `capabilities()`, CloudEvents header helpers, backoff, `dedupe()` and `MemoryAdapter` in `@weft-sdk/core`; rules in [spec/behaviour.md](../spec/behaviour.md#messaging). 116 tests in all. Not built: validating an event's `headers:` type, request-reply (`weft.nats.request`), transactions.
- [x] **P2.5** Core observability: OpenTelemetry spans and metrics per send and receive, and W3C Trace Context carried through message headers and HTTP. Needs P2.4. Done when: a test collects the spans of a publish and its consume and finds one trace. Done 2026-11-23. Spans, W3C trace context through message headers, and five metrics, emitted through the OpenTelemetry API only (adds `@opentelemetry/api`; the SDK packages are dev dependencies for the tests). `injectTraceContext` and `extractTraceContext` are for the HTTP adapter (P2.6). Rules in [spec/behaviour.md](../spec/behaviour.md#observability). 122 tests in all.
- [ ] **P2.6** `@weft-sdk/http-express`: serve routes from the contracts and call other services with `weft.http.to()`, RFC 9457 problem bodies, `raw` returning the Express app. Needs P2.3. Done when: a request that breaks the contract gets a 400 problem body, and a handler's error category gives the documented status.
- [ ] **P2.7** `@weft-sdk/kafka-kafkajs`: the messaging adapter on KafkaJS (keys, headers, consumer groups, ack after the handler, `raw` returning the Kafka instance). Needs P2.4. Done when: it passes the same adapter tests as the in-memory one, against a real Kafka-compatible broker started by docker compose (opt-in locally, run in CI).
- [ ] **P2.8** The generated facade: `weft gen` writes the typed `weft` object (event names, route paths and payload types from the contracts), keeping the file deterministic. Needs P2.2. Done when: `tsc --strict` accepts correct calls and rejects wrong names and payloads, and `gen --check` is stable.
- [ ] **P2.9** End to end: the Larder `orders` service on Express and KafkaJS using only generated code; `weft init` installs the packages (the `pnpm add` step); docs and README updated to say what exists. Needs P2.6, P2.7, P2.8. Done when: a service serves HTTP and publishes and consumes through one contract, with traces, against a real broker, and a fresh `weft init` project runs.

Done when: every task is checked, the package passes its own tests in CI, and nothing in the docs describes a TypeScript feature that is not built.

## Product phases after P1

Order accepted in [ADR 0020](decisions/0020-product-build-order.md): P1, then P2, then P3. P4 onward are proposed. P2 is broken into tasks above; the rest are not yet:

- [ ] **P3** Conformance suite harness, and the Go package (net/http, franz-go). Done when a TypeScript service and a Go service talk through one contract and pass the cross-language cases. Docs: [07-conformance.md](07-conformance.md)
- [ ] **P4** More transports in TS and Go (GraphQL, gRPC, WebSockets, RabbitMQ, NATS). Docs: [05-languages.md](05-languages.md)
- [ ] **P5** Local dashboard (`weft dashboard`). Docs: [08-dashboard.md](08-dashboard.md)
- [ ] **P6** More languages: Python, JVM, .NET, then the rest. Docs: [05-languages.md](05-languages.md)
- [ ] **P7** Language server and editor extensions; live playground on the website from the WebAssembly compiler. Docs: [10-tech-stack.md](10-tech-stack.md)
- [ ] **P8** Weft Cloud. Docs: [08-dashboard.md](08-dashboard.md)
