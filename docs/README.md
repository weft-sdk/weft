# Weft docs

Design documents for Weft: one interface for every way services talk, in every language. The product overview is the [top-level README](../README.md). Guidance for coding agents is in [CLAUDE.md](../CLAUDE.md).

Status: **design stage.** The compiler and CLI are built (phase P1). The language packages, conformance suite and dashboard are not. See the [roadmap](12-roadmap.md).

## Project state

| Doc | What it covers |
| --- | --- |
| [12-roadmap.md](12-roadmap.md) | **Start here.** Current status, what is next, every task and what it needs |
| [13-open-questions.md](13-open-questions.md) | Decisions waiting on the maintainer |
| [decisions/](decisions/) | Why things are the way they are, one record per decision |
| [14-workflow.md](14-workflow.md) | How a session picks up work and hands it off |
| [work/](work/) | Work files for branches in progress |
| [15-history.md](15-history.md) | How Weft got here, and what was rejected |

## The product

| Doc | What it covers |
| --- | --- |
| [01-concepts.md](01-concepts.md) | The vocabulary: contracts, transports, adapters, packages, codegen |
| [02-contract-language.md](02-contract-language.md) | The `.weft` contract language (0.1, decided) and how contracts evolve safely |
| [03-api.md](03-api.md) | The `weft.*` API for each transport, with examples across languages |
| [04-sdk-design.md](04-sdk-design.md) | How each language package is built: core, adapters, shared behaviour |
| [05-languages.md](05-languages.md) | Every supported language and the library Weft runs underneath for each transport |
| [06-cli.md](06-cli.md) | `weft init`, `gen`, `check`, `dashboard` and the config file |
| [07-conformance.md](07-conformance.md) | The shared test suite that keeps 13 languages behaving the same |
| [08-dashboard.md](08-dashboard.md) | The service map and live traffic UI: local, self-hosted and cloud |
| [09-architecture.md](09-architecture.md) | How the pieces fit, and the repo layout |
| [10-tech-stack.md](10-tech-stack.md) | Every tool and library used to build Weft |
| [11-website.md](11-website.md) | The landing page and docs site (built first) and its visual direction |

If you only read three product docs: **concepts**, **contract-language**, **api**.

## Conventions

- **MUST / SHOULD / MAY** follow RFC 2119 in the spec documents (contract language, SDK design, conformance).
- Examples use a fictional company, **Larder** (online grocery delivery). Its `orders` service is TypeScript, `payments` is Go, `notifications` is Python, `inventory` is Kotlin.
- TypeScript examples apply equally to JavaScript.
