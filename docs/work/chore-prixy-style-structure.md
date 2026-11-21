# Chore: lay the repository out like prixy

- Branch: `chore/prixy-style-structure`
- Task: none in the roadmap; asked for by the maintainer, no dependencies
- Started: 2026-11-22 · Last session: 2026-11-22
- Pushed: no

## Goal

Weft's docs, roadmap and commands are organised the way the maintainer's `prixy` project is, so everything is in front of them: numbered docs in reading order with a reading-order table, a checkbox roadmap, and a root `Makefile`. Shown by: every link in the repo resolves, and `make check` runs the same checks as the commands table in CLAUDE.md.

## Plan

1. Untrack `notes/` and add it to `.gitignore` (the file stays on disk).
2. Number the docs (`01-concepts.md` ... `15-history.md`) and fix every link and code comment that names one.
3. Rewrite `docs/README.md` as a reading-order table plus "Status at a glance".
4. Convert `docs/12-roadmap.md` to checkbox task lists with "Done when" lines.
5. Add a root `Makefile`; update CLAUDE.md's Commands and links.

## Decisions made on this branch

- Maintainer chose: numbered docs, checkbox roadmap, Makefile, and `notes/` gitignored. Not chosen: a glossary, a guarantees doc, runbooks.
- Not copied from prixy: its Current status cell, which grew to thousands of words. Weft's stays short.

## Done

Nothing yet.

## Next step

Step 1.

## Waiting on

Nothing.

## How to check

    git grep -nE '\]\([^)]*\.md' | check every target exists

## Found, not done

- Old decision records keep their text; only link targets change.
