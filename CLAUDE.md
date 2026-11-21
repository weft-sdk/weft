# Weft: guidance for coding agents

Weft is **one interface for every way services talk**. Developers call `weft.http`, `weft.gql`, `weft.grpc`, `weft.rpc`, `weft.ws`, `weft.sse`, `weft.kafka`, `weft.mq`, `weft.nats`, `weft.redis`, `weft.cloud`, `weft.mqtt` and `weft.pulsar`, and Weft runs the real library underneath (Express, Apollo, KafkaJS, Socket.io, Spring Kafka, FastAPI, ...). Contracts are written once in language-neutral `.weft` files, and a Rust compiler generates typed code for 13 languages. Overview: [README](README.md). Design: [docs/](docs/README.md).

**Status: design stage.** Only the website exists (`website/`) and an empty Rust workspace (`compiler/`, no features yet); the product is not built. Never describe a planned feature as existing in the docs or the README. The website is the exception: it is written as the launch page ([ADR 0019](docs/decisions/0019-site-written-as-launch-page.md)) and is not deployed publicly until its claims are true.

**Weft is not a code scanner.** An outdated README once described the project (then called Splice) as one. That idea was dropped completely ([ADR 0001](docs/decisions/0001-library-not-scanner.md), [history](docs/history.md)). Do not reintroduce it. Do not reopen decisions in [history](docs/history.md) or [decisions/](docs/decisions/) unless the maintainer asks.

## The `notes/` folder is not for you

If a `notes/` folder exists, it holds the maintainer's personal notes. Do not read, follow or edit it, and do not treat it as project state, unless the maintainer explicitly asks. Project state lives in `docs/`.

## Starting a session

Work is handed off through the repository, never through a conversation. Full procedure: [docs/workflow.md](docs/workflow.md). In short:

1. `git status`, `git branch --list`, `git worktree list`, `git fetch`.
2. **On a task branch:** read `docs/work/<branch with / replaced by ->.md` and continue from its **Next step**. Do not re-plan what it already decided.
3. **On `main`:** read [Roadmap › Current status](docs/roadmap.md#current-status), take the first task whose dependencies are done and that no branch or worktree already has. If it depends on an [open question](docs/open-questions.md), ask the maintainer. Create the branch (own worktree if another session may be running) and its work file from the [template](docs/work/README.md), committed first.
4. Before stopping, even part-way: commit everything and update the work file (Done, Next step, Waiting on, How to check).

## Read before changing anything

1. [docs/roadmap.md](docs/roadmap.md): current status and the active phase's tasks.
2. [docs/open-questions.md](docs/open-questions.md): do not build around an open question a task depends on. Ask.
3. [docs/decisions/](docs/decisions/): why things are as they are.
4. The design docs the task touches:
   - Website: [website.md](docs/website.md), plus the product docs for content.
   - Product: [concepts](docs/concepts.md), [contract-language](docs/contract-language.md), [api](docs/api.md), [sdk-design](docs/sdk-design.md), [languages](docs/languages.md), [cli](docs/cli.md), [conformance](docs/conformance.md), [dashboard](docs/dashboard.md), [architecture](docs/architecture.md), [tech-stack](docs/tech-stack.md).

## Hard rules

- **Docs describe reality.** Planned is never written as built in the docs or the README. The website is written as the launch page of the finished product ([ADR 0019](docs/decisions/0019-site-written-as-launch-page.md)): every claim must match the design docs, no made-up numbers, and it is never deployed publicly until its claims are true of shipped code.
- **Decisions are recorded.** A new or changed decision gets a record in `docs/decisions/` and the affected docs change in the same pull request. Old records are marked superseded or amended, never deleted or rewritten.
- **Open questions are the maintainer's.** Answers go into a decision record or doc, and the question is removed from `open-questions.md` in the same change.
- **The design is the maintainer's to approve.** The website's visual direction is not decided ([Q1](docs/open-questions.md)). Propose options; don't settle it alone.
- **Only the maintainer** buys domains, creates accounts or organisations, reserves or publishes packages, deploys to production, merges to `main`, or spends money ([workflow › What only the maintainer does](docs/workflow.md#what-only-the-maintainer-does)).
- **The name.** "Weft" in prose, `weft` in code, the CLI and file names (`.weft`, `weft.yaml`). GitHub org and npm scope are `weft-sdk`; other registries' names are in [ADR 0014](docs/decisions/0014-name-weft.md). "Splice" appears only in history. The local folder may still be called `Splice`; that is not the name.

## Working conventions

- Small, focused commits; each builds and passes its checks. Explain *why* in the body when it is not obvious. End commit messages with the attribution line the session's instructions give.
- Never commit secrets, `.env` files, keys or credentials.
- New dependencies need a reason in the commit message or a decision record.
- Writing docs: plain words, short sentences, active voice. Name things the way a user would. Tables for comparisons. No marketing filler in design docs.
- Website: Astro + TypeScript (strict), pnpm only, design tokens for every colour and size (`website/src/styles/tokens.css`, from [website.md › Visual direction](docs/website.md#visual-direction-the-terminal)), light and dark themes, works at phone width, keyboard accessible, respects reduced motion. Logos come from the `simple-icons` package through `src/components/Icon.astro`.
- Rust (`compiler/`): one workspace, crates in `compiler/crates/` named as in [architecture](docs/architecture.md#repo-layout). `unsafe` is forbidden. Commit `Cargo.lock`. A new crate dependency needs a reason in the commit message, and goes in `[workspace.dependencies]`.
- Language packages and the dashboard: conventions are added here when their first code lands. The stack is fixed in [tech-stack.md](docs/tech-stack.md) and [decisions/](docs/decisions/); changing it needs a new record.

## Commands

Add each command here in the same pull request that makes it work.

| Command | What it does |
| --- | --- |
| `pnpm --dir website install` | Install the website's dependencies (Node 22.23.4 or later, pnpm 11) |
| `pnpm --dir website dev` | Run the website locally at `http://localhost:4321` |
| `pnpm --dir website check` | Type-check the website (`astro check`, TypeScript strict) |
| `pnpm --dir website build` | Build the static site into `website/dist/` |
| `cargo build --manifest-path compiler/Cargo.toml` | Build the compiler workspace (Rust 1.95, pinned in `compiler/rust-toolchain.toml`) |
| `cargo test --manifest-path compiler/Cargo.toml` | Run the compiler's tests |
| `cargo clippy --manifest-path compiler/Cargo.toml --all-targets -- -D warnings` | Lint the compiler; CI denies warnings |
| `cargo fmt --manifest-path compiler/Cargo.toml --all --check` | Check formatting |
| `cargo run --manifest-path compiler/Cargo.toml -p weft-cli -- <args>` | Run the `weft` CLI from source |

## Repository

- GitHub: `github.com/weft-sdk/weft` (private). Default branch `main`.
- Layout today: `README.md`, `CLAUDE.md`, `AGENTS.md`, `docs/`, `website/`, `compiler/`, `.github/workflows/`. Planned layout: [architecture › Repo layout](docs/architecture.md#repo-layout).
