# P1.5: Formatter and `weft fmt`

- Branch: `feat/p1-5-formatter`
- Task: P1.5 in the roadmap, needs P1.2 (done)
- Started: 2026-11-21 · Last session: 2026-11-21
- Pushed: no

## Goal

`weft fmt` rewrites `.weft` files in one canonical style, keeping every comment. Done when formatting is idempotent on the example contracts (and does not change what they mean).

## Plan

1. Work file, committed first.
2. `weft-syntax::format`: a pretty printer over the AST that puts the lexer's comments back by position.
3. `weft fmt [PATH...] [--check]` in `weft-cli`.
4. Tests: idempotence, comments, alignment, meaning preserved (the IR of the formatted example equals the IR of the original).
5. Document the style in `spec/language.md`. Roadmap. Delete this file.

## Decisions made on this branch

- Style rules (2-space indent, aligned route and trailing-comment columns, blank lines kept as at most one, enums and inline types on one line when they hold no comments) go in `spec/language.md`, since the formatter is the reference for style.
- `weft fmt --check` exits 1 when a file would change ([cli.md exit codes](../cli.md#exit-codes)).

## Done

- Nothing yet.

## Next step

Step 2.

## Waiting on

- Nothing.

## How to check

    cargo test --manifest-path compiler/Cargo.toml -p weft-syntax

## Found, not done

- Nothing yet.
