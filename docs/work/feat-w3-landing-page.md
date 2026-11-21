# W3: Landing page

- Branch: `feat/w3-landing-page`, stacked on `feat/w2-site-skeleton`, which is stacked on `docs/w1-visual-direction`. None are merged yet.
- Task: W3 in the roadmap, needs W2
- Started: 2026-11-21 · Last session: 2026-11-21
- Pushed: yes, with this session's commits

## Goal

The landing page from the approved terminal mockup is built in `website/` with real copy. Done when all sections render with real copy, work at phone width, and pass a Lighthouse check with performance and accessibility at 90 or above.

## Plan

Done as planned, then reworked section by section with the maintainer. The page is ten sections in `website/src/components/landing/`, composed in `src/pages/index.astro`: hero, transports, compare, setup, contract, languages, behaviour, check, dashboard, get started. Sections have no prompt lines. Shared heading, intro, spacing and motion styles live in `tokens.css` and `global.css`.

## Decisions made on this branch

- The approved mockup is the reference: `680a44b:docs/work/w1-mockups/c3-terminal.html`.
- The site is written as the launch page of the finished product, not deployed until true ([ADR 0019](../decisions/0019-site-written-as-launch-page.md)). Geist is the font ([ADR 0018](../decisions/0018-website-fonts-geist.md)).
- Brand logos come from `simple-icons` plus a few images in `website/public/logos/`.
- Navbar: section links, theme toggle, a Get started button and a phone menu. Docs and GitHub links wait for W5 and a public repo.
- Footer links to `/docs/...` and to the repo and npm scope; none work until W5 and the public repo exist.

## Done

- All ten sections, the navbar with phone menu, the footer, the logo and favicon, light and dark themes, smooth scroll.
- `pnpm check` and `pnpm build` pass. Lighthouse 12.6.1 on the built site scores 100 in performance, accessibility, best practices and SEO, in both themes.
- Checked with a real browser (Playwright on system Chrome) at 1440px, 1100px and 420px: no horizontal overflow, no console errors, equal section padding.

## Next step

Open the pull request for W1, W2 and W3 (the maintainer merges), then update the roadmap and delete this work file. After that, start W5, the docs section.

## Waiting on

- The maintainer to merge W1 and W2 first, then this branch.

## How to check

    pnpm --dir website check
    pnpm --dir website build
    pnpm --dir website dev   # then open http://localhost:4321

## Found, not done

- The Developers footer links and the future Docs link have no pages until W5. The GitHub and npm links need a public repo and a reserved scope.
- The 390px phone layout was checked by measurement and a few screenshots, not on a real device.
