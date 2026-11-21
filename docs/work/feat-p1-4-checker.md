# P1.4: Checker

- Branch: `feat/p1-4-checker`
- Task: P1.4 in the roadmap, needs P1.2 and P1.3 (done)
- Started: 2026-11-21 · Last session: 2026-11-21
- Pushed: no

## Goal

`weft-check` turns parsed `.weft` files into one IR per package, or reports every contract error with its location. Done when the complete example compiles to IR (and the IR validates against `spec/ir.schema.json`), and every rule has a failing case.

## Plan

1. Work file, committed first.
2. Package grouping, index of declared names, import order and cycles.
3. Type resolution: primitives, generics, maps, imports, inline-type naming.
4. Annotations and validation rules.
5. Events, errors, HTTP, RPC, GraphQL and sockets.
6. Tests: the example (golden IR, schema-validated), a failing case per rule (snapshots).
7. Document the meaning rules in `spec/language.md`. Roadmap. Delete this file.

## Decisions made on this branch

- A result is a value type first, then errors: `-> Order | OutOfStock | InvalidInput`. Only the first member may be `?`.
- `@pattern` is checked by compiling it with the `regex` crate, whose syntax is the common subset (no lookaround, no backreferences). Go's RE2 has no lookahead either, so lookahead is rejected too.
- Types, enums and errors share one namespace per package.
- Inline types are named from where they appear (`OrderCancelled`, `ListOrdersQuery`, `OrderPlacedBy`). A clash is an error that tells the author to name the type.
- Built-in errors live in the package `weft`. `Forbidden` is built in (it was in contract-language.md but missing from api.md).
- New dependency: `regex`, to validate `@pattern`.

## Done

- Nothing yet.

## Next step

Step 2.

## Waiting on

- Nothing.

## How to check

    cargo test --manifest-path compiler/Cargo.toml -p weft-check

## Found, not done

- Nothing yet.
