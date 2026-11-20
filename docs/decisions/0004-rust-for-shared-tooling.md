# ADR 0004: Rust for everything shared

- Status: Accepted (2026-11-20)

**Decision.** The compiler, CLI, language server, conformance harness and dashboard server are Rust.

**Consequences.** One fast binary that runs in any project. The compiler also builds to WebAssembly for the website playground.
