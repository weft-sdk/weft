import { WeftError, parseConfig } from "@weft-sdk/core"
import { describe, expect, expectTypeOf, it } from "vitest"
import { createWeft, type Uuid, type orders } from "../src/weft.gen.js"

// These tests are mostly checked by `tsc` (pnpm typecheck): a line marked @ts-expect-error must NOT
// compile, and every other line must. Nothing here connects to anything.

const config = parseConfig("service: orders\ntransports:\n  http:\n    port: 0\n  kafka:\n    brokers: nowhere:9092\nservices:\n  orders: http://localhost:1", {})
const ID = "123e4567-e89b-42d3-a456-426614174000" as Uuid
const order: orders.Order = {
  id: ID,
  customerId: "c1",
  items: [{ sku: "apple", qty: 1 }],
  total: { amount: 100, currency: "EUR" },
  placedAt: new Date(),
}

describe("the generated weft object", () => {
  const weft = createWeft({ config })

  it("has the lifecycle, the runtime and a typed object per transport", () => {
    expect(typeof weft.start).toBe("function")
    expect(typeof weft.stop).toBe("function")
    expect(typeof weft.use).toBe("function")
    expect(weft.core.config.service).toBe("orders")
    expectTypeOf(weft.http.raw).toHaveProperty("listen")
    expectTypeOf(weft.kafka.raw).toHaveProperty("producer")
  })

  it("types Kafka events by name and payload", () => {
    weft.kafka.on("order.placed", (payload, ctx) => {
      expectTypeOf(payload).toEqualTypeOf<orders.Order>()
      expectTypeOf(payload.total.currency).toEqualTypeOf<orders.Currency>()
      expectTypeOf(ctx.attempt).toBeNumber()
    })
    weft.kafka.on("order.cancelled", (payload) => {
      expectTypeOf(payload).toEqualTypeOf<orders.OrderCancelled>()
    })
    const sent: Promise<void> = weft.kafka.publish("order.placed", order)
    expect(sent).toBeInstanceOf(Promise)
    sent.catch(() => undefined)

    // @ts-expect-error not an event of any contract
    expect(() => weft.kafka.on("order.shipped", () => undefined)).toThrow()
    // @ts-expect-error the payload is not an Order
    void weft.kafka.publish("order.placed", { id: ID }).catch(() => undefined)
    // @ts-expect-error a cancelled event is not an Order
    void weft.kafka.publish("order.cancelled", order).catch(() => undefined)
    // @ts-expect-error a handler of order.placed receives an Order, not an OrderCancelled
    weft.kafka.on("order.placed", (payload: orders.OrderCancelled) => payload)
  })

  it("types the routes this service serves", () => {
    weft.http.post("/orders", ({ body, ctx }) => {
      expectTypeOf(body).toEqualTypeOf<orders.CreateOrder>()
      expectTypeOf(ctx.response.status).toBeNumber()
      return order
    })
    weft.http.get("/v1/orders/{id}", ({ params }) => {
      expectTypeOf(params).toEqualTypeOf<{ id: Uuid }>()
      return undefined // the result is optional, so nothing is allowed: it answers 404
    })
    weft.http.get("/orders", ({ query }) => {
      expectTypeOf(query).toEqualTypeOf<orders.ListOrdersQuery>()
      return [order]
    })

    // @ts-expect-error no such route
    expect(() => weft.http.post("/payments", () => order)).toThrow()
    // @ts-expect-error GET /orders returns a list, not one order
    expect(() => weft.http.get("/orders", () => order)).toThrow()
    // @ts-expect-error a route that cannot be empty must return its value
    expect(() => weft.http.post("/orders", () => undefined)).toThrow()
    // @ts-expect-error POST /orders has no GET
    expect(() => weft.http.get("/v1/orders/{id}/items", () => order)).toThrow()
  })

  it("types calls to other services, asking only for what the route needs", () => {
    const orders = weft.http.to("orders")
    const calls = async () => {
      const created = await orders.post("/orders", { body: { customerId: "c", items: [] } })
      expectTypeOf(created).toEqualTypeOf<orders.Order>()
      const one = await orders.get("/orders/{id}", { params: { id: ID } })
      expectTypeOf(one).toEqualTypeOf<orders.Order | undefined>()
      const list = await orders.get("/orders") // the query is all-optional, so it may be left out
      expectTypeOf(list).toEqualTypeOf<orders.Order[]>()
      await orders.get("/orders", { query: { page: 2 }, timeoutMs: 500 })
      await orders.post("/orders", { body: { customerId: "c", items: [] }, idempotencyKey: "k" })

      // @ts-expect-error the body is required
      await orders.post("/orders")
      // @ts-expect-error the body is not a CreateOrder
      await orders.post("/orders", { body: { customerId: 1 } })
      // @ts-expect-error the path parameter is required
      await orders.get("/orders/{id}", {})
      // @ts-expect-error the path parameter is a Uuid
      await orders.get("/orders/{id}", { params: { id: 5 } })
      // @ts-expect-error this route takes no body
      await orders.get("/orders", { body: {} })
      // @ts-expect-error no such route
      await orders.delete("/orders/{id}", { params: { id: ID } })
    }
    expectTypeOf(calls).toBeFunction()
    // @ts-expect-error there is no such service in the contracts
    weft.http.to("payments")
  })

  it("fails at run time, with a clear error, for what the types cannot see", () => {
    const loose = weft.http as unknown as { get(path: string, h: () => void): void }
    expect(() => loose.get("/nope", () => undefined)).toThrow(WeftError)
  })
})
