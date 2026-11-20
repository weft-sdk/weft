# ADR 0005: Our own contract language

- Status: Accepted (2026-11-20)

**Decision.** Design a small `.weft` language covering events, HTTP, GraphQL, RPC and sockets in one file. Import OpenAPI, protobuf, GraphQL SDL, AsyncAPI, Avro and JSON Schema.

**Alternative considered.** Microsoft TypeSpec. Mature, but HTTP-centred and harder to extend naturally to messaging.
