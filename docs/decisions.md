# Decisions

Architecture decision records. Status is **Accepted** (settled), **Proposed** (working default, open to change), or **Superseded**.

To change a decision, add a new ADR that supersedes it rather than editing history.

> An earlier draft of these docs described Weft as a code scanner. That came from an outdated README and has been dropped entirely. ADR-001 records this.

---

## ADR-001: Weft is a communication library, not a scanner

**Status:** Accepted (2026-11-20)

**Decision.** Weft is a unified interface (`weft.http`, `weft.kafka`, `weft.ws`, ...) that developers call directly, running existing libraries underneath. There is no code scanner. The service map is built from contracts and telemetry instead.

## ADR-002: Language-neutral contracts with code generation

**Status:** Accepted (2026-11-20)

**Context.** Services in different languages exchange the same data. A TypeScript type can't be read by a Go service.

**Decision.** Contracts are written in `.weft` files. `weft gen` generates types and a typed facade for each language.

## ADR-003: A native package per language, wrapping existing libraries

**Status:** Accepted (2026-11-20)

**Decision.** Each language has its own Weft package written in that language, with adapters over the libraries that ecosystem already uses.

**Alternative rejected.** A single Rust core with bindings for each language. It would replace teams' existing libraries, make installs heavier (native binaries), and feel foreign in each language.

## ADR-004: Rust for everything shared

**Status:** Accepted (2026-11-20)

**Decision.** The compiler, CLI, language server, conformance harness and dashboard server are Rust.

**Consequences.** One fast binary that runs in any project. The compiler also builds to WebAssembly for the website playground.

## ADR-005: Our own contract language

**Status:** Accepted (2026-11-20)

**Decision.** Design a small `.weft` language covering events, HTTP, GraphQL, RPC and sockets in one file. Import OpenAPI, protobuf, GraphQL SDL, AsyncAPI, Avro and JSON Schema.

**Alternative considered.** Microsoft TypeSpec. Mature, but HTTP-centred and harder to extend naturally to messaging.

## ADR-006: Events are transport-neutral

**Status:** Proposed

**Decision.** An `event` contract describes the message, not the transport. The call site chooses `weft.kafka`, `weft.mq`, `weft.nats`, and so on. The same applies to `rpc` contracts and `weft.grpc` / `weft.rpc`.

**Consequences.** Moving an event from RabbitMQ to Kafka changes call sites and config, not contracts.

## ADR-007: Always provide an escape hatch

**Status:** Proposed

**Decision.** Every transport exposes the underlying library instance as `raw`.

**Consequences.** Users are never blocked by a missing Weft feature. Weft's tracing and middleware don't cover `raw` calls, and the docs say so.

## ADR-008: Conformance suite gates every release

**Status:** Accepted (2026-11-20)

**Decision.** No language package or adapter ships without passing the shared conformance suite, including cross-language cases.

## ADR-009: Dashboard in three modes, one codebase

**Status:** Accepted (2026-11-20)

**Decision.** The dashboard runs locally (`weft dashboard` on a port), self-hosted (Docker/Helm), or in Weft Cloud with sign-in. Same server and UI in all three.

## ADR-010: Website on Astro with a custom design

**Status:** Accepted (2026-11-20). Visual design not decided.

**Decision.** Astro for the landing page and docs, with a custom design rather than a stock docs theme.

## ADR-011: Shared runtime standards

**Status:** Proposed

**Decision.** CloudEvents for event metadata, OpenTelemetry for tracing and metrics, W3C Trace Context for propagation, RFC 9457 for HTTP errors, gRPC status codes for RPC errors.

## ADR-012: Where shared contracts live

**Status:** Proposed

**Decision.** Support three sources: a local path (monorepos), a git repo and tag (polyrepos), and the dashboard / Weft Cloud (`weft publish` / `weft pull`). Each service owns the contracts it provides.

## ADR-013: Open core

**Status:** Proposed

**Decision.** Compiler, CLI, language packages, adapters and the local dashboard are Apache-2.0. Weft Cloud (hosted dashboard, team features, alerts, history) is the paid product. Self-hosted dashboard licensing to be decided.

---

## Open questions

| Question | Notes |
| --- | --- |
| Contract syntax details | Generics, validation constraints, error declarations. See [contract-language.md § Open questions](contract-language.md#open-questions) |
| `weft.rpc` wire protocol | Reuse Connect's protocol, or define our own on HTTP + JSON? |
| Contract versioning across repos | Semver on contract packages? How `weft check` picks the baseline in polyrepos |
| Self-hosted dashboard licence | Free, source-available, or paid above a size |
| Cloud auth provider | WorkOS vs Clerk |
| Order of building | Which languages and transports first, once we start building |

---

## ADR-014: Product name is Weft, published under `weft-sdk`

**Status:** Accepted (2026-11-20)

**Context.** "Splice" clashed with an established software company (trademark risk) and with JavaScript's `Array.prototype.splice` (search noise), and its handles were taken.

**Decision.** The product is **Weft**: `weft.*` in code, the `weft` CLI, `.weft` contract files, `weft.yaml` config. GitHub organisation `weft-sdk`, main repo `github.com/weft-sdk/weft`.

Package names, where plain `weft` is taken:

| Registry | Name |
| --- | --- |
| npm | `@weft-sdk/*` |
| PyPI, RubyGems | `weft-core` |
| crates.io | `weft-sdk` |
| NuGet | `WeftSdk.*` |
| Packagist | `weft-sdk/*` |
| Hex, pub.dev | `weft_sdk` |
| Go | `github.com/weft-sdk/weft/go` until a domain is chosen |
| Maven | `io.github.weft-sdk` until a domain is chosen (`dev.weft` needs weft.dev) |

**Open.** Domain (weft.dev looked unregistered on 2026-11-20). Trademark search still to do.
