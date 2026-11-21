# Decision records

Why Weft is the way it is. One file per decision, numbered in the order they were made.

## Rules

- A decision that outlives one branch gets a record here, in the same pull request as the docs and code it affects.
- Records are never deleted or rewritten. To change a decision, write a new record and mark the old one `Superseded by NNNN` (or `Amended by NNNN`).
- Status is one of: **Proposed** (a working default, open to change), **Accepted**, **Superseded by NNNN**, **Amended by NNNN**.
- Only the maintainer accepts a record that changes direction.

## Index

| # | Decision | Status |
| --- | --- | --- |
| [0001](0001-library-not-scanner.md) | Weft is a communication library, not a scanner | Accepted |
| [0002](0002-contracts-with-codegen.md) | Language-neutral contracts with code generation | Accepted |
| [0003](0003-native-package-per-language.md) | A native package per language, wrapping existing libraries | Accepted |
| [0004](0004-rust-for-shared-tooling.md) | Rust for everything shared | Accepted |
| [0005](0005-own-contract-language.md) | Our own contract language | Accepted |
| [0006](0006-transport-neutral-events.md) | Events are transport-neutral | Accepted |
| [0007](0007-escape-hatch.md) | Always provide an escape hatch | Accepted |
| [0008](0008-conformance-gates-releases.md) | Conformance suite gates every release | Accepted |
| [0009](0009-dashboard-three-modes.md) | Dashboard in three modes, one codebase | Accepted |
| [0010](0010-website-on-astro.md) | Website on Astro with a custom design | Accepted, amended by 0017 |
| [0011](0011-shared-runtime-standards.md) | Shared runtime standards | Accepted |
| [0012](0012-where-contracts-live.md) | Where shared contracts live | Accepted |
| [0013](0013-open-core.md) | Open core | Accepted |
| [0014](0014-name-weft.md) | Product name is Weft, published under `weft-sdk` | Accepted |
| [0015](0015-rpc-uses-connect.md) | `weft.rpc` uses the Connect protocol | Accepted |
| [0016](0016-website-first.md) | Build the website first | Accepted |
| [0017](0017-website-terminal-direction.md) | Website visual direction: the terminal | Accepted, amended by 0018 |
| [0018](0018-website-fonts-geist.md) | Website fonts: Geist and Geist Mono | Accepted |
| [0019](0019-site-written-as-launch-page.md) | The website is written as the launch page | Accepted |
| [0020](0020-product-build-order.md) | Product build order after the website | Accepted |

## Template

```markdown
# ADR NNNN: <decision in a few words>

- Status: Proposed | Accepted (YYYY-MM-DD) | Superseded by NNNN

**Context.** What forced a decision.

**Decision.** What we chose.

**Alternatives considered.** What else, and why not.

**Consequences.** What follows, good and bad.
```
