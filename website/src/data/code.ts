/**
 * Code shown on the landing page, written by hand until the compiler exists (ADR 0016).
 * Examples follow docs/03-api.md and docs/06-cli.md. Colour helpers wrap text in token classes.
 */
const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const k = (s: string) => `<span class="tok-k">${esc(s)}</span>`;
const s = (x: string) => `<span class="tok-s">${esc(x)}</span>`;
const a = (x: string) => `<span class="tok-a">${esc(x)}</span>`;

/** The same endpoint written with Express, KafkaJS and Socket.io. One entry per line. */
export const beforeLines = [
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
];

/** The same endpoint written with Weft. */
export const afterLines = [
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
];
