/** The 13 transport namespaces and what runs them in TypeScript. Source: docs/05-languages.md. */
export interface Transport {
  /** Namespace after `weft.`. */
  ns: string;
  /** What the transport is, in words. */
  name: string;
  /** Simple Icons slug or glyph name for Icon.astro. */
  icon: string;
  /** Libraries Weft runs underneath, in TypeScript. */
  runsOn: string;
  /** An example call, without the `weft.<ns>` prefix. */
  example: string;
}

export const transports: Transport[] = [
  { ns: "http", name: "HTTP and REST", icon: "http", runsOn: "Express, Fastify, Hono, NestJS", example: '.post("/orders", createOrder)' },
  { ns: "gql", name: "GraphQL", icon: "graphql", runsOn: "Apollo Server, GraphQL Yoga", example: '.query("order", findOrder)' },
  { ns: "grpc", name: "gRPC", icon: "grpc", runsOn: "grpc-js, Connect", example: '.call("orders.CreateOrder", req)' },
  { ns: "rpc", name: "Typed RPC", icon: "trpc", runsOn: "tRPC, Connect protocol", example: '.call("orders.createOrder", input)' },
  { ns: "ws", name: "WebSockets", icon: "ws", runsOn: "Socket.io, ws", example: '.emit("courier.moved", position)' },
  { ns: "sse", name: "Server-sent events", icon: "sse", runsOn: "the HTTP adapter", example: '.send("order.status", status)' },
  { ns: "kafka", name: "Kafka", icon: "apachekafka", runsOn: "KafkaJS, confluent-kafka-javascript", example: '.publish("order.placed", order)' },
  { ns: "mq", name: "RabbitMQ", icon: "rabbitmq", runsOn: "amqplib", example: '.send("emails", message)' },
  { ns: "nats", name: "NATS", icon: "natsdotio", runsOn: "official NATS client", example: '.request("pricing.quote", req)' },
  { ns: "redis", name: "Redis", icon: "redis", runsOn: "ioredis, node-redis", example: '.publish("cart.updated", cart)' },
  { ns: "cloud", name: "Cloud queues", icon: "cloud", runsOn: "AWS SDK, Google Cloud, Azure", example: '.sqs.send("emails", message)' },
  { ns: "mqtt", name: "MQTT", icon: "mqtt", runsOn: "MQTT.js", example: '.publish("courier.position", position)' },
  { ns: "pulsar", name: "Pulsar", icon: "apachepulsar", runsOn: "official Pulsar client", example: '.publish("order.placed", order)' },
];
