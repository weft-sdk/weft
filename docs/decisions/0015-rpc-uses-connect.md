# ADR 0015: `weft.rpc` uses the Connect protocol

- Status: Accepted (2026-11-20)

**Decision.** `weft.rpc` speaks the Connect protocol: HTTP POST with JSON (or protobuf), over HTTP/1.1 or HTTP/2. In TypeScript, tRPC may be used underneath when both ends are TypeScript.

**Consequences.** No protocol to design or maintain. Calls can be made with curl. Weft services interoperate with existing Connect and gRPC services.
