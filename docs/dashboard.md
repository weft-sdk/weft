# Dashboard

The dashboard shows how services talk to each other: a live map built from contracts and from telemetry that Weft packages already produce. It's the visible side of Weft, and the paid product in its cloud form.

## Three ways to run it

All three use the same UI and the same server code.

| Mode | How | For |
| --- | --- | --- |
| **Local** | `weft dashboard` → `http://localhost:4747` | One developer, on a laptop, with services running locally or in Docker Compose |
| **Self-hosted** | Docker image or Helm chart, inside the company's cluster | A team or company that wants a shared view without sending data out |
| **Cloud** | Sign in on the Weft website | Companies that don't want to run it, plus team features |

This mirrors tools people already know: RabbitMQ's management UI on port 15672, Kafka UI, the Temporal web UI.

## What it shows

**Service map**
- Every service as a node, every operation between them as an edge, coloured by transport.
- Built from contracts (who provides and who uses each operation), so it works before any traffic flows.
- Click a service: what it provides, what it uses, which language and libraries it runs.
- Click an operation: its contract, version history, every service that uses it.

**Live traffic**
- Messages per second, latency (p50/p95/p99) and error rate per operation, animated on the map.
- Consumer lag for Kafka, queue depth for RabbitMQ, connected clients for WebSockets.
- Recent messages for an operation (opt-in, with field redaction).
- Dead-letter queues: what failed, why, with replay.

**Contracts**
- Browse every published contract, with generated code in any language.
- Diff between versions.
- Deprecated fields and which services still use them.
- Results of `weft check` from CI.

**Traces**
- Follow one request across transports: HTTP → Kafka → gRPC → WebSocket, across languages.

## Where the data comes from

| Data | Source |
| --- | --- |
| Map structure | Contracts, from `weft publish`, a git repo, or a local folder |
| Which services use what | Contracts plus each service's generated operation metadata, reported on startup |
| Traffic, latency, errors | OpenTelemetry metrics and traces from Weft packages (OTLP) |
| Lag, queue depth | Optional read-only connections to brokers |
| Message samples | Opt-in sampling in the Weft packages, with redaction rules |

Weft packages already emit OpenTelemetry. Pointing them at the dashboard is one line in `weft.yaml` (`telemetry.dashboard`). Teams with an existing OpenTelemetry collector can fan out to both.

## Architecture

```
Weft packages ──OTLP──► dashboard server ──► storage
                                │
contracts ──────────────────────┤
broker connections (optional) ──┘
                                │
                                ▼
                         dashboard UI (browser)
```

| Part | Local | Self-hosted / Cloud |
| --- | --- | --- |
| Server | Rust (axum), inside the `weft` binary | Same server, as its own binary / container |
| UI | React SPA embedded in the binary | Same build |
| Storage | SQLite + in-memory buffers for live data | Postgres for metadata, ClickHouse for telemetry |
| Auth | None by default; listens on localhost only. Optional token | SSO (OIDC, SAML), roles, audit log |

## Cloud-only features

- Organisations, teams, roles, SSO.
- History beyond what fits on a laptop.
- Alerts: error rate, lag, a breaking change merged, a deprecated field ready for removal.
- Slack, email and webhook notifications.
- PR comments from `weft check` naming affected teams.

## Not doing

- Replacing an observability platform. The dashboard is about service communication. It links out to Grafana, Datadog or Honeycomb for everything else.
- Sitting in the request path. The dashboard only receives telemetry. If it's down, services keep working.
