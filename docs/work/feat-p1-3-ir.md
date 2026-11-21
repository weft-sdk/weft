# P1.3: IR

- Branch: `feat/p1-3-ir`
- Task: P1.3 in the roadmap, needs P1.1 (done)
- Started: 2026-11-21 · Last session: 2026-11-21
- Pushed: no

## Goal

`weft-ir` defines the versioned JSON intermediate representation, and `spec/ir.schema.json` describes it. Done when the schema validates the IR the checker emits (the checker arrives in P1.4, so here the schema is checked against hand-written IR and kept in sync with the Rust types by a test).

## Plan

1. Work file, committed first.
2. Rust types in `weft-ir` with serde (camelCase JSON, tagged unions) and schemars.
3. Generate `spec/ir.schema.json` from the types. A test fails when the file is stale.
4. Tests: round trip, a sample validated with the `jsonschema` crate.
5. Docs: `spec/ir.md` (what each part means). Roadmap. Delete this file.

## Decisions made on this branch

- The IR is fully resolved: every named type carries its package, inline types have generated names and sit in `types`, operations are split into a value and a list of errors.
- One IR per package. Other packages are referred to by name, never copied in.
- Lists are flat and in source order (`routes`, `rpcServices`, `graphql`, `sockets`), so diffs by name are simple.
- New dependencies: `serde`, `serde_json` (named in tech-stack.md), `schemars` (generates the JSON Schema from the Rust types, so the two cannot drift), `jsonschema` in tests only.

## Done

- Nothing yet.

## Next step

Step 2.

## Waiting on

- Nothing.

## How to check

    cargo test --manifest-path compiler/Cargo.toml -p weft-ir

## Found, not done

- `api.md` lists seven built-in errors; `contract-language.md` lists eight (it adds `Forbidden`). Reconcile in P1.4.
