# ADR 0002: Language-neutral contracts with code generation

- Status: Accepted (2026-11-20)

**Context.** Services in different languages exchange the same data. A TypeScript type can't be read by a Go service.

**Decision.** Contracts are written in `.weft` files. `weft gen` generates types and a typed facade for each language.
