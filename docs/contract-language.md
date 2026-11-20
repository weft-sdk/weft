# Contract language

Version: **0.1 (draft)**. Syntax is a proposal and open to change.

Contracts live in `.weft` files. They describe data and operations once, for every language. The compiler checks them and generates code ([cli.md § weft gen](cli.md#weft-gen)).

Design goals:

- **Readable by anyone** on any team, whatever language they write.
- **Small.** A contract for a typical service fits on one screen.
- **Covers every transport** in one file: events, HTTP, GraphQL, RPC, sockets.
- **Maps cleanly** to TypeScript, Go, Python, Kotlin, C#, Rust, Swift and the rest.

## A complete example

```weft
// contracts/orders.weft
package orders

enum Currency { EUR GBP USD }

type Money {
  amount: int64          // minor units (cents)
  currency: Currency
}

type OrderItem {
  sku: string
  qty: int32
}

type Order {
  id: uuid
  customerId: string
  items: list<OrderItem>
  total: Money
  note?: string          // optional
  placedAt: timestamp
}

type CreateOrder {
  customerId: string
  items: list<OrderItem>
}

// ---------- events ----------
event order.placed: Order {
  key: id                // partition / routing key
}

event order.cancelled: { id: uuid, reason: string }

// ---------- HTTP ----------
http {
  POST "/orders"       createOrder(body: CreateOrder) -> Order
  GET  "/orders/{id}"  getOrder(id: uuid) -> Order | NotFound
}

// ---------- RPC (gRPC or weft.rpc) ----------
rpc Orders {
  createOrder(CreateOrder) -> Order
  streamUpdates(id: uuid) -> stream Order
}

// ---------- GraphQL ----------
graphql {
  query        order(id: uuid): Order?
  mutation     placeOrder(input: CreateOrder): Order
  subscription orderUpdated(id: uuid): Order
}

// ---------- WebSockets ----------
socket tracking {
  server courier.moved: { orderId: uuid, lat: float64, lng: float64 }
  client track.start:   { orderId: uuid }
}
```

## Types

| Weft | TypeScript | Go | Python | Kotlin | C# | Rust |
| --- | --- | --- | --- | --- | --- | --- |
| `string` | `string` | `string` | `str` | `String` | `string` | `String` |
| `bool` | `boolean` | `bool` | `bool` | `Boolean` | `bool` | `bool` |
| `int32` | `number` | `int32` | `int` | `Int` | `int` | `i32` |
| `int64` | `bigint` or `number`¹ | `int64` | `int` | `Long` | `long` | `i64` |
| `float64` | `number` | `float64` | `float` | `Double` | `double` | `f64` |
| `decimal` | `string`² | `string`² | `Decimal` | `BigDecimal` | `decimal` | `rust_decimal::Decimal` |
| `uuid` | `string` (branded) | `uuid.UUID` | `UUID` | `UUID` | `Guid` | `uuid::Uuid` |
| `timestamp` | `Date` | `time.Time` | `datetime` | `Instant` | `DateTimeOffset` | `chrono::DateTime<Utc>` |
| `bytes` | `Uint8Array` | `[]byte` | `bytes` | `ByteArray` | `byte[]` | `Vec<u8>` |
| `list<T>` | `T[]` | `[]T` | `list[T]` | `List<T>` | `List<T>` | `Vec<T>` |
| `map<string, T>` | `Record<string, T>` | `map[string]T` | `dict[str, T]` | `Map<String, T>` | `Dictionary<string, T>` | `HashMap<String, T>` |
| `A \| B` | union | interface + variants | `Union` | sealed class | `OneOf` | `enum` |

¹ Configurable per project. ² Where no native decimal type exists, decimals travel as strings to avoid precision loss.

Field modifiers:

- `name?: T`: optional (may be absent).
- `name: T?`: nullable (present, may be null).
- `@deprecated("use totalCents")`: marks a field for removal.
- `@wire("total_cents")`: name on the wire differs from the name in code.

Inline types (`{ id: uuid, reason: string }`) are allowed for small payloads and get a generated name (`OrderCancelled`).

## Operation kinds

| Kind | Used by | Declares |
| --- | --- | --- |
| `event name: Type` | `kafka`, `mq`, `nats`, `redis`, `cloud`, `mqtt`, `pulsar` | A message. Options: `key`, `headers` |
| `http { METHOD "path" name(params) -> Result }` | `http` | Routes with path params, query, body, responses |
| `rpc Service { method(Req) -> Res }` | `grpc`, `rpc` | Methods; `stream` for streaming |
| `graphql { query / mutation / subscription }` | `gql` | Root fields |
| `socket ns { server / client event: Type }` | `ws`, `sse` (server only) | Events and which side sends them |

Errors are declared as types and listed in results: `-> Order | NotFound | Conflict`. Each language gets its idiomatic form (typed result, exception, or `(value, err)` in Go).

## Packages and imports

```weft
package payments
import "orders"                 // another package in the same contract set
import "github.com/larder/contracts/orders@v3"   // from a shared repo
```

A **package** groups contracts owned by one service. Names are referenced as `orders.Order`, `orders.order.placed` from other packages.

## Where contracts live

A service owns the contracts it provides. Other services need to read them. Supported sources ([ADR-012](decisions.md#adr-012-where-shared-contracts-live)):

| Source | Fits |
| --- | --- |
| Local path (`./contracts`) | Monorepos |
| Git repo + tag | Polyrepos with a shared `contracts` repo |
| Weft Cloud or self-hosted dashboard | Polyrepos that want versioned, browsable contracts. `weft publish` pushes, `weft add orders` pulls |

## Evolving contracts safely

`weft check` compares the current contracts against a baseline (the last git tag, the default branch, or the published version) and reports breaking changes before merge.

| Change | Result |
| --- | --- |
| Add an optional field | safe |
| Add a required field to an event or response | safe for receivers |
| Add a required field to a request or HTTP body | **breaking** for callers |
| Remove or rename a field | **breaking** |
| Change a field's type | **breaking** (widening such as `int32 → int64` is reported as risky) |
| Make a field nullable or optional | **breaking** for receivers |
| Add an enum value | risky (receivers may not handle it) |
| Remove an operation | **breaking** |
| Change an event's `key` | **breaking** (ordering and partitioning change) |

The safe pattern for a rename: add the new field, mark the old one `@deprecated`, remove it later. When the dashboard is connected, it knows which services still use a deprecated field and says when removal is safe.

## Importing existing schemas

Teams with existing schemas don't rewrite them. The compiler reads:

- Protobuf (`.proto`) → `rpc` and types
- OpenAPI 3 → `http` and types
- GraphQL SDL → `graphql` and types
- AsyncAPI, Avro, JSON Schema → `event` and types

`weft import openapi.yaml` converts to `.weft`. Alternatively, contracts can reference the original file directly so it stays the source of truth.

## Open questions

- Generics for user types (`Page<T>`)?
- Validation constraints (`string(max: 200)`, `int32(min: 1)`): in contracts, or left to application code?
