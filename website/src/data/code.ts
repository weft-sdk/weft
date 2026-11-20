/**
 * Code shown on the landing page, written by hand until the compiler exists (ADR 0016).
 * Examples follow docs/api.md and docs/cli.md. Colour helpers wrap text in token classes.
 */
const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const k = (s: string) => `<span class="tok-k">${esc(s)}</span>`;
const s = (x: string) => `<span class="tok-s">${esc(x)}</span>`;
const a = (x: string) => `<span class="tok-a">${esc(x)}</span>`;
const ok = `<span class="tok-ok">✓</span>`;

export const before = [
  `${k("const")} app = express()`,
  `app.use(express.json())`,
  `${k("const")} io = ${k("new")} Server(createServer(app))`,
  `${k("const")} producer = ${k("new")} Kafka({ brokers }).producer()`,
  `${k("await")} producer.connect()`,
  ``,
  `app.post(${s('"/orders"')}, ${k("async")} (req, res) =&gt; {`,
  `  ${k("const")} order = ${k("await")} orders.create(req.body)  ${k("// any")}`,
  `  ${k("await")} producer.send({ topic: ${s('"order.placed"')},`,
  `    messages: [{ key: order.id, value: JSON.stringify(order) }] })`,
  `  io.to(${s("`order:${order.id}`")}).emit(${s('"order.status"')}, ${s('"placed"')})`,
  `  res.json(order)`,
  `})`,
].join("\n");

export const after = [
  `weft.${a("http")}.post(${s('"/orders"')}, ${k("async")} ({ body }) =&gt; {  ${k("// CreateOrder")}`,
  `  ${k("const")} order = ${k("await")} orders.create(body)`,
  `  ${k("await")} weft.${a("kafka")}.publish(${s('"order.placed"')}, order)`,
  `  weft.${a("ws")}.to(${s("`order:${order.id}`")}).emit(${s('"order.status"')}, ${s('"placed"')})`,
  `  ${k("return")} order`,
  `})`,
  ``,
  `${k("await")} weft.start()`,
  ``,
  `${k("// the same libraries still run underneath")}`,
].join("\n");

export const init = [
  `${k("$")} weft init`,
  `Detected: TypeScript (pnpm)`,
  `${k("? Service name")}               orders`,
  `${k("? Which communication do you use?")}`,
  `  HTTP, GraphQL, Kafka, WebSockets`,
  `${k("? HTTP library?")}              Express`,
  `${k("? GraphQL library?")}           Apollo Server`,
  `${k("? Kafka library?")}             KafkaJS`,
  `${k("? WebSocket library?")}         Socket.io`,
  ``,
  `${ok} Wrote weft.yaml`,
  `${ok} Wrote contracts/orders.weft (example)`,
  `${ok} Wrote src/weft.gen.ts`,
  ``,
  `${k("Next: edit contracts/orders.weft, then run `weft gen`.")}`,
].join("\n");

export const contract = [
  `${k("package")} orders @version(${s('"1.0.0"')})`,
  ``,
  `${k("type")} Money {`,
  `  amount: int            ${k("// minor units")}`,
  `  currency: Currency`,
  `}`,
  ``,
  `${k("type")} Order {`,
  `  id: uuid`,
  `  customerId: string`,
  `  items: OrderItem[]`,
  `  total: Money`,
  `  placedAt: timestamp`,
  `}`,
  ``,
  `${k("event")} ${a("order.placed")}: Order {`,
  `  key: id`,
  `}`,
].join("\n");

export const check = [
  `weft check: 1 breaking change`,
  ``,
  `  orders.order.placed`,
  `  ${a("✗")} field \`total\` removed`,
  `      used by payments (Go), analytics (Python)`,
  ``,
  `  Suggested: keep \`total\` with @deprecated,`,
  `  add the new field alongside.`,
].join("\n");

export const dashboard = [
  `${k("$")} weft dashboard`,
  `Weft dashboard running at http://localhost:4747`,
  `Receiving telemetry on   http://localhost:4318 (OTLP)`,
].join("\n");
