# ADR 0017: Website visual direction: the terminal

- Status: Accepted (2026-11-21). Amended by [0018](0018-website-fonts-geist.md): the fonts are now Geist and Geist Mono.

**Context.** [ADR 0010](0010-website-on-astro.md) left the website's look open ([Q1](../13-open-questions.md)), and [ADR 0016](0016-website-first.md) made the website the first thing built. Task W1 settled the look with the maintainer through six rounds of mockups.

**Decision.** The site reads like one terminal session. Each section opens with its command as a prompt line and its output follows at full width. Dark is the default theme, both themes use near-neutral backgrounds that are not pure black or white, and one blue accent is used sparingly. JetBrains Mono is the base face, Bricolage Grotesque is for statements, and Instrument Sans is for paragraphs. Styling is hand-written CSS with design tokens. Fonts are self-hosted from Fontsource. Logos come from Simple Icons. The tokens, rules and layout are in [11-website.md › Visual direction](../11-website.md#visual-direction-the-terminal).

**Alternatives considered.** A precise hairline grid; a coloured "weave" of transports; a black-and-white weave; a single large call as the hero; a section stack with a bento grid; a type specimen; a split-screen scroll story. The rounds are in [history](../15-history.md#5-website). Tailwind was considered for styling; hand-written CSS was chosen because the design is small, token-driven and specific.

**Consequences.**
- The terminal framing suits a developer tool and keeps every section honest: commands the product lacks are marked as planned.
- Mono as the base face costs some reading comfort, so paragraphs use Instrument Sans.
- Showing other projects' logos needs each project's trademark guidelines checked before launch (W7).
