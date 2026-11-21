# ADR 0020: Product build order after the website

- Status: Accepted (2026-11-21)

**Context.** The website is built first ([ADR 0016](0016-website-first.md)). Open question Q2 asked what the product build order is afterwards. The roadmap carried a default proposal.

**Decision.** The default proposal is accepted:

1. **P1:** compiler and CLI (parser, checker, IR, formatter, `weft gen` for TypeScript, `weft check`, `weft init`).
2. **P2:** TypeScript package (core, Express and KafkaJS adapters, generated facade).
3. **P3:** conformance suite harness and the Go package (net/http, franz-go).

P4 onward keep the order in the [roadmap](../12-roadmap.md#product-phases).

**Consequences.**
- Phase P1 is broken into tasks next, each sized for one pull request.
- The website's remaining work (W6, W7) and the docs content wait for the backend, as before.
- The website's transport switcher and site-wide language picker are deferred until the docs content is written, not cancelled (maintainer, 2026-11-21).
