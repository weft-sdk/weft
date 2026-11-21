# P1.7: `weft check`

- Branch: `feat/p1-7-weft-check`
- Task: P1.7 in the roadmap, needs P1.4 (done)
- Started: 2026-11-21 · Last session: 2026-11-21
- Pushed: no

## Goal

`weft check` compares the current contracts with a baseline and reports safe, risky and breaking changes, failing (exit 1) when a breaking change is not covered by a version bump. Done when every row of the "Evolving contracts safely" table in contract-language.md has a test.

## Plan

1. Work file, committed first.
2. `weft-compat`: diff two IRs of one package into changes (safe, risky, breaking), and the version rule.
3. `weft check [--baseline REF] [--baseline-dir PATH]` in `weft-cli`: baseline from a git ref or a folder, then the report.
4. Tests: one per row of the table, plus the cases the table does not list (removed enum value, errors, results, routes, rpc, graphql, sockets).
5. Docs: `weft check` section in cli.md, the rules in contract-language.md, roadmap. Delete this file.

## Decisions made on this branch

- **Direction matters.** A change is judged by who sends and who receives the data. Types used in requests (HTTP path, query, body and headers, rpc requests, GraphQL arguments, socket `client` events) are *input*. Types used in responses, errors, events and socket `server` events are *output*. A type used both ways, or in neither (another package may import it), gets the stricter judgement of both.
- **Only breaking changes fail the check, and only when the version does not cover them.** A breaking change needs a major bump (a minor bump while the major is 0, as in semver). A risky or safe change that the version bump does not cover is a warning, since contracts change many times between releases.
- **Baseline.** `--baseline <git ref>`, or by default the latest git tag. `--baseline-dir <path>` compares against a folder of older contracts (also how a pulled published version will be compared later). The check reads the baseline's contract files with `git show`, never by checking out.
- **Every declared type is public.** Removing one is breaking, because another package may import it.
- Decisions that go beyond the table in contract-language.md are written into that doc in the finishing commit.

## Done

- Nothing yet.

## Next step

Step 2.

## Waiting on

- Nothing.

## How to check

    cargo test --manifest-path compiler/Cargo.toml -p weft-compat -p weft-cli

## Found, not done

- `--json` output for tools and the dashboard.
- Comparing against a published version (`weft pull`, dashboard).
