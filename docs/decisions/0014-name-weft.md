# ADR 0014: Product name is Weft, published under `weft-sdk`

- Status: Accepted (2026-11-20)

**Context.** "Splice" clashed with an established software company (trademark risk) and with JavaScript's `Array.prototype.splice` (search noise), and its handles were taken.

**Decision.** The product is **Weft**: `weft.*` in code, the `weft` CLI, `.weft` contract files, `weft.yaml` config. GitHub organisation `weft-sdk`, main repo `github.com/weft-sdk/weft`.

Package names, where plain `weft` is taken:

| Registry | Name |
| --- | --- |
| npm | `@weft-sdk/*` |
| PyPI, RubyGems | `weft-core` |
| crates.io | `weft-sdk` |
| NuGet | `WeftSdk.*` |
| Packagist | `weft-sdk/*` |
| Hex, pub.dev | `weft_sdk` |
| Go | `github.com/weft-sdk/weft/go` until a domain is chosen |
| Maven | `io.github.weft-sdk` until a domain is chosen (`dev.weft` needs weft.dev) |

**Open.** Domain (weft.dev looked unregistered on 2026-11-20). Trademark search still to do.
