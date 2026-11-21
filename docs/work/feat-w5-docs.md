# W5: Docs section

- Branch: `feat/w5-docs`, from `main` after W1 to W3 were merged
- Task: W5 in the roadmap, needs W2
- Started: 2026-11-21 · Last session: 2026-11-21
- Pushed: no

## Goal

A docs section in `website/` with its own layout, navigation and search, and the first pages ported from the design docs (concepts, contract language, API, languages, CLI). Done when the pages render in the site's design and every code example has a language tab.

## Plan

1. Propose 3 or 4 docs layouts as a private mockup page (sidebar, on-this-page nav, search, language tabs). The maintainer picks. The design is theirs to approve.
2. Build the layout in Astro with MDX content: `website/src/pages/docs/`, a docs layout, a sidebar from a single nav data file.
3. Port the first pages from `docs/` (concepts, contract language, API, languages, CLI). The footer already links to `/docs`, `/docs/cli`, `/docs/contracts` and `/docs/migrating`, so these paths should exist.
4. Add per-language tabs on code examples, remembering the choice across pages.
5. Search: choose a static, build-time index (decision record if it adds a dependency).
6. Add the Docs link to the navbar.
7. Check at desktop and phone width in both themes. Run Lighthouse.

## Decisions made on this branch

- None yet.

## Done

- Work file.

## Next step

Build the docs layout options page for the maintainer (plan step 1), reusing the header and footer. Do not build the layout until they choose.

## Waiting on

- The maintainer's choice of docs layout.

## How to check

    pnpm --dir website check
    pnpm --dir website build
    pnpm --dir website dev   # then open http://localhost:4321

## Found, not done

- The roadmap says docs pages are "clearly marked as design-stage", which conflicts with [ADR 0019](decisions/0019-site-written-as-launch-page.md) (the site is written as the launch page). Needs the maintainer's decision before the pages are written. See also the footer's `/docs/migrating` link.
- The 390px phone layout of the landing page was checked by measurement and a few screenshots, not on a real device.
