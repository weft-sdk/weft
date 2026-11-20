# ADR 0001: Weft is a communication library, not a scanner

- Status: Accepted (2026-11-20)

**Decision.** Weft is a unified interface (`weft.http`, `weft.kafka`, `weft.ws`, ...) that developers call directly, running existing libraries underneath. There is no code scanner. The service map is built from contracts and telemetry instead.
