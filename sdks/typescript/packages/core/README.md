# @weft-sdk/core

The runtime of [Weft](https://github.com/weft-sdk/weft) for TypeScript: one interface for every way services talk.

Weft reads your contracts (`.weft` files, compiled by the `weft` CLI) and gives you `weft.http`, `weft.kafka` and the rest, running on the real library underneath. This package holds what every transport shares: validation and encoding from the contracts, the lifecycle (`start`, `stop`, graceful shutdown), `weft.yaml` configuration, middleware, messaging with CloudEvents envelopes, retries and dead letters, HTTP calls with deadlines, and OpenTelemetry traces and metrics.

```sh
npm install @weft-sdk/core @weft-sdk/http-express @weft-sdk/kafka-kafkajs
weft init        # writes weft.yaml, a first contract and src/weft.gen.ts
```

You normally use it through the file `weft gen` writes, which wires the adapters and types everything from your contracts. See the [documentation](https://github.com/weft-sdk/weft/tree/main/docs).

**Pre-release.** The API can still change before 1.0. Adapters: [`@weft-sdk/http-express`](https://www.npmjs.com/package/@weft-sdk/http-express), [`@weft-sdk/kafka-kafkajs`](https://www.npmjs.com/package/@weft-sdk/kafka-kafkajs).

Apache-2.0.
