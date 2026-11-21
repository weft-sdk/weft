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
| `--line-hover` | `#3d3d3c` | `#bdbdba` | Border of a control on hover, eased in over about 200ms, a step up from `--line` and softer than `--faint` |
| `--fg` | `#ececea` | `#141414` | Text |
| `--muted` | `#a2a2a0` | `#5e5e5c` | Secondary text |
| `--faint` | `#82827f` | `#686866` | Labels, comments, keywords. Meets 4.5:1 contrast on `--bg` and `--raised` |
| `--accent` | `#7d92ff` | `#2447f5` | Prompt `$`, namespace names, the lower thread of the logo, the call to action, "now" in status |
| `--accent-soft` | `#1b2142` | `#e5e9fd` | Background of highlighted code lines |
| `--on-accent` | `#0d0d14` | `#ffffff` | Text on the accent |
| `--del`, `--del-soft` | `#ff7b72`, red at 10% | `#b3202c`, red at 8% | Removed lines, in the before/after comparison only |
| `--add`, `--add-soft` | `#56d364`, green at 10% | `#116329`, green at 10% | Added lines, in the before/after comparison only |

Section headings (`.lead`), intros (`.intro`), the spacing under them (`--stack-lead` 16px, `--stack-content` 32px, `--stack-row` 40px) and motion (`--t-fast` 200ms for controls, `--t-dim` 320ms with `--ease-dim` for dimmed neighbours) are defined once, in `tokens.css` and `global.css`. A section never restyles its own heading; it only sets the space under it or lifts the width limit. This keeps the sections from drifting apart.

### Type

One family, Geist ([ADR 0018](decisions/0018-website-fonts-geist.md)).

| Role | Face | Where |
| --- | --- | --- |
| Text | Geist | All text: navigation, paragraphs, labels, table text. The page's default |
| Statements | Geist, weight 600, tracking -0.045em | The headline and section leads |
| Code | Geist Mono | Code, commands, prompt lines, file names, namespaces |

Fonts are self-hosted from Fontsource packages, not loaded from Google Fonts.

### Layout

- Side gutters `clamp(16px, 2vw, 32px)`. Content spans the full width between them.
- Sections are separated by a 1px rule, not by cards or background bands.
- Explanatory sections use a two-part split: a statement and a short paragraph on the left, the output (code or terminal) on the right. Below 900px everything is one column.
- Code and terminal output sit on `--raised` with a 1px border and a 3px radius. Nothing else has a radius above 3px.
- The setup section is four open columns, one per transport, separated by thin vertical rules with no outer frame. Each column shows the picked library as a large logo, a list of the alternatives (a native radio group), and the `weft.yaml` lines that choice writes, with the library line tinted. Picking a library swaps the logo and rewrites the line. Questions, libraries and keys come from [cli.md](cli.md).
- The contract section has two parts. At the top, the heading and intro sit on the left and the contract on the right (a strip in two halves under the Weft mark: the `Order` type, then `Money` and the event). A line labelled `weft gen` runs down from the contract to five equal open columns, like the setup section: the same type as `weft gen` writes it in TypeScript, Go, Python, Rust and Java, each under its large logo, in boxes of equal height. Generated code follows the type-mapping table in [contract-language.md](contract-language.md) and the naming of each language; the Java record uses a `@Nullable` annotation for the optional field. Hovering one language column softly dims the others. Below 1100px the heading, then the contract, stack, and the languages sit in two columns; below 640px everything is one column.
- The languages section is the twelve languages (TypeScript and JavaScript are listed separately; the JVM and .NET families count once each) on a six-column grid, two even rows of six, between two thin rules, each a 36px colour logo in a fixed square box (so wide and square logos weigh the same and the names line up) and a name at 1.625rem. Pointing at one, or tabbing to it, fades the rest to 45% and shows that language's details from [languages.md](languages.md) on a single caption line underneath, which keeps its height so nothing shifts. Its heading stays on one line while it fits. The grid drops to four columns below 1200px, three below 760px and two below 520px. There is no frame and no section prompt line.
- The shared-behaviour section is a defaults sheet in three open columns (Messaging, Calls, Everywhere) on one grid so the rows line up. Each default is a big value, a line of text and a small picture: five squares then a dashed `dlq` box for five attempts, a path of mono boxes for acknowledgement, deadlines and shutdown, and chips for the key, the CloudEvents attributes and the trace carriers. Values come from the defaults in [sdk-design.md](sdk-design.md).
- The breaking-changes section has the real `weft check` output beside the heading, and under it a ledger sorting the common changes into safe, risky and breaking, each with a one-line example and its own icon (a shield with a tick, a warning triangle, an octagon with a cross). Safe and risky are narrow columns, breaking is wide and runs in two columns. Changes follow the table in [contract-language.md](contract-language.md).
- The dashboard section has three cards beside the heading (on your laptop, in your cluster, in Weft Cloud, each with an icon, one line and how you run it, from [dashboard.md](dashboard.md)). Under it is a full-width trace of one order across HTTP, gRPC, Kafka and a WebSocket, in four languages: each span has its operation and transport, its service with the language logo, its duration, and a waterfall with a time axis, and the header carries the trace id (the example id from the W3C Trace Context spec), the total time and the span and service counts. The timings are sample values, so the trace is labelled "example trace". Pointing at a span fades the others and shows what ran, where and for how long on a fixed-height line. A strip of what the dashboard shows closes the section, set a little below the trace with short dividers centred between its four columns.
- The closing section mirrors the hero without copying it. The commands window is on the left: a plain `weft --help` listing of every CLI command from [cli.md](cli.md), each with its one-line description, in a raised window with a mono title bar ("weft --help", with the count computed from the list); pointing at a row fades the others to 45%. On the right are the heading (section-sized, not hero-sized), the intro, the copyable install command (the copy icon turns into a tick) and the Apache-2.0 note. There is no second Get started button, since it would point at this section. Below 900px the text comes first and the window follows.
- The navbar is sticky. It has the mark, five links (transports, compare, languages, dashboard, docs) and the theme toggle. There is no Get started button, because the hero already has one. Below 900px the links collapse into a menu button that opens a panel under the bar (it closes on a link, Escape or a wider screen). A GitHub link joins the footer, not the navbar.
- The footer is a brand column, then four open columns with a thin vertical rule between them, running the full height. The brand column holds the mark and a one-line description at the top and three ruled counts at the bottom: 13 transports, 12 languages, 1 contract (counted from the same data as the sections). Groups are stacked inside the columns so all four end at about the same height, as in the Stripe and Anthropic footers: Request and response with Realtime (the transports' own folders), Messaging, Documentation, and Product with Project (GitHub and npm, each once, with its logo). The Documentation and Transports links are built from `src/data/docs.ts`. There is no bottom row and no licence or logo-ownership text, and nothing is listed twice. Below 900px the mark sits on top with the columns in two rows, and below 520px they stack with rules between them. The Project links work once the repository is public and the npm scope is reserved.
- The before/after comparison is a side-by-side git diff: two columns split by a 1px vertical rule, removed lines in red with `-` on the left, added lines in green with `+` on the right, each with line numbers. The `-13` and `+10` counts are computed from the code shown. Red and green appear nowhere else on the site.

### Logos

Language and transport logos come from Simple Icons, drawn in one colour. Transports without a logo (HTTP, gRPC, WebSockets, server-sent events, cloud queues) use simple line glyphs at the same size. The footer carries no logo-ownership notice (the maintainer removed it). Each project's trademark guidelines are still checked before launch (task W7).

### Motion

One thing moves on its own: the hero terminal plays its session once on load (commands type out, output appears line by line), then stays still. Every line keeps its space while it plays, so nothing shifts. With reduced motion it shows complete at once. Hover states change colour or background, never position.

### Honesty

The site is written as the launch page of the finished product ([ADR 0019](decisions/0019-site-written-as-launch-page.md)). Every claim must match the design docs, and there are no made-up numbers. The site is not deployed publicly until every claim on it is true of shipped code.

## Ideas to explore

Not commitments. Ideas that use the product itself as the visual:

1. **Live playground.** Write a contract and see generated code in all 13 languages instantly, with no server. Possible because the compiler is Rust and compiles to WebAssembly.
2. **Transport switcher hero.** Click HTTP → Kafka → WebSockets: the `weft.*` line stays the same, and the library underneath swaps with its logo.
3. **Persistent language picker.** Choose Go once and every example on every page shows Go.
4. **Animated service map.** Services as nodes, messages moving between them, each transport in its own colour. Could reuse dashboard components.
5. **Before and after.** Forty lines of setup across KafkaJS, Express and Socket.io collapse into a few lines of Weft.
6. **Live compatibility table.** The nightly [conformance](conformance.md) results, showing exactly what is verified for each language and library.

## Docs layout (W5)

Chosen by the maintainer as a mix of three options: section tabs on top, a directory tree for the current section on the left, and an outline on the right.

- **Section bar.** Under the site header: Guides, Contracts, Transports, Libraries, CLI, Dashboard, with a search button at the right (⌘K, Ctrl+K or `/`). It stays pinned.
- **Tree.** A mono directory tree for the current section only: `section/`, its folders, then its pages, drawn with line characters. The current page is the accent colour. It sticks while you scroll.
- **Page.** Breadcrumb, title, summary, then the body, then previous/next links within the section. Code examples use language tabs; the choice is remembered in the browser and applies to every tab group on every page.
- **Outline.** "On this page" on the right, with the heading nearest the top marked.
- **Search.** A dialog that filters a build-time index of every page and section in the browser. Arrow keys and Enter work. No server.
- **Narrow screens.** Below 1200px the outline is dropped. Below 1000px the tree moves behind a Contents button and the section bar scrolls sideways.
- **Docs home** (`/docs`): the search box and the six sections as open ruled columns.
- **Content.** The structure lives in `website/src/data/docs.ts`; routes are `/docs`, `/docs/<section>` and `/docs/<section>/<page>`. Pages show a title and summary until their content is written, except Events, which demonstrates the page parts. The code in its non-TypeScript tabs is illustrative and is replaced when real generated output exists.

## Docs structure (draft)

- Getting started (per language)
- Concepts
- Contracts (language reference)
- Transports (one page each, every example in every language)
- Libraries (one page per adapter: setup, options, limits)
- Dashboard
- CLI reference
- Migrating existing code (per library: from KafkaJS, from Express, ...)
