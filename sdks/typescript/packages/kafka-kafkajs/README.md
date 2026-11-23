# @weft-sdk/kafka-kafkajs

The Kafka adapter of [Weft](https://github.com/weft-sdk/weft) for TypeScript, on [KafkaJS](https://kafka.js.org/). Weft does the CloudEvents envelopes, validation, retries and dead letters; KafkaJS keeps the connection, partitions and rebalancing. `weft.kafka.raw` is your `Kafka` instance.

```sh
npm install @weft-sdk/core @weft-sdk/kafka-kafkajs kafkajs
```

Use it through the file `weft gen` writes (`transports.kafka.library: kafkajs` in `weft.yaml`). Messages carry the same `ce_` headers and key as the Go adapter, so services in the two languages read each other's messages. Needs `@weft-sdk/core` and `kafkajs` ^2.2 as peer dependencies.

**Pre-release.** Apache-2.0.
