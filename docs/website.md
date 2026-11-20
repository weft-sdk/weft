# Website

The landing page and docs site. This doc records the tool choice, the approved visual direction and the ideas worth exploring.

## Goal

A site that feels like a product people want to try, not a standard open-source documentation site. It should show Weft working, not only describe it.

## Tool choice: Astro

**Astro** is the framework. It has no fixed look, so the design is fully ours:

- Any design, animation, 3D or interaction.
- React (or Svelte/Vue) components where interactivity is needed, plain HTML elsewhere ("islands").
- Only interactive parts ship JavaScript, so a visually heavy site still loads fast.
- Built-in View Transitions for animated navigation between pages.
- First-class Markdown/MDX for docs.

**Starlight** is Astro's docs theme. It's what produces the standard documentation look. We either restyle it completely or build our own docs layout. Either way the docs match the landing page.

Next.js was considered. It's the better choice for app-like sites with accounts and dashboards. The marketing site and docs are content-first, so Astro fits better. The cloud dashboard is a separate React app ([dashboard.md](dashboard.md)).

| Part | Tools |
| --- | --- |
| Landing page | Custom Astro, React islands, GSAP or Motion, Three.js / React Three Fiber if 3D is used |
| Docs | Astro + MDX, custom design |
| Playground | Weft compiler compiled to WebAssembly, running in the browser |
| Styling | Hand-written CSS with design tokens ([ADR 0017](decisions/0017-website-terminal-direction.md)) |

## Visual direction: the terminal

Approved by the maintainer on 2026-11-21 ([ADR 0017](decisions/0017-website-terminal-direction.md)). Reached over six rounds of mockups; what was tried and rejected is in [history](history.md#5-website).

**The idea.** The page reads like one terminal session, top to bottom. Each section opens with its command as a prompt line (`$ weft transports`, `$ weft check`) and its output follows at full width: a table, code, a grid of logos, or a single statement set large. There is no sidebar or command column.

### Hard rules

From the maintainer. They apply to every page of the site.

- No gradients.
- No made-up stats or numbers. Every figure must be real.
- No pill shapes: no fully rounded tags, buttons or tabs.
- One call to action, not the same button repeated in each section.
- Precise, not decorative. No emojis.
- Content runs nearly full width. Small side gutters, no wide empty margins.
- Backgrounds are never pure black or pure white, one shade off each.
- One accent colour, used sparingly.
- Hero and section visuals stay clean and minimal. No busy diagrams.

### Colour

Dark is the default theme; light is the alternative. Both are near-neutral.

| Token | Dark | Light | Used for |
| --- | --- | --- | --- |
| `--bg` | `#111111` | `#f4f4f3` | Page background |
| `--raised` | `#181818` | `#ececea` | Code blocks, hovered rows |
| `--line` | `#262626` | `#dcdcd9` | Rules between sections, table rows, borders |
| `--fg` | `#ececea` | `#141414` | Text |
| `--muted` | `#a2a2a0` | `#5e5e5c` | Secondary text |
| `--faint` | `#5f5f5d` | `#9a9a97` | Labels, comments, keywords |
| `--accent` | `#7d92ff` | `#2447f5` | Prompt `$`, namespace names, the cursor, the call to action, "now" in status |
| `--on-accent` | `#0d0d14` | `#ffffff` | Text on the accent |

### Type

| Role | Face | Where |
| --- | --- | --- |
| Base | JetBrains Mono | Navigation, prompts, tables, code, labels. The page's default |
| Display | Bricolage Grotesque, weight 600, tight tracking | Only where the output is a statement: the headline and section leads |
| Reading | Instrument Sans | Paragraphs longer than a line |

Fonts are self-hosted from Fontsource packages, not loaded from Google Fonts.

### Layout

- Side gutters `clamp(16px, 2vw, 32px)`. Content spans the full width between them.
- Sections are separated by a 1px rule, not by cards or background bands.
- Explanatory sections use a two-part split: a statement and a short paragraph on the left, the output (code or terminal) on the right. Below 900px everything is one column.
- Code and terminal output sit on `--raised` with a 1px border and a 3px radius. Nothing else has a radius above 3px.

### Logos

Language and transport logos come from Simple Icons, drawn in one colour. Transports without a logo (HTTP, gRPC, WebSockets, server-sent events, cloud queues) use simple line glyphs at the same size. The footer states that logos belong to their owners and Weft is not affiliated with them. Each project's trademark guidelines are checked before launch (task W7).

### Motion

Only the blinking cursor after the headline moves. Hover states change colour or background, never position. With reduced motion, the cursor stops blinking.

### Honesty

Every command the product does not have yet is labelled as planned. Nothing invites the reader to install or run anything until it is published.

## Ideas to explore

Not commitments. Ideas that use the product itself as the visual:

1. **Live playground.** Write a contract and see generated code in all 13 languages instantly, with no server. Possible because the compiler is Rust and compiles to WebAssembly.
2. **Transport switcher hero.** Click HTTP → Kafka → WebSockets: the `weft.*` line stays the same, and the library underneath swaps with its logo.
3. **Persistent language picker.** Choose Go once and every example on every page shows Go.
4. **Animated service map.** Services as nodes, messages moving between them, each transport in its own colour. Could reuse dashboard components.
5. **Before and after.** Forty lines of setup across KafkaJS, Express and Socket.io collapse into a few lines of Weft.
6. **Live compatibility table.** The nightly [conformance](conformance.md) results, showing exactly what is verified for each language and library.

## Docs structure (draft)

- Getting started (per language)
- Concepts
- Contracts (language reference)
- Transports (one page each, every example in every language)
- Libraries (one page per adapter: setup, options, limits)
- Dashboard
- CLI reference
- Migrating existing code (per library: from KafkaJS, from Express, ...)
