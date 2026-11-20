# W3: Landing page

- Branch: `feat/w3-landing-page`, stacked on `feat/w2-site-skeleton`, which is stacked on `docs/w1-visual-direction`. None are merged yet.
- Task: W3 in the roadmap, needs W2
- Started: 2026-11-21 · Last session: 2026-11-21
- Pushed: no

## Goal

The landing page from the approved terminal mockup is built in `website/` with real copy. Done when all sections render with real copy, work at phone width, and pass a Lighthouse check with performance and accessibility at 90 or above.

## Plan

1. Data files for transports and languages in `website/src/data/`, taken from `docs/languages.md`.
2. One component per section in `website/src/components/landing/`. Each section opens with its prompt line:
   - hero (`$ weft`)
   - transports (`$ weft transports`)
   - before and after (`$ weft diff --before`)
   - setup (`$ weft init`)
   - contract (`$ cat contracts/orders.weft`)
   - languages (`$ weft languages`)
   - breaking changes (`$ weft check`)
   - dashboard (`$ weft dashboard`)
   - status (`$ weft status`)
3. Compose them in `src/pages/index.astro`.
4. Check at desktop and phone width, in both themes. Run Lighthouse.

## Decisions made on this branch

- The approved mockup is the reference: `680a44b:docs/work/w1-mockups/c3-terminal.html`.
- Added a `$ weft dashboard` section, because the roadmap's W3 asks for a dashboard preview. It shows the planned command output and what the dashboard will show, as text, so it stays minimal.

## Done

- Work file.
- The landing page, all nine sections, in `website/src/components/landing/` and `src/pages/index.astro`. `pnpm check` and `pnpm build` pass.
- Checked with headless Chrome screenshots at 1440px (dark and light) and 500px (dark). The table and code blocks scroll sideways inside their own boxes at narrow widths; the page itself does not.
- Lighthouse 12.6.1 on the built site, desktop: performance 97, accessibility 100 in dark and 95 in light, best practices 100, SEO 100. The light-mode points were contrast on code comments. The `--faint` token was raised after that run, and every text token now computes at 4.5:1 or better on both backgrounds. Lighthouse has not been re-run since that change.

## Next step

Apply the maintainer's remaining design changes (they will send them after seeing this page). Then re-run Lighthouse, check true phone width (390px) in a real browser, update the roadmap and delete this work file.

## Waiting on

- The maintainer's remaining changes to the design, which they will send after seeing this page.
- The maintainer to merge W1 and W2 first.

## How to check

    pnpm --dir website check
    pnpm --dir website build
    pnpm --dir website dev   # then open http://localhost:4321

## Found, not done

- The call to action has no public destination yet. The GitHub repo is private and there are no public docs. It points to the status section for now.
