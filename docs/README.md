# Weft design docs

Internal design documents for Weft: a unified communication library for every language. The product overview is in the [top-level README](../README.md).

Status: **draft**. Nothing is implemented yet. Decisions that are settled versus still open are tracked in [decisions.md](decisions.md).

## Reading order

| Doc | What it covers |
| --- | --- |
| [concepts.md](concepts.md) | The vocabulary: contracts, transports, adapters, packages, codegen |
| [contract-language.md](contract-language.md) | The `.weft` contract language, and how contracts evolve safely |
| [api.md](api.md) | The `weft.*` API for each transport, with examples across languages |
| [sdk-design.md](sdk-design.md) | How each language package is built: core, adapters, shared behaviour |
| [languages.md](languages.md) | Every supported language and the library Weft runs underneath for each transport |
| [cli.md](cli.md) | `weft init`, `gen`, `check`, `dashboard` and the config file |
| [conformance.md](conformance.md) | The shared test suite that keeps 13 languages behaving the same |
| [dashboard.md](dashboard.md) | The service map and live traffic UI: local, self-hosted and cloud |
| [architecture.md](architecture.md) | How the pieces fit, and the repo layout |
| [tech-stack.md](tech-stack.md) | Every tool and library we use to build Weft |
| [website.md](website.md) | The landing page and docs site: tool choice and ideas (design not final) |
| [decisions.md](decisions.md) | Decision log and open questions |

If you only read three: **concepts**, **contract-language**, **api**.

## Conventions

- **MUST / SHOULD / MAY** follow RFC 2119 in the spec documents (contract language, SDK design, conformance).
- Examples use a fictional company, **Larder** (online grocery delivery). Its `orders` service is TypeScript, `payments` is Go, `notifications` is Python, `inventory` is Kotlin.
- TypeScript examples apply equally to JavaScript.
