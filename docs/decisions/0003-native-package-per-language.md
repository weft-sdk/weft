# ADR 0003: A native package per language, wrapping existing libraries

- Status: Accepted (2026-11-20)

**Decision.** Each language has its own Weft package written in that language, with adapters over the libraries that ecosystem already uses.

**Alternative rejected.** A single Rust core with bindings for each language. It would replace teams' existing libraries, make installs heavier (native binaries), and feel foreign in each language.
