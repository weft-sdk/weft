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

- Round 2 feedback (2026-11-21): looked very bad. Wants a little colour (an accent) and a clean, minimal hero visual; the weave visual is rejected outright. Do not go back to busy or diagrammatic hero visuals.
- Round 3 answers: off-black and off-white stay; one accent colour (blue `#2447f5` light / `#7d92ff` dark; orange and green offered through a mockup-only switcher in the header). The hero visual is one `weft.*` call set large, with the transport name in the accent; a row of transport logos above it and a small language picker below change the call and the library underneath. The rest of the page is unchanged from round 2. The file keeps its name `b-weave.html` so the preview link stays the same.

- Round 3 feedback (2026-11-21): accent chosen, **the first one, blue** (`#2447f5` light, `#7d92ff` dark). The page structure felt like everything was dumped in a stack; wants a better, new layout with good references.
- Round 4 answers: rebuilt the page structure.
  - Every section uses one split header: eyebrow and title on the left, the explanation on the right.
  - Each section then holds one composed object, not loose stacked panels.
  - Hero: headline, then one product window. Transport tabs and a language picker sit in the window. The code highlights the call line, and a side panel names the library underneath with its `weft.yaml` lines. A single row of framework logos follows.
  - "Why Weft" is a bento grid of the README's "What you get" points.
  - Before and after is a split: a sticky caption with a Without/With toggle, and one code window.
  - The rest: three steps, a tile grid of language logos, the dashboard window, and status as three phases with "Now" in the accent. The footer has columns.
  - Accent switcher removed. Escape hatch now uses `raw`, as in `api.md`.

## Done

- Work file (b2b599e).
- Mockups, both full landing pages with real copy from the design docs:
  - `w1-mockups/a-precise.html`: dark-first, Geist and Geist Mono, a visible hairline 12-column grid, one blue accent. Hero is a transport switcher (13 namespaces; the code line and the library underneath change). Sections: problem table, three steps, transports grid, languages grid, status. Private preview: https://claude.ai/artifact/9DQk27ezbFVvHSe45FXiUt
  - `w1-mockups/b-weave.html`: light-first, Bricolage Grotesque, Instrument Sans and JetBrains Mono. Hero is a weave: six services as vertical warp lines, six transports as flat coloured threads with over/under crossings and moving messages; choosing a thread shows its call. Sections: before/after code (Express + KafkaJS + Socket.io vs Weft), contract with TypeScript/Go/Python tabs marked "Planned API", transports list, status. Private preview: https://claude.ai/artifact/LBp7oBcKiFhCpVSUeMZDVG
  - Both: light and dark themes with a toggle, phone width, keyboard focus, reduced motion respected, one call to action.
- Round 2 of B, same file and preview link (version 2). Monochrome, real logos, more sections. Logos are Simple Icons 13.21.0 paths (CC0) inlined into the page. gRPC, HTTP, WebSockets, SSE and cloud queues have no logo there, so they use simple line glyphs. Checked with headless Chrome screenshots at desktop width; fixed a CSS selector that stretched the logo icons.

## Next step

Get the maintainer's reaction to round 4 (the new structure). Revise if asked. Once approved, write the direction into `docs/website.md` (tokens, type, weave, motion, layout and the hard rules) and ADR 0017, then delete `a-precise.html`.

## Waiting on

- The maintainer's verdict on round 4.

## How to check

    open docs/work/w1-mockups/a-precise.html
    open docs/work/w1-mockups/b-weave.html

## Found, not done

- Using other projects' logos on the real site needs a check of each project's trademark guidelines, and an "not affiliated" line (round 2 has one).
- Call shapes for Rust, PHP, Elixir, Swift and Dart in the weave are guesses in the style of `api.md`; the docs do not define them yet. The weave's call panel is labelled as planned.

- The roadmap's W3 call to action says "GitHub", but the repository is private. The landing page needs a call to action that works before the repo is public.
