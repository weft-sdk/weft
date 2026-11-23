# Contributing to Weft

Thank you for helping. This page tells you how to build Weft, run its tests and send a change.

## Set up

You need Rust 1.95 (pinned in `compiler/rust-toolchain.toml`), Node 22.23.4 or later with pnpm 11, and Go 1.26 or later. Docker is needed only for the Kafka tests.

```sh
git clone https://github.com/weft-sdk/weft && cd weft
make help          # every command
make check         # what CI runs for the compiler and the website
make ts-install && make ts-check   # the TypeScript packages
make go-check      # the Go module
make conformance   # the shared test suite, across every language package
```

The Kafka and cross-language tests need a broker: `make ts-kafka-up`, then `make ts-kafka-test`, `make go-kafka-test` or `make conformance-table`.

## Where things live

| Path | What is in it |
| --- | --- |
| `compiler/` | The Rust workspace: parser, checker, code generators and the `weft` CLI |
| `sdks/typescript/` | The TypeScript packages |
| `go/` | The Go module |
| `conformance/` | The shared test cases that every language package must pass |
| `spec/` | Language-neutral specs: shared behaviour, the runner protocol and the IR schema |
| `docs/` | The documentation and the decision records |

Start with [docs/09-architecture.md](docs/09-architecture.md) for how the pieces fit.

## Sending a change

1. **Open an issue first** for anything bigger than a small fix, so we agree on the approach before you write it.
2. **Work on a branch** and keep commits small and focused. Each should build and pass its checks. Say *why* in the commit message when it is not obvious.
3. **Add tests.** A bug fix gets a test that fails without it. A new behaviour that every language must share goes in `conformance/cases/` as well.
4. **Update the docs** that the change touches, in the same pull request. A new or changed decision gets a record in [docs/decisions/](docs/decisions/).
5. **Run the checks** for the part you changed (see above). CI runs them on Linux and Windows for every pull request, and on macOS weekly.
6. **Open a pull request.** Say what changed, why, how you checked it, and what it does not cover.

## Conventions

- **Rust:** `cargo fmt`, and clippy with warnings denied. `unsafe` is forbidden. A new crate dependency needs a reason in the commit message.
- **TypeScript:** strict mode, exact dependency versions, tests with Vitest, pnpm only.
- **Go:** standard library first, `gofmt` and `go vet` clean, tests run with `-race`.
- **Docs:** plain words, short sentences, active voice. Tables for comparisons.
- **Snapshots:** `make snapshots` accepts changed snapshot tests. Read the `.snap` diff before you commit it.

## Behaviour that must match

Every language package behaves the same, and the conformance suite is what proves it. If you change how validation, encoding, retries, errors or the message envelope work, the change starts in [spec/behaviour.md](spec/behaviour.md) and `conformance/cases/`, and then every package follows.

## Code of conduct

Be kind and be direct. Disagree with ideas, not people. Harassment of any kind is not welcome, and maintainers may remove comments, issues or contributors that cross the line.

## License

By contributing you agree that your work is licensed under the [Apache License 2.0](LICENSE).
