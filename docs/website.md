# Website

The landing page and docs site. **Visual design is not decided yet.** This doc records the tool choice and the ideas worth exploring.

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
| Styling | Tailwind or hand-written CSS, decided with the design |

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
