# Contract language

Version: **0.1**. Syntax decided on 2026-11-20. The formal grammar and lexical rules are in [spec/language.md](../spec/language.md); this page is the guide.

Contracts live in `.weft` files. They describe data and operations once, for every language. The compiler checks them and generates code ([cli.md § weft gen](cli.md#weft-gen)).

Design goals:

- **Readable by anyone** on any team, whatever language they write.
- **Small.** A contract for a typical service fits on one screen.
- **Covers every transport** in one file: events, HTTP, GraphQL, RPC, sockets.
- **Maps cleanly** to TypeScript, Go, Python, Kotlin, C#, Rust, Swift and the rest.

## A complete example

```weft
// contracts/orders.weft
package orders @version("1.0.0")

enum Currency { EUR GBP USD }

type Money {
  amount: int  // minor units (cents)
  currency: Currency
}

type OrderItem {
  sku: string
  qty: int
}

/// An order placed by a customer.
type Order {
  id: uuid
  customerId: string
  items: OrderItem[]
  total: Money
  note?: string  // optional
  placedAt: timestamp
}

type CreateOrder {
  customerId: string
  items: OrderItem[]
}

// ---------- events ----------
event order.placed: Order {
  key: id  // partition / routing key
}

event order.cancelled: { id: uuid, reason: string }

// ---------- errors ----------
error OutOfStock {
  sku: string
  available: int
}

// ---------- HTTP ----------
http /v1 {
  POST "/orders"       createOrder(body: CreateOrder) -> Order | OutOfStock | InvalidInput
  GET  "/orders/{id}"  getOrder(id: uuid) -> Order?
  GET  "/orders"       listOrders(query: { page?: int }) -> Order[]
}

// ---------- RPC (gRPC or weft.rpc) ----------
rpc Orders {
  createOrder(CreateOrder) -> Order
  streamUpdates(id: uuid) -> stream Order
}

// ---------- GraphQL ----------
graphql {
  query        order(id: uuid) -> Order?
  mutation     placeOrder(input: CreateOrder) -> Order
  subscription orderUpdated(id: uuid) -> Order
}

// ---------- WebSockets ----------
socket tracking {
  server courier.moved: { orderId: uuid, lat: float, lng: float }
  client track.start:   { orderId: uuid }
}
```

## Basics (decided 2026-11-20)

- **Fields** are written `name: type`.
- **Optional** is one concept: `name?: type` means the value may be missing or null. Receivers treat both the same. There is no separate "nullable".
- **Numbers:** `int` and `float` are 64-bit and cover everyday use. Sized types (`int32`, `int64`, `float32`, `float64`) and `decimal` exist for when size or precision matters.
- **Lists** are `T[]`. **Maps** are `map<K, V>` with string-like keys.
- **Comments:** `//` for notes, `///` for documentation that is copied into generated code (JSDoc, GoDoc, docstrings).
- **No separators.** One field per line, no commas or semicolons. Commas are used only inside inline types on one line.

## Types

| Weft | TypeScript | Go | Python | Kotlin | C# | Rust |
| --- | --- | --- | --- | --- | --- | --- |
| `string` | `string` | `string` | `str` | `String` | `string` | `String` |
| `bool` | `boolean` | `bool` | `bool` | `Boolean` | `bool` | `bool` |
| `int` (64-bit) | `number` or `bigint`¹ | `int64` | `int` | `Long` | `long` | `i64` |
| `int32` | `number` | `int32` | `int` | `Int` | `int` | `i32` |
| `int64` | same as `int` | `int64` | `int` | `Long` | `long` | `i64` |
| `float` (64-bit) | `number` | `float64` | `float` | `Double` | `double` | `f64` |
| `float32` | `number` | `float32` | `float` | `Float` | `float` | `f32` |
| `float64` | same as `float` | `float64` | `float` | `Double` | `double` | `f64` |
| `decimal` | `string`² | `string`² | `Decimal` | `BigDecimal` | `decimal` | `rust_decimal::Decimal` |
| `uuid` | `string` (branded) | `uuid.UUID` | `UUID` | `UUID` | `Guid` | `uuid::Uuid` |
| `timestamp` | `Date` | `time.Time` | `datetime` | `Instant` | `DateTimeOffset` | `chrono::DateTime<Utc>` |
| `bytes` | `Uint8Array` | `[]byte` | `bytes` | `ByteArray` | `byte[]` | `Vec<u8>` |
| `T[]` | `T[]` | `[]T` | `list[T]` | `List<T>` | `List<T>` | `Vec<T>` |
| `map<string, T>` | `Record<string, T>` | `map[string]T` | `dict[str, T]` | `Map<String, T>` | `Dictionary<string, T>` | `HashMap<String, T>` |
| `name?: T` | `name?: T` | `*T` | `T \| None = None` | `T? = null` | `T?` | `Option<T>` |
| `A \| B` | union | interface + variants | `Union` | sealed class | `OneOf` | `enum` |

¹ `number` by default (safe up to 2^53). Projects that need the full 64-bit range set `codegen.int64: bigint` in `weft.yaml`. ² Where no native decimal type exists, decimals travel as strings to avoid precision loss.

Field annotations:

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

### Operation rules (decided 2026-11-20)

- **Results always use `->`**, for every operation kind. `-> Order?` means the result may be empty (HTTP 404, GraphQL null, gRPC NOT_FOUND).
- **Event names are topic names by default.** `event order.placed` travels on the topic, queue or subject `order.placed`. Teams with naming rules remap it in `weft.yaml` under `topics:` without changing the contract.
- **HTTP routes are one line each:** method, path, a name, inputs, result. The name becomes the function name in generated clients (`orders.createOrder(...)`). Path parameters (`{id}`) must appear in the inputs. An optional base path follows `http` (`http /v1 { ... }`). Long routes may wrap onto the next line after `->`.

### Errors

Two kinds, both listed in results with `|`:

- **Built-in errors** always exist: `NotFound`, `InvalidInput`, `Unauthorized`, `Forbidden`, `Conflict`, `Unavailable`, `Timeout`, `Internal`. Their mapping to HTTP status, gRPC codes and messaging behaviour is fixed ([api.md § Errors](api.md#errors)).
- **Custom errors** are declared with `error` and carry structured fields:

```weft
error PaymentDeclined {
  reason: string
  retryable: bool
}

http {
  POST "/charges" charge(body: Charge) -> Receipt | PaymentDeclined | NotFound
}
```

A custom error maps to HTTP 422 by default (override with `@status(402)`) and to gRPC `FAILED_PRECONDITION`. Each language gets its idiomatic form: a typed result union in TypeScript, `(value, error)` with typed errors in Go, exceptions in Python, Java, C# and Ruby, `Result<T, E>` in Rust, `{:error, e}` tuples in Elixir.

## Generics

Types can take type parameters. No constraints, no generic operations, only generic types.

```weft
type Page<T> {
  items: T[]
  nextCursor?: string
}

type Result<T, E> {
  value?: T
  error?: E
}

http {
  GET "/orders" listOrders(query: { cursor?: string }) -> Page<Order>
}
```

Every target language has generics (Go since 1.18). Languages without them at runtime (Python, Ruby, PHP, Elixir) get typed hints where available and runtime validation of the concrete type.

## Validation

A small, fixed set of rules, written as annotations after the type. Generated code checks them in every language, so every service rejects the same bad data with the same error.

| Rule | Applies to | Example |
| --- | --- | --- |
| `@min(n)`, `@max(n)` | numbers | `qty: int @min(1) @max(100)` |
| `@length(min, max)` | strings, lists, maps | `name: string @length(1, 200)` |
| `@pattern("regex")` | strings | `postcode: string @pattern("^[A-Z0-9 ]{5,8}$")` |
| `@format(name)` | strings | `email`, `url`, `uri`, `hostname`, `ipv4`, `ipv6`, `date`, `time` |

Regex patterns use the common subset supported by every language's regex engine (no lookbehind, no backreferences). The compiler rejects patterns outside that subset.

Business rules ("a customer can only have 5 open orders") stay in application code.

## Packages and imports

```weft
package payments
import orders

event payment.captured: {
  order: orders.Order
  amount: orders.Money
}
```

- A **package** groups the contracts owned by one service. Every `.weft` file starts with `package <name>`.
- **Imports name a package only.** Where it comes from (a local folder, a git repo, the dashboard) and which version are set in `weft.yaml`, never in the contract. Moving a repo or bumping a version doesn't touch any `.weft` file.
- Names from another package are written `orders.Order`, `orders.order.placed`.

## Versioning

Each package has a semantic version:

```weft
package orders @version("2.3.0")
```

- **Patch** (2.3.0 → 2.3.1): docs and comments only.
- **Minor** (2.3.0 → 2.4.0): safe changes, such as a new optional field or a new operation.
- **Major** (2.3.0 → 3.0.0): breaking changes.

`weft check` enforces this. A breaking change without a major bump fails, and the message suggests the safe route (deprecate first, remove in the next major). Consumers pin a range in `weft.yaml` (`orders: ^2.3`), the same way they pin libraries.

## Where contracts live

A service owns the contracts it provides. Other services need to read them. Supported sources ([ADR 0012](decisions/0012-where-contracts-live.md)):

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
| Change a field's type | **breaking** (widening such as `int32 → int` is reported as risky) |
| Make a required field optional | **breaking** for receivers |
| Add an enum value | risky (receivers may not handle it) |
| Remove an operation | **breaking** |
| Change an event's `key` | **breaking** (ordering and partitioning change) |
| Tighten validation (lower `@max`, add `@pattern`) | **breaking** for senders |
| Loosen validation | safe for senders, risky for receivers that relied on it |

The safe pattern for a rename: add the new field, mark the old one `@deprecated`, remove it later. When the dashboard is connected, it knows which services still use a deprecated field and says when removal is safe.

## Importing existing schemas

Teams with existing schemas don't rewrite them. The compiler reads:

- Protobuf (`.proto`) → `rpc` and types
- OpenAPI 3 → `http` and types
- GraphQL SDL → `graphql` and types
- AsyncAPI, Avro, JSON Schema → `event` and types

`weft import openapi.yaml` converts to `.weft`. Alternatively, contracts can reference the original file directly so it stays the source of truth.

## Open questions

None blocking. The grammar and reserved words are settled in [spec/language.md](../spec/language.md). Formatter style is settled with the formatter (task P1.5).
