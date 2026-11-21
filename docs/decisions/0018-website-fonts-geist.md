# ADR 0018: Website fonts: Geist and Geist Mono

- Status: Accepted (2026-11-21)

**Context.** [ADR 0017](0017-website-terminal-direction.md) set three faces: JetBrains Mono as the base, Bricolage Grotesque for statements and Instrument Sans for paragraphs. Once the landing page was built, the maintainer asked for a font that feels more like developer tooling, chose Geist for the headline, and then asked for Geist across the whole site.

**Decision.** One family. **Geist** (variable) for all text: headlines and section statements at weight 600 with tracking -0.045em, everything else at regular weight. **Geist Mono** (variable) only for code, commands, prompt lines and file names. Both are self-hosted from `@fontsource-variable/geist` and `@fontsource-variable/geist-mono`.

**Alternatives considered.** For the headline, shown side by side with the real hero copy: Geist Mono, Martian Mono, JetBrains Mono bold, IBM Plex Mono and Space Mono. For the whole site, Geist was asked for directly. Setting code in Geist (not mono) was ruled out because code must keep its alignment.

**Consequences.** The page reads as one voice and loads two font files instead of three families. JetBrains Mono, Bricolage Grotesque and Instrument Sans are no longer dependencies.
