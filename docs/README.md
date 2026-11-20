# Weft docs

Design documents for Weft: one interface for every way services talk, in every language. The product overview is the [top-level README](../README.md). Guidance for coding agents is in [CLAUDE.md](../CLAUDE.md).

Status: **design stage. Nothing is built.**

## Project state

| Doc | What it covers |
| --- | --- |
| [roadmap.md](roadmap.md) | **Start here.** Current status, what is next, every task and what it needs |
| [open-questions.md](open-questions.md) | Decisions waiting on the maintainer |
| [decisions/](decisions/) | Why things are the way they are, one record per decision |
| [workflow.md](workflow.md) | How a session picks up work and hands it off |
| [work/](work/) | Work files for branches in progress |
| [history.md](history.md) | How Weft got here, and what was rejected |

## The product

| Doc | What it covers |
| --- | --- |
| [concepts.md](concepts.md) | The vocabulary: contracts, transports, adapters, packages, codegen |
| [contract-language.md](contract-language.md) | The `.weft` contract language (0.1, decided) and how contracts evolve safely |
| [api.md](api.md) | The `weft.*` API for each transport, with examples across languages |
| [sdk-design.md](sdk-design.md) | How each language package is built: core, adapters, shared behaviour |
| [languages.md](languages.md) | Every supported language and the library Weft runs underneath for each transport |
| [cli.md](cli.md) | `weft init`, `gen`, `check`, `dashboard` and the config file |
| [conformance.md](conformance.md) | The shared test suite that keeps 13 languages behaving the same |
| [dashboard.md](dashboard.md) | The service map and live traffic UI: local, self-hosted and cloud |
| [architecture.md](architecture.md) | How the pieces fit, and the repo layout |
| [tech-stack.md](tech-stack.md) | Every tool and library used to build Weft |
| [website.md](website.md) | The landing page and docs site (built first; visual design not decided) |

If you only read three product docs: **concepts**, **contract-language**, **api**.

## Conventions

- **MUST / SHOULD / MAY** follow RFC 2119 in the spec documents (contract language, SDK design, conformance).
- Examples use a fictional company, **Larder** (online grocery delivery). Its `orders` service is TypeScript, `payments` is Go, `notifications` is Python, `inventory` is Kotlin.
- TypeScript examples apply equally to JavaScript.
