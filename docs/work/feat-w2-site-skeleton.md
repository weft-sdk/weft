# W2: Site skeleton

- Branch: `feat/w2-site-skeleton`, stacked on `docs/w1-visual-direction` (W1 is finished there but not merged to `main` yet)
- Task: W2 in the roadmap, needs W1
- Started: 2026-11-21 · Last session: 2026-11-21
- Pushed: no

## Goal

`website/` holds an Astro + TypeScript (strict) + pnpm project with the W1 design tokens, a base layout with light and dark themes, and a CI workflow that builds it. Done when `pnpm --dir website build` passes in CI and an empty styled page renders.

## Plan

1. Scaffold `website/` by hand: `package.json`, `astro.config.mjs`, `tsconfig.json`, `src/pages/index.astro`.
2. `src/styles/tokens.css` and `global.css` from [website.md › Visual direction](../website.md#visual-direction-the-terminal).
3. `src/layouts/Base.astro`: head, self-hosted fonts, header with theme toggle, footer. The theme is set before first paint, so the page never flashes the wrong theme.
4. `.github/workflows/website.yml`: install with pnpm and build on pushes and pull requests that touch `website/`.
5. Add the commands to CLAUDE.md.

## Decisions made on this branch

- Astro 7.4.6, pinned exactly: the newest release more than two weeks old on 2026-11-21.
- Fonts from Fontsource packages (ADR 0017). Logos from the `simple-icons` package, imported at build time so no icon data ships as JavaScript.
- No React yet. The landing page needs only a theme toggle, which is a few lines of inline script. React islands come when an interactive piece needs them (W4).

## Done

- Work file (this commit).

## Next step

Scaffold `website/` (plan step 1).

## Waiting on

- The maintainer to merge `docs/w1-visual-direction` first. This branch is stacked on it.
- CI only runs once the branch is pushed, which needs the maintainer's approval.

## How to check

    pnpm --dir website install
    pnpm --dir website build
    pnpm --dir website check

## Found, not done

- Nothing yet.
