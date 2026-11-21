# P1.8: `weft init`

- Branch: `feat/p1-8-weft-init`
- Task: P1.8 in the roadmap, needs P1.6 (done)
- Started: 2026-11-21 · Last session: 2026-11-21
- Pushed: no

## Goal

`weft init` sets up a project: in an empty folder it writes a working `weft.yaml`, a first contract and the generated `weft.gen.ts`, interactively or with flags (`weft init --yes --http express --kafka kafkajs`).

## Plan

1. Work file, committed first.
2. `weft-init`: the catalogue of transports and TypeScript libraries (from languages.md), language and package-manager detection, service-name rules, the `weft.yaml` text and an example contract matched to the chosen transports.
3. `weft init` in `weft-cli`: flags, interactive prompts (`inquire`), writing files, then `weft gen`.
4. Tests: every transport on its own and all together produce a contract that checks, formats and generates; the CLI end to end; refusals.
5. Docs: cli.md, roadmap. Delete this file.

## Decisions made on this branch

- **No package installation yet.** cli.md shows `weft init` running `pnpm add @weft-sdk/...`. Those packages are written in phase P2 and do not exist, so the command would fail. `weft init` detects the package manager and says so, and the install step arrives with P2. Everything else is built.
- **TypeScript is the only language** (the only one `weft gen` writes). `--lang` accepts it, and other languages say they are not available yet.
- **It never overwrites.** If `weft.yaml` or the example contract exists, `weft init` stops before writing anything.
- **It checks before it writes.** The example contract is checked and generated in memory first, so a bad service name (a TypeScript keyword, say) fails with nothing left behind.
- Library names are the ones in languages.md, lower-cased: `express`, `fastify`, `hono`, `nestjs`, `apollo-server`, `graphql-yoga`, `grpc-js`, `connect`, `trpc`, `socket.io`, `ws`, `kafkajs`, `confluent-kafka-javascript`, `amqplib`, `nats`, `ioredis`, `node-redis`, `mqtt.js`, `pulsar-client`, and the cloud providers `aws`, `gcp`, `azure`.
- New dependency: `inquire`, for the interactive prompts (named in tech-stack.md).

## Done

- Nothing yet.

## Next step

Step 2.

## Waiting on

- Nothing.

## How to check

    cargo test --manifest-path compiler/Cargo.toml -p weft-init -p weft-cli

## Found, not done

- Installing the language packages (needs P2).
- Languages other than TypeScript.
- The interactive prompts are not covered by automated tests (they need a terminal); the non-interactive path is.
