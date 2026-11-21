# The `weft.*` API

This is what developers write. Every language exposes the same shape, adapted to its own conventions (camelCase in TypeScript, PascalCase in Go and C#, snake_case in Python and Ruby).

All examples use Larder's contracts from [02-contract-language.md](02-contract-language.md).

## Lifecycle

```ts
import { weft } from "./weft.gen"

// register handlers first...
weft.kafka.on("order.placed", handleOrder)
weft.http.post("/orders", createOrder)

// ...then connect everything and start listening
await weft.start()

// graceful shutdown: stop taking new work, finish in-flight work, close connections
process.on("SIGTERM", () => weft.stop())
```

Connection settings (brokers, ports, URLs) come from `weft.yaml` and environment variables. See [06-cli.md § Config](06-cli.md#config-file-weftyaml).

## HTTP — `weft.http`

Serve:

```ts
weft.http.post("/orders", async ({ body }) => {
  const order = await orders.create(body)   // body: CreateOrder
  return order                              // must be Order
})

weft.http.get("/orders/{id}", async ({ params }) => {
  return (await orders.find(params.id)) ?? weft.http.notFound()
})
```

Call another service:

```ts
const order = await weft.http.to("orders").post("/orders", { body: newOrder })
```

`to("orders")` resolves the service's base URL from config.

## GraphQL — `weft.gql`

```ts
// serve
weft.gql.query("order", ({ id }) => orders.find(id))
weft.gql.mutation("placeOrder", ({ input }) => orders.create(input))
weft.gql.subscription("orderUpdated", ({ id }) => orders.updates(id))

// call
const order = await weft.gql.to("api").query("order", { id }, { select: ["id", "total"] })
```

## gRPC — `weft.grpc`

```ts
// serve
weft.grpc.serve("orders.Orders", {
  createOrder: async (req) => orders.create(req),
  streamUpdates: async function* ({ id }) { yield* orders.updates(id) },
})

// call
const order = await weft.grpc.call("orders.CreateOrder", req)
for await (const update of weft.grpc.stream("orders.StreamUpdates", { id })) { ... }
```

## Typed RPC — `weft.rpc`

The tRPC-style experience in every language, defined by the same `rpc` contracts. In TypeScript it can run on tRPC itself; elsewhere it runs over HTTP with Weft's own protocol.

```ts
weft.rpc.handle("orders.createOrder", (input) => orders.create(input))
const order = await weft.rpc.call("orders.createOrder", input)
```

## Messaging — `weft.kafka`, `weft.mq`, `weft.nats`, `weft.redis`, `weft.cloud`, `weft.mqtt`, `weft.pulsar`

All messaging transports share one shape:

```ts
await weft.kafka.publish("order.placed", order)

weft.kafka.on("order.placed", async (order, ctx) => {
  // order: Order
  // ctx: headers, key, timestamp, attempt number, trace context
})
```

Transport-specific options are passed as a last argument, and only accept what that transport supports:

```ts
weft.kafka.on("order.placed", handle, { group: "payments", fromBeginning: false })
weft.mq.send("emails", message)                   // point-to-point queue
weft.mq.publish("order.placed", order)            // to an exchange
weft.mq.on("emails", handle, { prefetch: 10 })
weft.cloud.sqs.send("emails", message)
weft.nats.request("pricing.quote", req)           // NATS request–reply
```

Messages are wrapped in a **CloudEvents** envelope (id, type, source, time) so every consumer, in every language, sees the same metadata.

## WebSockets — `weft.ws`

```ts
// server
weft.ws.on("track.start", async ({ orderId }, socket) => {
  socket.join(`order:${orderId}`)
})
weft.ws.to(`order:${id}`).emit("courier.moved", position)

// client (browser, mobile)
const ws = weft.ws.connect("tracking")
ws.emit("track.start", { orderId })
ws.on("courier.moved", (pos) => moveMarker(pos))
```

## Server-sent events — `weft.sse`

```ts
weft.sse.stream("/orders/{id}/updates", async function* ({ params }) {
  yield* orders.updates(params.id)
})
```

## Middleware

One middleware model across every transport:

```ts
weft.use(async (ctx, next) => {
  const start = Date.now()
  await next()
  log(ctx.transport, ctx.operation, Date.now() - start)
})

weft.kafka.use(dedupe())        // only for one transport
```

Built-in middleware: tracing (OpenTelemetry), retries with backoff, timeouts, idempotency keys, payload validation, dead-letter routing for messaging.

## Errors

Every language gets the same error categories, mapped to its idioms:

| Category | HTTP | gRPC | Messaging |
| --- | --- | --- | --- |
| `NotFound` | 404 | NOT_FOUND | not retried |
| `InvalidInput` | 400 | INVALID_ARGUMENT | sent to dead-letter |
| `Unauthorized` | 401 | UNAUTHENTICATED | not retried |
| `Forbidden` | 403 | PERMISSION_DENIED | not retried |
| `Conflict` | 409 | ALREADY_EXISTS | not retried |
| `Unavailable` | 503 | UNAVAILABLE | retried |
| `Timeout` | 504 | DEADLINE_EXCEEDED | retried |
| `Internal` | 500 | INTERNAL | retried, then dead-letter |

HTTP error bodies use RFC 9457 Problem Details.

## The same code in other languages

```go
// Go — payments service
weft.Kafka.On("order.placed", func(ctx context.Context, o orders.Order, m weft.Meta) error {
    return charge(ctx, o.ID, o.Total)
})
order, err := weft.GRPC.Call(ctx, orders.CreateOrder, req)
```

```python
# Python — notifications service
@weft.kafka.on("order.placed")
async def send_confirmation(order: Order, ctx: Context) -> None:
    await email.send(order.customer_id, "Order received")

await weft.mq.send("emails", message)
```

```kotlin
// Kotlin — inventory service
weft.kafka.on("order.placed") { order: Order, ctx ->
    stock.reserve(order.items)
}
```

```csharp
// C#
weft.Kafka.On<Order>("order.placed", async (order, ctx) => await Reserve(order));
```

```ruby
# Ruby
weft.kafka.on("order.placed") { |order, ctx| Reserve.call(order) }
```

## Escape hatch

```ts
weft.kafka.raw     // KafkaJS Kafka instance
weft.http.raw      // Express app
weft.ws.raw        // Socket.io Server
```

Use it for features Weft doesn't cover. Weft's tracing and middleware don't apply to calls made through `raw`.
