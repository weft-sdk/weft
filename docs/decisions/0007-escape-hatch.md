# ADR 0007: Always provide an escape hatch

- Status: Accepted (2026-11-20)

**Decision.** Every transport exposes the underlying library instance as `raw`.

**Consequences.** Users are never blocked by a missing Weft feature. Weft's tracing and middleware don't cover `raw` calls, and the docs say so.
