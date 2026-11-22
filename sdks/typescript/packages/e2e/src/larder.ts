/**
 * The Larder example: an `orders` service and a `notifications` service that talk through one
 * contract (spec/examples/orders.weft). They use nothing but the generated code, and are what the end to
 * end test runs, over real HTTP and a real Kafka broker.
 */
import { DeclaredError, type Config } from "@weft-sdk/core"
import { createWeft, type Uuid, type orders } from "./weft.gen.js"

/** Serves orders over HTTP and announces each new one on Kafka. */
export function ordersService(config: Config) {
  const weft = createWeft({ config })
  const store = new Map<string, orders.Order>()
  const stock: Record<string, number> = { apple: 100, pear: 0 }

  weft.http.post("/orders", async ({ body }) => {
    for (const item of body.items) {
      const available = stock[item.sku] ?? 0
      if (available < item.qty) throw new DeclaredError("orders", "OutOfStock", { sku: item.sku, available }, `Not enough ${item.sku}`)
    }
    const order: orders.Order = {
      id: crypto.randomUUID() as Uuid,
      customerId: body.customerId,
      items: body.items,
      total: { amount: body.items.reduce((sum, i) => sum + i.qty * 150, 0), currency: "EUR" },
      placedAt: new Date(),
    }
    store.set(order.id, order)
    await weft.kafka.publish("order.placed", order)
    return order
  })

  weft.http.get("/orders/{id}", ({ params }) => store.get(params.id))
  weft.http.get("/orders", ({ query }) => [...store.values()].slice(0, query.page === undefined || query.page === null ? undefined : query.page * 10))

  return { weft, store }
}

/** Sends a confirmation for every order placed. `fail` makes it fail, to see the dead letters. */
export function notificationsService(config: Config, options: { fail?: () => boolean } = {}) {
  const weft = createWeft({ config })
  const sent: orders.Order[] = []
  weft.kafka.on(
    "order.placed",
    (order) => {
      if (options.fail?.() === true) throw new Error("the mail server is down")
      sent.push(order)
    },
    { fromBeginning: true },
  )
  return { weft, sent }
}
