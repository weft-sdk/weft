# P1.6: `weft gen` for TypeScript

- Branch: `feat/p1-6-gen-typescript`
- Task: P1.6 in the roadmap, needs P1.4 (done)
- Started: 2026-11-21 · Last session: 2026-11-21
- Pushed: no

## Goal

`weft gen` reads `weft.yaml`, checks the contracts and writes `weft.gen.ts`. Done when the generated file type-checks with `tsc --strict` for the example contracts.

## Plan

1. Work file, committed first.
2. ADR 0021: generated code embeds the IR (see Decisions).
3. New crate `weft-codegen` with the TypeScript generator: types, brand types, per-package event maps, embedded IR.
4. `weft.yaml` reading in `weft-cli` (`service`, `language`, `contracts`, `codegen.out`, `codegen.int64`).
5. `weft gen [--lang] [--out] [--check]`.
6. Tests: snapshot of the output, a `tsc --strict` check for every example (and a multi-package one), CLI tests.
7. CI: install Node and TypeScript for the compiler workflow, and fail when `tsc` is missing there.
8. Docs: cli.md, sdk-design.md "Generated code", contract-language.md, roadmap. Delete this file.

## Decisions made on this branch

- **Generated code embeds the IR** as a `const`, instead of generating validator functions. Validation, wire names and operation metadata then run from the same data in every language, which is what makes services reject bad data identically. The logic lives once per language in the core package (P2), not once per contract. Recorded in ADR 0021 (Proposed: it changes how `sdk-design.md` describes "runtime validators").
- **Types live in type-only `namespace`s**, one per package (`orders.Order`), so two packages can both declare `Money`. Namespaces that hold only types are erased by the compiler and allowed by `erasableSyntaxOnly` and Node's type stripping. No runtime code goes in a namespace.
- **The facade is not part of this task.** It needs `@weft-sdk/core`, so it arrives in P2. P1.6 generates what the facade will be typed from: the types and a per-package `Events` map.
- **The generator is plain Rust string building, not minijinja templates.** For TypeScript's output the templates add indirection and no clarity. Revisit when more languages exist (tech-stack.md names minijinja).
- **`--watch` is not in this task.** It needs the `notify` crate and a test that waits for file events. Listed under Found, not done.
- New dependencies: `serde_yaml_ng` for `weft.yaml` (the maintained fork of serde_yaml, which is archived).

## Done

- Nothing yet.

## Next step

Step 2.

## Waiting on

- ADR 0021 is Proposed: the maintainer accepts it (it changes how "runtime validators" are delivered).

## How to check

    cargo test --manifest-path compiler/Cargo.toml -p weft-codegen
    cargo test --manifest-path compiler/Cargo.toml -p weft-cli

## Found, not done

- `weft gen --watch`.
- `weft gen --lang js` (JSDoc-typed JavaScript).
- Contract sources other than local paths (git, dashboard): `weft pull`.
