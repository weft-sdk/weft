# P1.1: Rust workspace skeleton

- Branch: `feat/p1-1-workspace-skeleton`
- Task: P1.1 in the roadmap, needs nothing
- Started: 2026-11-21 · Last session: 2026-11-21
- Pushed: no

## Goal

A Rust workspace in `compiler/` that builds, tests and lints in CI, with a `weft --version` binary. Done when `cargo build`, `cargo test` and `cargo clippy` pass in CI.

## Plan

1. Work file (this file), committed first.
2. `compiler/Cargo.toml` workspace, pinned toolchain (`rust-toolchain.toml`), `rustfmt.toml`, workspace lints.
3. Crates `weft-syntax`, `weft-ir`, `weft-check` (library, empty but with one test each) and `weft-cli` (binary named `weft`, clap, `--version`).
4. `.github/workflows/compiler.yml`: fmt check, clippy with warnings denied, test, build. Path-filtered like the website workflow.
5. `.gitignore`: `compiler/target/`.
6. CLAUDE.md: Rust commands and conventions. Roadmap: P1.1 marked done and Current status updated, in the finishing commit.

## Decisions made on this branch

- Crate names follow [architecture › Repo layout](../architecture.md#repo-layout). `weft-compat`, `weft-codegen`, `weft-init` are created by the tasks that need them (P1.7, P1.6, P1.8).
- clap is the only dependency, because `--version` needs the CLI parser the tech stack already names. The others (chumsky, miette, serde) come with the task that uses them.

## Done

- Nothing yet.

## Next step

Step 2: create the workspace.

## Waiting on

- Nothing.

## How to check

    cd compiler && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && cargo run -q -p weft-cli -- --version

## Found, not done

- Nothing yet.
