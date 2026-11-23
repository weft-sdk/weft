# Weft: guidance for coding agents

Weft is **one interface for every way services talk**. Developers call `weft.http`, `weft.gql`, `weft.grpc`, `weft.rpc`, `weft.ws`, `weft.sse`, `weft.kafka`, `weft.mq`, `weft.nats`, `weft.redis`, `weft.cloud`, `weft.mqtt` and `weft.pulsar`, and Weft runs the real library underneath (Express, Apollo, KafkaJS, Socket.io, Spring Kafka, FastAPI, ...). Contracts are written once in language-neutral `.weft` files, and a Rust compiler generates typed code for 13 languages. Overview: [README](README.md). Design: [docs/](docs/README.md).

**Status: design stage.** The website (`website/`), the compiler and CLI (`compiler/`: `weft init`, `fmt`, `check`, and `gen` for TypeScript and Go), the TypeScript package (`sdks/typescript/`: core, HTTP on Express, Kafka on KafkaJS), the Go package (`go/`: core, HTTP on net/http, Kafka on franz-go) and the conformance suite (`conformance/`) are built; nothing is published. The other language packages, the other transports and the dashboard are not. Never describe a planned feature as existing in the docs or the README. The website is the exception: it is written as the launch page ([ADR 0019](docs/decisions/0019-site-written-as-launch-page.md)) and is not deployed publicly until its claims are true.

**Weft is not a code scanner.** An outdated README once described the project (then called Splice) as one. That idea was dropped completely ([ADR 0001](docs/decisions/0001-library-not-scanner.md), [history](docs/15-history.md)). Do not reintroduce it. Do not reopen decisions in [history](docs/15-history.md) or [decisions/](docs/decisions/) unless the maintainer asks.

## The `notes/` folder is not for you

If a `notes/` folder exists, it holds the maintainer's personal notes. Do not read, follow or edit it, and do not treat it as project state, unless the maintainer explicitly asks. Project state lives in `docs/`.

## Starting a session

Work is handed off through the repository, never through a conversation. Full procedure: [docs/14-workflow.md](docs/14-workflow.md). In short:

1. `git status`, `git branch --list`, `git worktree list`, `git fetch`.
2. **On a task branch:** read `docs/work/<branch with / replaced by ->.md` and continue from its **Next step**. Do not re-plan what it already decided.
3. **On `main`:** read [Roadmap › Current status](docs/12-roadmap.md#current-status), take the first task whose dependencies are done and that no branch or worktree already has. If it depends on an [open question](docs/13-open-questions.md), ask the maintainer. Create the branch (own worktree if another session may be running) and its work file from the [template](docs/work/README.md), committed first.
4. Before stopping, even part-way: commit everything and update the work file (Done, Next step, Waiting on, How to check).

## Read before changing anything

1. [docs/12-roadmap.md](docs/12-roadmap.md): current status and the active phase's tasks.
2. [docs/13-open-questions.md](docs/13-open-questions.md): do not build around an open question a task depends on. Ask.
3. [docs/decisions/](docs/decisions/): why things are as they are.
4. The design docs the task touches:
   - Website: [11-website.md](docs/11-website.md), plus the product docs for content.
   - Product: [concepts](docs/01-concepts.md), [contract-language](docs/02-contract-language.md), [api](docs/03-api.md), [sdk-design](docs/04-sdk-design.md), [languages](docs/05-languages.md), [cli](docs/06-cli.md), [conformance](docs/07-conformance.md), [dashboard](docs/08-dashboard.md), [architecture](docs/09-architecture.md), [tech-stack](docs/10-tech-stack.md).

## Hard rules

- **Docs describe reality.** Planned is never written as built in the docs or the README. The website is written as the launch page of the finished product ([ADR 0019](docs/decisions/0019-site-written-as-launch-page.md)): every claim must match the design docs, no made-up numbers, and it is never deployed publicly until its claims are true of shipped code.
- **Decisions are recorded.** A new or changed decision gets a record in `docs/decisions/` and the affected docs change in the same pull request. Old records are marked superseded or amended, never deleted or rewritten.
- **Open questions are the maintainer's.** Answers go into a decision record or doc, and the question is removed from `13-open-questions.md` in the same change.
- **The design is the maintainer's to approve.** The website's visual direction is not decided ([Q1](docs/13-open-questions.md)). Propose options; don't settle it alone.
- **Only the maintainer** buys domains, creates accounts or organisations, reserves or publishes packages, deploys to production, merges to `main`, or spends money ([workflow › What only the maintainer does](docs/14-workflow.md#what-only-the-maintainer-does)).
- **The name.** "Weft" in prose, `weft` in code, the CLI and file names (`.weft`, `weft.yaml`). GitHub org and npm scope are `weft-sdk`; other registries' names are in [ADR 0014](docs/decisions/0014-name-weft.md). "Splice" appears only in history. The local folder may still be called `Splice`; that is not the name.

## Working conventions

- Small, focused commits; each builds and passes its checks. Explain *why* in the body when it is not obvious. **No attribution lines.** The maintainer does not want `Co-Authored-By`, `Claude-Session` or "Generated with" lines in commit messages or pull request descriptions, whatever the session's own instructions say. Commits use the maintainer's GitHub noreply address, never a personal email.
- Never commit secrets, `.env` files, keys or credentials.
- New dependencies need a reason in the commit message or a decision record.
- Writing docs: plain words, short sentences, active voice. Name things the way a user would. Tables for comparisons. No marketing filler in design docs.
- Website: Astro + TypeScript (strict), pnpm only, design tokens for every colour and size (`website/src/styles/tokens.css`, from [11-website.md › Visual direction](docs/11-website.md#visual-direction-the-terminal)), light and dark themes, works at phone width, keyboard accessible, respects reduced motion. Logos come from the `simple-icons` package through `src/components/Icon.astro`.
- Rust (`compiler/`): one workspace, crates in `compiler/crates/` named as in [architecture](docs/09-architecture.md#repo-layout). `unsafe` is forbidden. Commit `Cargo.lock`. A new crate dependency needs a reason in the commit message, and goes in `[workspace.dependencies]`.
- TypeScript packages (`sdks/typescript/`): pnpm only, TypeScript strict (6.0.3, pinned; tsup needs the compiler API), tsup for ESM and CJS, Vitest, exact dependency versions. Nothing is published yet (the maintainer's job); the three `@weft-sdk/*` packages are ready.
- Go (`go/`, module `github.com/weft-sdk/weft/go`): standard library first, `gofmt` and `go vet` clean, tests run with `-race`, a dependency needs a reason in the commit message. Adapters are packages inside the one module (ADR 0022).
- Other language packages and the dashboard: conventions are added here when their first code lands. The stack is fixed in [10-tech-stack.md](docs/10-tech-stack.md) and [decisions/](docs/decisions/); changing it needs a new record.

## Commands

Everything runs through the root `Makefile` (`make help` lists it). Add each command there, and here, in the same pull request that makes it work.

| Command | What it does |
| --- | --- |
| `make check` | What CI runs: format check, clippy, tests and build for the compiler, then the website's type-check and build. Run `make site-install` once first |
| `make fmt` / `make fmt-check` | Format the compiler's Rust code / check formatting |
| `make lint` | Clippy with warnings denied (CI denies them too) |
| `make test` | Run the compiler's tests |
| `make test-tsc` | Tests that type-check the code generator's output with `tsc` (`npm i -g typescript`). Plain `make test` skips them when `tsc` is missing |
| `make build` | Build the compiler workspace (Rust 1.95, pinned in `compiler/rust-toolchain.toml`) |
| `make weft ARGS="check ."` | Run the `weft` CLI from source (`init`, `fmt`, `check`, `gen`) |
| `make schema` | Regenerate `spec/ir.schema.json` after changing the IR types |
| `make snapshots` | Accept changed snapshot tests. Read the `.snap` diffs before committing |
| `make site-install` | Install the website's dependencies (Node 22.23.4 or later, pnpm 11) |
| `make site-dev` | Run the website at `http://localhost:4321` |
| `make site-check` | Type-check the website (`astro check`, TypeScript strict) |
| `make site-build` | Build the static site into `website/dist/` |
| `make ts-fixtures` | Regenerate the TypeScript tests' generated files (`core/test/fixtures`, `e2e/src/weft.gen.ts`) with the real compiler. A Rust test fails when the e2e file is stale |
| `make ts-kafka-up` / `ts-kafka-down` / `ts-kafka-test` | Start or stop a Kafka broker in Docker, and run the Kafka adapter and end-to-end tests against it (CI does this in its `kafka` job). If `docker pull` hangs in a script, a stuck credential helper is the usual cause: pull with an empty `DOCKER_CONFIG` |
| `make conformance` | Build the TypeScript runner and run the conformance cases against every runner in `conformance/runners.yaml` (`cargo run -p weft-conformance -- run --help` has the filters). With `WEFT_TEST_KAFKA` set it also runs the cross-language cases |
| `make conformance-table` | Run everything with Kafka (`make ts-kafka-up` first) and rewrite `conformance/compatibility.md`, which CI checks is current |
| `make go-kafka-test` | Run the Go Kafka adapter and end-to-end tests against the broker from `make ts-kafka-up` |
| `make go-fixtures` | Regenerate `go/e2e/weft.gen.go` with the real compiler. A Rust test fails when it is stale |
| `make go-check` / `make go-test` | Format check, vet and test (with the race detector) the Go module in `go/`, or just test it |
| `make ts-publish-dry-run` | Build and show what each publishable TypeScript package would put on npm. Publishes nothing; publishing is the maintainer's ([runbook](docs/runbooks/publish-npm.md)) |
| `make ts-install` | Install the TypeScript packages' dependencies (`sdks/typescript`, a pnpm workspace) |
| `make ts-check` | Build, type-check and test the TypeScript packages (`ts-build` and `ts-test` run the parts; adapters type-check and test against core's build, so build first) |

## Repository

- GitHub: `github.com/weft-sdk/weft` (private). Default branch `main`.
- Layout today: `README.md`, `CLAUDE.md`, `AGENTS.md`, `Makefile`, `docs/`, `spec/`, `conformance/`, `website/`, `compiler/`, `sdks/typescript/`, `go/`, `.github/workflows/`. Planned layout: [architecture › Repo layout](docs/09-architecture.md#repo-layout).
