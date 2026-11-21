# Roadmap

Work is split into phases, and each phase into tasks sized for one pull request. Every task has an id (`W3`, `P1.2`) and lists what it needs. How to pick up a task and hand it off: [workflow.md](workflow.md).

## Current status

_Describes `main`. Updated in the pull request that finishes a task, for what `main` will be after the merge. Work in progress lives in each branch's work file in [work/](work/)._

| Field | Value |
| --- | --- |
| Phase | W (website). W1, W2, W3 and W5 done |
| Built | **The website** (`website/`: the landing page and the docs layout with placeholder pages, light and dark themes, Lighthouse 100). No docs content, compiler, CLI, language package or dashboard exists |
| Next task | **W4, interactive pieces** (transport switcher, language picker). Writing the docs content waits for the backend |
| Last done | 2026-11-21: W5, docs layout. `pnpm --dir website check` and `build` pass locally; the CI workflow has not run yet |
| Waiting on the maintainer | Not blocking: npm org `weft-sdk` (confirm it was created), trademark search for "Weft", domain (Q3), talking to 5–10 developers about the idea |

## P0: Design documents — done

The product, contract language, API, language package design, supported languages, CLI, conformance suite, dashboard, architecture, tech stack and website plan are written in [docs/](README.md). Decisions 0001–0016 are accepted.

## W: Website — in progress

The landing page and docs site, built before any product code ([ADR 0016](decisions/0016-website-first.md)). Plan: [website.md](website.md). The site is written as the launch page of the finished product, and is not deployed publicly until its claims are true ([ADR 0019](decisions/0019-site-written-as-launch-page.md)).

| Task | What | Needs | Done when |
| --- | --- | --- | --- |
| **W1** ✓ | Visual direction: references the maintainer likes, palette, type, motion, layout principles. Written into `website.md` and a decision record | Maintainer | Done 2026-11-21. Approved direction in [website.md](website.md#visual-direction-the-terminal) |
| **W2** ✓ | Site skeleton: Astro + TypeScript + pnpm in `website/`, design tokens from W1, base layout, light and dark themes, CI build | W1 | `pnpm --dir website build` passes in CI; an empty styled page renders |
| **W3** ✓ | Landing page: hero, the problem, how it works (init → contract → call), transports, languages, dashboard preview, call to action (GitHub, follow updates) | W2 | All sections render with real copy, work at phone width, and pass a Lighthouse check (performance and accessibility ≥ 90) | Done 2026-11-21. Lighthouse is 100 in all four categories, in both themes |
| **W4** | Interactive pieces: transport switcher in the hero, persistent language picker, before/after comparison | W3 | Each works with keyboard and respects reduced motion |
| **W5** ✓ | Docs section: custom layout, navigation, search, language tabs, and a page for every planned entry. Page content is written later, once the backend exists | W2 | Done 2026-11-21. Layout chosen by the maintainer ([website.md](website.md#docs-layout-w5)); Lighthouse 100 on both docs pages |
| **W6** | Playground preview: a contract editor showing pre-generated output for a few example contracts in several languages | W4 | Switching examples and languages works without a server |
| **W7** | Launch prep: social images, metadata, sitemap, analytics decision, hosting, domain. Check every claim on the site against shipped code first ([ADR 0019](decisions/0019-site-written-as-launch-page.md)) | W3–W6, Q3, and the product claims being true | The maintainer has deployed it to the chosen domain |

## Product phases — proposed, order open

Not broken into tasks until [Q2](open-questions.md) is answered. Default proposal:

| Phase | What | Docs |
| --- | --- | --- |
| P1 | Compiler and CLI: parser, checker, IR, formatter, `weft gen` for TypeScript, `weft check`, `weft init` | [contract-language.md](contract-language.md), [cli.md](cli.md), [architecture.md](architecture.md) |
| P2 | TypeScript package: core, Express and KafkaJS adapters, generated facade | [sdk-design.md](sdk-design.md), [api.md](api.md) |
| P3 | Conformance suite harness, and the Go package (net/http, franz-go). Done when a TypeScript service and a Go service talk through one contract and pass the cross-language cases | [conformance.md](conformance.md) |
| P4 | More transports in TS and Go (GraphQL, gRPC, WebSockets, RabbitMQ, NATS) | [languages.md](languages.md) |
| P5 | Local dashboard (`weft dashboard`) | [dashboard.md](dashboard.md) |
| P6 | More languages: Python, JVM, .NET, then the rest | [languages.md](languages.md) |
| P7 | Language server and editor extensions; live playground on the website from the WebAssembly compiler | [tech-stack.md](tech-stack.md) |
| P8 | Weft Cloud | [dashboard.md](dashboard.md) |
