# P2.1: TypeScript workspace skeleton

- Branch: `feat/p2-1-ts-workspace`
- Task: P2.1 in the roadmap, needs P1
- Started: 2026-11-23 · Last session: 2026-11-23
- Pushed: no

## Goal

`sdks/typescript` is a pnpm workspace with an empty `@weft-sdk/core`. `make ts-check` builds, type-checks and tests it, and CI runs that on Linux, macOS and Windows.

## Plan

1. Workspace files, strict tsconfig, tsup (ESM and CJS), Vitest, one smoke test.
2. `make ts-*` targets and CLAUDE.md commands.
3. `.github/workflows/typescript.yml`.

## Done

Nothing yet.

## Next step

Step 1.

## Waiting on

Nothing.

## How to check

    make ts-install && make ts-check
