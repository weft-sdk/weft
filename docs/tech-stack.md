# Tech stack

Rust for everything shared. Native code for each language package. One conformance suite to keep them identical.

## Compiler and CLI (Rust)

| Concern | Tool |
| --- | --- |
| Parser | chumsky |
| Error messages | miette |
| IR serialisation | serde, serde_json |
| Code generation | minijinja templates, genco |
| Importers | protobuf-parse, openapiv3, apollo-parser, apache-avro |
| CLI | clap |
| Interactive prompts | inquire |
| File watching | notify |
| Release builds | cargo-dist (macOS, Linux, Windows; x64 and ARM) |
| WebAssembly build | wasm-bindgen (compiler in the browser, for the website playground) |

## Editor tooling

| Concern | Tool |
| --- | --- |
| Language server | tower-lsp (Rust) |
| Syntax highlighting | tree-sitter grammar (Neovim, Zed, Helix, GitHub) |
| VS Code extension | TypeScript |
| JetBrains plugin | Kotlin, using the language server |

## Language packages

| Language | Written in | Build | Tests | Registry |
| --- | --- | --- | --- | --- |
| TypeScript / JS | TypeScript | tsup (ESM + CJS) | Vitest | npm, JSR |
| Go | Go | go | go test | Go modules |
| Python | Python (typed) | uv, hatch | pytest | PyPI |
| JVM | Kotlin, Java-friendly API | Gradle | JUnit 5 | Maven Central |
| .NET | C# | dotnet | xUnit | NuGet |
| Rust | Rust | cargo | cargo test | crates.io |
| Ruby | Ruby + RBS | bundler | RSpec | RubyGems |
| PHP | PHP 8.2+ | composer | PHPUnit | Packagist |
| Elixir | Elixir | mix | ExUnit | Hex |
| Swift | Swift | SwiftPM | XCTest | SwiftPM |
| Dart | Dart | pub | dart test | pub.dev |

## Shared standards

| Concern | Standard |
| --- | --- |
| Encoding | JSON (default), Protobuf, Avro, MessagePack |
| Event envelope | CloudEvents 1.0 |
| Tracing and metrics | OpenTelemetry, W3C Trace Context |
| HTTP errors | RFC 9457 Problem Details |
| RPC status codes | gRPC status codes |

## Conformance suite

| Concern | Tool |
| --- | --- |
| Harness | Rust |
| Test cases | YAML |
| Infrastructure | Testcontainers, Docker Compose |
| Brokers | Redpanda, RabbitMQ, NATS, Redis, LocalStack, Pub/Sub emulator, Service Bus emulator, Mosquitto, Pulsar |
| Trace assertions | OpenTelemetry Collector |

## Dashboard

| Concern | Tool |
| --- | --- |
| Server | Rust, axum, tokio |
| Telemetry intake | OTLP (HTTP and gRPC) |
| Local storage | SQLite |
| Hosted storage | Postgres (metadata), ClickHouse (telemetry) |
| UI | React, Vite, TypeScript |
| Graph rendering | React Flow; Sigma.js / WebGL for large maps |
| Charts | to decide with UI design |
| Embedding UI in binary | rust-embed |
| Cloud auth | WorkOS or Clerk (SSO, SAML) — to decide |
| Deploy | Docker image, Helm chart |

## Website

| Concern | Tool |
| --- | --- |
| Framework | Astro |
| Interactive pieces | React islands |
| Animation | GSAP or Motion |
| 3D (if used) | Three.js / React Three Fiber |
| Styling | Tailwind or hand-written CSS |
| Docs | Custom design on Astro (Starlight heavily restyled, or a custom layout) |
| Playground | The compiler built to WebAssembly |

See [website.md](website.md).

## Repo, CI and releases

| Concern | Tool |
| --- | --- |
| Repo | One monorepo |
| Toolchain versions | mise |
| Task runner | just |
| CI | GitHub Actions, matrix per language × adapter |
| Versioning, changelogs | release-please |
| Licence | Apache-2.0 for compiler, CLI, packages and local dashboard |
