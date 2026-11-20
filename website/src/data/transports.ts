/** The 13 transport namespaces and what runs them in TypeScript. Source: docs/languages.md. */
export interface Transport {
  /** Namespace after `weft.`. */
  ns: string;
  /** Simple Icons slug or glyph name for Icon.astro. */
  icon: string;
  /** Libraries Weft runs underneath, in TypeScript. */
  runsOn: string;
  /** An example call, without the `weft.<ns>` prefix. */
  example: string;
}

export const transports: Transport[] = [
  { ns: "http", icon: "http", runsOn: "Express, Fastify, Hono, NestJS", example: '.post("/orders", createOrder)' },
  { ns: "gql", icon: "graphql", runsOn: "Apollo Server, GraphQL Yoga", example: '.query("order", findOrder)' },
  { ns: "grpc", icon: "grpc", runsOn: "grpc-js, Connect", example: '.call("orders.CreateOrder", req)' },
  { ns: "rpc", icon: "trpc", runsOn: "tRPC, Connect protocol", example: '.call("orders.createOrder", input)' },
  { ns: "ws", icon: "ws", runsOn: "Socket.io, ws", example: '.emit("courier.moved", position)' },
  { ns: "sse", icon: "sse", runsOn: "the HTTP adapter", example: '.send("order.status", status)' },
  { ns: "kafka", icon: "apachekafka", runsOn: "KafkaJS, confluent-kafka-javascript", example: '.publish("order.placed", order)' },
  { ns: "mq", icon: "rabbitmq", runsOn: "amqplib", example: '.send("emails", message)' },
  { ns: "nats", icon: "natsdotio", runsOn: "official NATS client", example: '.request("pricing.quote", req)' },
  { ns: "redis", icon: "redis", runsOn: "ioredis, node-redis", example: '.publish("cart.updated", cart)' },
  { ns: "cloud", icon: "cloud", runsOn: "AWS, Google Cloud and Azure SDKs", example: '.sqs.send("emails", message)' },
  { ns: "mqtt", icon: "mqtt", runsOn: "MQTT.js", example: '.publish("courier.position", position)' },
  { ns: "pulsar", icon: "apachepulsar", runsOn: "official Pulsar client", example: '.publish("order.placed", order)' },
];
