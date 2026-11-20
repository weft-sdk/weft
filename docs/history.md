# History

How Weft got to its current shape, including what was considered and rejected. The design was worked out in one long conversation on 2026-11-20 and 21. That conversation is not kept, so everything that matters from it is recorded here, in the [decision records](decisions/) and in the design docs.

Do not reopen these decisions unless the maintainer asks.

## 1. It started as "Splice", described as a code scanner

The repository began with a README for **Splice**: a tool that statically scanned codebases to map which services talk to which (REST, GraphQL, gRPC, Kafka, WebSockets) and blocked pull requests that would break another team's consumer. A unified runtime API (`splice.kafka.publish(...)`) appeared only as an optional later idea.

Working from that README, a full set of scanner design docs was written (manifest format, rule packs, tree-sitter scanning, a registry, consumer-aware breaking-change checks) and an illustrative web page of the scanner in use was published.

**The maintainer then clarified that README was outdated.** The real product is the unified communication library. The scanner was never the plan. All scanner docs were deleted ([ADR 0001](decisions/0001-library-not-scanner.md)).

What survived from the scanner phase, reshaped:

- **Breaking-change checks** now compare contract versions (`weft check`), not scanned code.
- **The service map** is now built from contracts and runtime telemetry, shown in the dashboard.
- **Rust** was first chosen for the scanner. It stayed, for the shared tooling (compiler, CLI, LSP, conformance harness, dashboard server).

## 2. What Weft is

Developers write all service communication through one interface (`weft.http`, `weft.gql`, `weft.grpc`, `weft.rpc`, `weft.ws`, `weft.sse`, `weft.kafka`, `weft.mq`, `weft.nats`, `weft.redis`, `weft.cloud`, `weft.mqtt`, `weft.pulsar`). Weft runs the real library underneath (Express, Apollo, KafkaJS, Socket.io, ...). The flow: `weft init` asks which transports and which library for each, contracts are written once in `.weft` files, `weft gen` produces typed code, and the developer calls `weft.*`.

## 3. Multi-language architecture

The maintainer required support for every common language. Options considered:

- **One Rust core with bindings for each language** (like Temporal's SDK core). Rejected: it would replace teams' existing libraries with Rust ones and make installs heavier ([ADR 0003](decisions/0003-native-package-per-language.md)).
- **Contracts written as TypeScript types.** Rejected: a Go or Python service can't read them ([ADR 0002](decisions/0002-contracts-with-codegen.md)).
- **Chosen:** language-neutral `.weft` contracts, one Rust compiler generating code for each language, and a small native package per language with adapters over its existing libraries.

13 languages are in scope: TypeScript/JavaScript, Go, Python, JVM (Java, Kotlin, Scala, Clojure), .NET (C#, F#), Rust, Ruby, PHP, Elixir/Erlang, Swift, Dart, plus browser and mobile clients for the client-side transports. Full matrix: [languages.md](languages.md). The maintainer asked explicitly that plain JavaScript be supported, not only TypeScript.

TypeSpec was considered as the contract language and rejected as too HTTP-centred ([ADR 0005](decisions/0005-own-contract-language.md)).

## 4. Dashboard

First proposed as a later, optional paid product. The maintainer wanted it to work both locally on a port (like RabbitMQ's management UI) and through sign-in on the website, so it became three modes with one codebase ([ADR 0009](decisions/0009-dashboard-three-modes.md)). Both local and self-hosted are free; Weft Cloud is paid ([ADR 0013](decisions/0013-open-core.md)).

## 5. Website

The maintainer has high hopes for the website and does not want it to look like a standard open-source documentation site. Astro was chosen for full design freedom, with Starlight either restyled or replaced ([ADR 0010](decisions/0010-website-on-astro.md)). The maintainer then chose to **build the website first**, before any product code ([ADR 0016](decisions/0016-website-first.md)). The visual design is not decided ([open question Q1](open-questions.md)).

## 6. The name

"Splice" was dropped: an established software company has that name (trademark risk), it collides with JavaScript's `Array.prototype.splice` in search, and its handles and domains were taken. Candidates considered: Weft, Parley, Tether, Hitch, Lace, Franca, Patchbay. **Weft** was chosen (the threads woven across a loom that bind everything into one fabric).

For the GitHub and npm organisation, `weftdev`, `weftio`, `weftloom`, `weftwire` and others were offered. The maintainer chose **`weft-sdk`** ([ADR 0014](decisions/0014-name-weft.md)). The GitHub org `weft-sdk` and private repo `weft-sdk/weft` were created on 2026-11-20.

## 7. Contract language

The syntax was decided on 2026-11-20 by choosing between alternatives, recommended option each time: `name: type` fields; a single "optional" concept (`name?: T`, absent or null); `int`/`float` as 64-bit defaults plus sized types; `T[]` and `map<K, V>`; `->` for every result; event names as topic names (remappable in `weft.yaml`); built-in plus custom errors; one line per HTTP route; simple generics; a fixed set of validation annotations; imports by package name only; package semver enforced by `weft check`. See [contract-language.md](contract-language.md).

## Advice given along the way that still stands

- **The biggest risk is not technical.** Developers are often wary of abstraction layers over their libraries ("it will hide features" or "only the lowest common denominator"). The `raw` escape hatch and adapter `capabilities()` exist for this. Showing the idea to 5–10 backend developers early is the cheapest way to test it.
- **Not every language on day one.** The architecture makes each new language a code generator, a core package and adapters. Build order is still an [open question](open-questions.md).
