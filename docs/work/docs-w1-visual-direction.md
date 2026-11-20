# W1: Website visual direction

- Branch: `docs/w1-visual-direction`
- Task: W1 in the roadmap, needs the maintainer (Q1)
- Started: 2026-11-21 · Last session: 2026-11-21
- Pushed: no

## Goal

The maintainer has approved a visual direction for the website in writing. The direction (palette, type, motion, layout principles, references) is written into `docs/website.md` and a decision record, and Q1 is removed from `open-questions.md`.

## Plan

1. Build two throwaway HTML mockups of the landing page, one per shortlisted direction, in `docs/work/w1-mockups/`. They are exploration only and are deleted with this work file.
2. The maintainer reviews them and picks one direction, or a mix.
3. Write the chosen direction into `docs/website.md` (tokens, type, motion, layout, rules) and add ADR 0017.
4. Remove Q1 from `open-questions.md`, mark W1 done in the roadmap, update Current status.

## Decisions made on this branch

- Shortlist from the maintainer (2026-11-21): mock up direction **A, precise and technical** (dark-first, monospace accent, thin lines, little colour, quiet motion; references Linear, Vercel, Zed) and direction **B, the product as the visual** (a woven look on the name Weft, each transport a coloured thread through the services, an animated service map in the hero). Direction C (warm and editorial) was dropped.
- Hard rules from the maintainer for every direction:
  - No gradients.
  - No fake stats or invented numbers.
  - No pill shapes (fully rounded tags, buttons or tabs).
  - No repeated buttons. One clear call to action, not the same button in every section.
  - Precise, not decorative.
  - No emojis.
  - Content runs nearly full width. Small side gutters, no wide empty margins left and right.

## Done

- Work file (this commit).

## Next step

Build mockup A and mockup B in `docs/work/w1-mockups/`, then show both to the maintainer.

## Waiting on

- The maintainer's choice between A and B, or a mix, after seeing the mockups.

## How to check

    open docs/work/w1-mockups/a-precise.html
    open docs/work/w1-mockups/b-weave.html

## Found, not done

- The roadmap's W3 call to action says "GitHub", but the repository is private. The landing page needs a call to action that works before the repo is public.
