# Open questions

Decisions waiting on the maintainer. Do not build around an open question a task depends on: ask instead. When one is answered, remove it here and record the answer in a [decision record](decisions/) or the relevant doc, in the same change.

| # | Question | Blocks | Notes |
| --- | --- | --- | --- |
| Q1 | Website visual direction: look, feel, references | W1 and everything after it | Deliberately left open in [ADR 0010](decisions/0010-website-on-astro.md). Settled with the maintainer in task W1 |
| Q2 | Product build order after the website | Phase P1 onward | Default proposal in the [roadmap](roadmap.md): compiler + CLI, then TypeScript, then Go |
| Q3 | Domain | Maven group `dev.weft`, Go vanity import path, website hosting URL | weft.dev looked unregistered on 2026-11-20. Maintainer deferred buying it |
| Q4 | Cloud auth provider: WorkOS or Clerk | Weft Cloud only | Not blocking anything before Weft Cloud |
