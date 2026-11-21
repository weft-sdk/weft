# ADR 0019: The website is written as the launch page

- Status: Accepted (2026-11-21)

**Context.** Until now the website had to present Weft as in development and label every command as planned, because the README and site may only claim what exists. The site is not public yet. The maintainer wants it written as the finished product's page now, so the copy and design are ready for launch.

**Decision.** The website's copy describes Weft as the finished product defined in the design docs, with no "planned" or "in development" labels. In exchange, the site is **not deployed publicly** (task W7) until every claim on it is true of shipped code. The README and the design docs keep describing reality.

**Rules that still hold.** Every claim must match the design docs: transports, languages, libraries, commands, defaults and output. No made-up numbers: no user counts, benchmarks or adoption figures.

**Alternatives considered.** Keeping the "in development" framing until launch. Rejected by the maintainer: it would mean rewriting the whole page at launch.

**Consequences.**
- Before W7, each claim on the page is checked against what has shipped; anything not yet true is cut or the deploy waits.
- Private previews (for example on a protected preview URL) are fine.
