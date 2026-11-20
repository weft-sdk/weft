# ADR 0012: Where shared contracts live

- Status: Accepted (2026-11-20)

**Decision.** Support three sources: a local path (monorepos), a git repo and tag (polyrepos), and the dashboard / Weft Cloud (`weft publish` / `weft pull`). Each service owns the contracts it provides.
