# Roadmap

Work is split into phases, and each phase into tasks sized for one pull request. Every task has an id (`W3`, `P1.2`) and lists what it needs. How to pick up a task and hand it off: [workflow.md](workflow.md).

## Current status

_Describes `main`. Updated in the pull request that finishes a task, for what `main` will be after the merge. Work in progress lives in each branch's work file in [work/](work/)._

| Field | Value |
| --- | --- |
| Phase | P1 (compiler and CLI) done. P2 (TypeScript package) is next. W (website) paused until the backend exists |
| Built | **The website** (`website/`: the landing page and the docs layout with placeholder pages, light and dark themes, Lighthouse 100). **A parser, checker and compatibility checker** (`weft-syntax`, `weft-ir`, `weft-check`, `weft-compat`: they turn contracts into a checked IR, or into located errors, and judge changes between two versions) inside a Rust workspace (`compiler/`) whose CLI has four commands: `weft init`, `weft fmt`, `weft check` and `weft gen` (TypeScript only). No docs content, working compiler, language package or dashboard exists |
| Next task | **P2, the TypeScript package**, to be broken into tasks first ([ADR 0020](decisions/0020-product-build-order.md)). It builds on [ADR 0021](decisions/0021-generated-code-embeds-the-ir.md), which the maintainer has not accepted yet. W6, W7 and the docs content wait for the backend |
| Last done | 2026-11-21: P1.8, `weft init`, which finishes phase P1. `cargo fmt --check`, `clippy` and `test` pass locally; neither CI workflow has run yet |
| Waiting on the maintainer | [ADR 0021](decisions/0021-generated-code-embeds-the-ir.md) (generated code embeds the IR) to accept or change; P2 builds on it. Not blocking: trademark search for "Weft", domain (Q3), talking to 5–10 developers about the idea |

## P0: Design documents — done

The product, contract language, API, language package design, supported languages, CLI, conformance suite, dashboard, architecture, tech stack and website plan are written in [docs/](README.md). Decisions 0001–0016 are accepted.

## W: Website — in progress

The landing page and docs site, built before any product code ([ADR 0016](decisions/0016-website-first.md)). Plan: [website.md](website.md). The site is written as the launch page of the finished product, and is not deployed publicly until its claims are true ([ADR 0019](decisions/0019-site-written-as-launch-page.md)).

| Task | What | Needs | Done when |
| --- | --- | --- | --- |
| **W1** ✓ | Visual direction: references the maintainer likes, palette, type, motion, layout principles. Written into `website.md` and a decision record | Maintainer | Done 2026-11-21. Approved direction in [website.md](website.md#visual-direction-the-terminal) |
| **W2** ✓ | Site skeleton: Astro + TypeScript + pnpm in `website/`, design tokens from W1, base layout, light and dark themes, CI build | W1 | `pnpm --dir website build` passes in CI; an empty styled page renders |
| **W3** ✓ | Landing page: hero, the problem, how it works (init → contract → call), transports, languages, dashboard preview, call to action (GitHub, follow updates) | W2 | All sections render with real copy, work at phone width, and pass a Lighthouse check (performance and accessibility ≥ 90) | Done 2026-11-21. Lighthouse is 100 in all four categories, in both themes |
| **W4** ✓ | Interactive pieces: transport switcher in the hero, persistent language picker, before/after comparison. The before/after comparison is built (W3). The docs language tabs already remember the chosen language. The transport switcher and the site-wide picker are deferred until the docs content is written (maintainer, 2026-11-21) | W3 | Done 2026-11-21 for what is built; the two deferred pieces are not scheduled |
| **W5** ✓ | Docs section: custom layout, navigation, search, language tabs, and a page for every planned entry. Page content is written later, once the backend exists | W2 | Done 2026-11-21. Layout chosen by the maintainer ([website.md](website.md#docs-layout-w5)); Lighthouse 100 on both docs pages |
| **W6** | Playground preview: a contract editor showing pre-generated output for a few example contracts in several languages | W4 | Switching examples and languages works without a server |
| **W7** | Launch prep: social images, metadata, sitemap, analytics decision, hosting, domain. Check every claim on the site against shipped code first ([ADR 0019](decisions/0019-site-written-as-launch-page.md)) | W3–W6, Q3, and the product claims being true | The maintainer has deployed it to the chosen domain |

## P1: Compiler and CLI — done

Rust workspace in `compiler/` ([architecture](architecture.md#repo-layout), [tech-stack](tech-stack.md#compiler-and-cli-rust)). The chain that decides the length is P1.1, P1.2, P1.4, P1.6. P1.3 and P1.5 can run beside it. `weft dashboard` waits for P5; importers, the language server and the WebAssembly build wait for later phases.

| Task | What | Needs | Done when |
| --- | --- | --- | --- |
| **P1.1** ✓ | Rust workspace skeleton: `weft-syntax`, `weft-ir`, `weft-check` and `weft-cli` as near-empty crates, a `weft --version` binary, rustfmt and clippy config, a CI job. Rust commands added to CLAUDE.md | none | Done 2026-11-21. `cargo build`, `test`, `clippy` and `fmt --check` pass locally; the CI workflow (Linux, macOS, Windows) has not run yet |
| **P1.2** ✓ | Parser: lexer, grammar and AST for the whole contract language, `miette` errors. The language spec moves to `spec/language.md` | P1.1 | Done 2026-11-21. The complete example parses (`spec/examples/orders.weft`), and bad input gives located errors (snapshot tests) |
| **P1.3** ✓ | IR: versioned JSON types and `spec/ir.schema.json` | P1.1 | Done 2026-11-21. `weft-ir` types, `spec/ir.schema.json` generated from them (a test keeps it current), and a sample using every part validates. The checker's output is validated from P1.4 on |
| **P1.4** ✓ | Checker: name resolution, type checking, operation rules, errors, generics, validation rules, imports; produces the IR | P1.2, P1.3 | Done 2026-11-21. The complete example compiles to IR that validates against the schema, packages import each other, and every rule has a failing test (about 100 tests, a few snapshots of the messages). Rules are in [spec/language.md](../spec/language.md#meaning-rules) |
| **P1.5** ✓ | Formatter and `weft fmt` | P1.2 | Done 2026-11-21. Formatting is idempotent, keeps every comment, and leaves the IR unchanged. `weft fmt [PATH...] [--check]` works. Style is in [spec/language.md](../spec/language.md#style) |
| **P1.6** ✓ | `weft gen` for TypeScript: type mapping and naming from the docs, reads `weft.yaml` | P1.4 | Done 2026-11-21. The generated `weft.gen.ts` type-checks with `tsc --strict` (also with `erasableSyntaxOnly`, and with `int64: bigint`) for the examples, and tests prove the types reject wrong values. The typed facade waits for the TypeScript package (P2). Not built: `--watch`, `--lang js`, contract sources other than local folders. [ADR 0021](decisions/0021-generated-code-embeds-the-ir.md) is proposed |
| **P1.7** ✓ | `weft check` (`weft-compat`): diff two IRs into safe, risky and breaking, with the documented exit codes | P1.4 | Done 2026-11-21. Every row of the evolving-contracts table has a test, plus the cases the table does not list (about 40 tests in `weft-compat`, 13 through the CLI against real git repositories). Direction-aware rules and the version rule are written into [contract-language.md](contract-language.md#evolving-contracts-safely). Not built: `--json` output, comparing with a published version |
| **P1.8** ✓ | `weft init`, non-interactive and interactive | P1.6 | Done 2026-11-21. In an empty folder it writes a working `weft.yaml`, a first contract matched to the chosen transports, and the generated code; every transport alone and all together are tested, and the result passes `gen --check`, `fmt --check` and `check`. The interactive prompts were driven by hand through a pseudo-terminal, with no automated test. Package installation waits for P2 |

## Product phases after P1

Order accepted in [ADR 0020](decisions/0020-product-build-order.md): P1, then P2, then P3. P4 onward are proposed. Not yet broken into tasks:

| Phase | What | Docs |
| --- | --- | --- |
| P2 | TypeScript package: core, Express and KafkaJS adapters, generated facade | [sdk-design.md](sdk-design.md), [api.md](api.md) |
| P3 | Conformance suite harness, and the Go package (net/http, franz-go). Done when a TypeScript service and a Go service talk through one contract and pass the cross-language cases | [conformance.md](conformance.md) |
| P4 | More transports in TS and Go (GraphQL, gRPC, WebSockets, RabbitMQ, NATS) | [languages.md](languages.md) |
| P5 | Local dashboard (`weft dashboard`) | [dashboard.md](dashboard.md) |
| P6 | More languages: Python, JVM, .NET, then the rest | [languages.md](languages.md) |
| P7 | Language server and editor extensions; live playground on the website from the WebAssembly compiler | [tech-stack.md](tech-stack.md) |
| P8 | Weft Cloud | [dashboard.md](dashboard.md) |
