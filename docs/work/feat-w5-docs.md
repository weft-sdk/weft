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
3. Placeholder pages only for now, at the paths the footer links to: `/docs`, `/docs/cli`, `/docs/contracts` and `/docs/migrating`. Porting the real content waits for the backend.
4. Add per-language tabs on code examples, remembering the choice across pages.
5. Search: choose a static, build-time index (decision record if it adds a dependency).
6. Add the Docs link to the navbar.
7. Check at desktop and phone width in both themes. Run Lighthouse.

## Decisions made on this branch

- Docs are written as the finished product, like the landing page ([ADR 0019](../decisions/0019-site-written-as-launch-page.md)), with no "design-stage" marking. The roadmap's "clearly marked as design-stage" wording is superseded by this. The backend is not built, so the page content is not written yet: W5 on this branch is the layout, navigation, search and language tabs only, with placeholder pages. (Maintainer, 2026-11-21.)

## Done

- Work file.

## Next step

The docs layout options page is built and sent. Build the layout the maintainer picks (plan step 2), with placeholder pages only.

## Waiting on

- The maintainer's choice of docs layout (options A to D).

## How to check

    pnpm --dir website check
    pnpm --dir website build
    pnpm --dir website dev   # then open http://localhost:4321

## Found, not done

- The roadmap's W5 row still says "clearly marked as design-stage"; update it when W5 is finished. The docs content itself (the ported pages) is a later task, once the backend exists.
- The 390px phone layout of the landing page was checked by measurement and a few screenshots, not on a real device.
