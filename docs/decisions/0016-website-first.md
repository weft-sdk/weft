# ADR 0016: Build the website first

- Status: Accepted (2026-11-20)

**Decision.** The first thing built is the website (landing page and docs, [website.md](../website.md)), before the compiler, language packages or dashboard.

**Consequences.**
- The visual design must be decided now (it was deferred in [ADR 0010](0010-website-on-astro.md)).
- Code examples on the site are written by hand until the compiler exists. The live playground ships once the compiler builds to WebAssembly; until then it can show pre-generated output for a few example contracts.
- Order of product work after the website is still to decide.
