# Weft documentation

Weft is one interface for every way services talk, in every language. These documents are the source of truth: what Weft is, how it works, why it is designed this way, and what to do next. They are written so that someone (or a coding-agent session) with no other context can pick up the work. The product overview is the [top-level README](../README.md). Guidance for coding agents is in [CLAUDE.md](../CLAUDE.md).

**Design stage.** Every feature described is planned unless it says otherwise. What exists beyond documents is listed under "Status at a glance" below.

## Read in this order

| # | Document | What it answers |
| --- | --- | --- |
| 1 | [Concepts](01-concepts.md) | The vocabulary: contracts, transports, adapters, packages, codegen |
| 2 | [Contract language](02-contract-language.md) | The `.weft` language (0.1, decided) and how contracts evolve safely |
| 3 | [API](03-api.md) | The `weft.*` API for each transport, with examples across languages |
| 4 | [SDK design](04-sdk-design.md) | How each language package is built: core, adapters, shared behaviour |
| 5 | [Languages](05-languages.md) | Every supported language and the library Weft runs underneath for each transport |
| 6 | [CLI](06-cli.md) | `weft init`, `gen`, `check`, `dashboard` and the config file |
| 7 | [Conformance](07-conformance.md) | The shared test suite that keeps 13 languages behaving the same |
| 8 | [Dashboard](08-dashboard.md) | The service map and live traffic UI: local, self-hosted and cloud |
| 9 | [Architecture](09-architecture.md) | How the pieces fit, and the repo layout |
| 10 | [Tech stack](10-tech-stack.md) | Every tool and library used to build Weft |
| 11 | [Website](11-website.md) | The landing page and docs site (built first) and its visual direction |
| 12 | [Roadmap](12-roadmap.md) | **Start here for work.** Current status, phases and tasks with ids and dependencies |
| 13 | [Open questions](13-open-questions.md) | Decisions waiting on the maintainer |
| 14 | [Workflow](14-workflow.md) | How a session picks up work, works in parallel and hands off |
| 15 | [History](15-history.md) | How Weft got here, and what was rejected |
| — | [Decision records](decisions/) | Why each major choice was made |
| — | [Work in progress](work/) | One handoff file per open branch |

If you only read three product docs: **concepts**, **contract-language**, **api**.

## Status at a glance

The exact resume point is [Roadmap › Current status](12-roadmap.md#current-status).

- **Phase:** P1 (compiler and CLI) and P2 (the TypeScript package) are done. P3 (the conformance suite and the Go package) is next. The website is built and can resume now that the backend exists for TypeScript.
- **Built:** the website (`website/`); a Rust workspace (`compiler/`) with the parser, checker, compatibility checker and the `weft init`, `fmt`, `check` and `gen` commands; and the TypeScript package (`sdks/typescript/`): the core, HTTP on Express and Kafka on KafkaJS, tested end to end against a real Kafka broker. CI passes on Linux, macOS and Windows.
- **Not built:** the conformance suite, every other language package, every other transport, the dashboard and the docs content. Nothing is published to npm.
- **Production-ready:** no.

## Rules for these docs

- Describe what exists separately from what is planned. Never present a planned feature as done.
- When a decision changes, add a record in `decisions/` and update the affected documents in the same change. Mark old records superseded or amended, never delete or rewrite them.
- Keep [Roadmap › Current status](12-roadmap.md#current-status) accurate: it is the resume point. Update **Status at a glance** above in the same change.
- **MUST / SHOULD / MAY** follow RFC 2119 in the spec documents (contract language, SDK design, conformance).
- Examples use a fictional company, **Larder** (online grocery delivery). Its `orders` service is TypeScript, `payments` is Go, `notifications` is Python, `inventory` is Kotlin.
- TypeScript examples apply equally to JavaScript.
