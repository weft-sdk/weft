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

- Round 1 feedback (2026-11-21): **B over A.** Keep B's layout and fonts (Bricolage Grotesque, Instrument Sans, JetBrains Mono) and the weave concept, but:
  - The round 1 hero read like a Jaeger trace diagram (bars between service lines). Replace it.
  - Too many colours. Go black and white.
  - Backgrounds: not pure black or pure white, one shade off each (round 2 uses `#f4f4f3` and `#111111`, text `#141414` and `#ececea`).
  - Use real logos for every transport and language, for credibility and a cleaner look.
  - Add more to the page. Cleaner, better.
  - Don't commit every mockup change.
- Round 2 answers: the hero is now a true weave of languages (warp) by transports (weft), with real over/under crossings. Gaps show unsupported combinations taken from `languages.md`. Choosing a crossing shows the call in that language and the library that runs it. Added sections: framework logo wall, how it works, contract with six language tabs (examples from `api.md`), shared behaviour, dashboard preview. Direction A is dropped.

## Done

- Work file (b2b599e).
- Mockups, both full landing pages with real copy from the design docs:
  - `w1-mockups/a-precise.html`: dark-first, Geist and Geist Mono, a visible hairline 12-column grid, one blue accent. Hero is a transport switcher (13 namespaces; the code line and the library underneath change). Sections: problem table, three steps, transports grid, languages grid, status. Private preview: https://claude.ai/artifact/9DQk27ezbFVvHSe45FXiUt
  - `w1-mockups/b-weave.html`: light-first, Bricolage Grotesque, Instrument Sans and JetBrains Mono. Hero is a weave: six services as vertical warp lines, six transports as flat coloured threads with over/under crossings and moving messages; choosing a thread shows its call. Sections: before/after code (Express + KafkaJS + Socket.io vs Weft), contract with TypeScript/Go/Python tabs marked "Planned API", transports list, status. Private preview: https://claude.ai/artifact/LBp7oBcKiFhCpVSUeMZDVG
  - Both: light and dark themes with a toggle, phone width, keyboard focus, reduced motion respected, one call to action.
- Round 2 of B, same file and preview link (version 2). Monochrome, real logos, more sections. Logos are Simple Icons 13.21.0 paths (CC0) inlined into the page. gRPC, HTTP, WebSockets, SSE and cloud queues have no logo there, so they use simple line glyphs. Checked with headless Chrome screenshots at desktop width; fixed a CSS selector that stretched the logo icons.

## Next step

Get the maintainer's reaction to round 2 of B. Revise if asked. Once approved, write the direction into `docs/website.md` (tokens, type, weave, motion, layout and the hard rules) and ADR 0017, then delete `a-precise.html`.

## Waiting on

- The maintainer's verdict on round 2 of B.

## How to check

    open docs/work/w1-mockups/a-precise.html
    open docs/work/w1-mockups/b-weave.html

## Found, not done

- Using other projects' logos on the real site needs a check of each project's trademark guidelines, and an "not affiliated" line (round 2 has one).
- Call shapes for Rust, PHP, Elixir, Swift and Dart in the weave are guesses in the style of `api.md`; the docs do not define them yet. The weave's call panel is labelled as planned.

- The roadmap's W3 call to action says "GitHub", but the repository is private. The landing page needs a call to action that works before the repo is public.
