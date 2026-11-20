# ADR 0006: Events are transport-neutral

- Status: Accepted (2026-11-20)

**Decision.** An `event` contract describes the message, not the transport. The call site chooses `weft.kafka`, `weft.mq`, `weft.nats`, and so on. The same applies to `rpc` contracts and `weft.grpc` / `weft.rpc`.

**Consequences.** Moving an event from RabbitMQ to Kafka changes call sites and config, not contracts.
