/**
 * The docs structure: sections along the top, folders inside each section, pages inside folders.
 * It drives the section bar, the directory tree, search, previous/next links and the page routes.
 * Page content is added to the pages as it is written; until then a page shows its title and summary.
 * The pages follow "Docs structure" in docs/11-website.md.
 */
export interface DocPage {
  slug: string;
  title: string;
  summary: string;
}

export interface DocGroup {
  folder: string;
  pages: DocPage[];
}

export interface DocSection {
  id: string;
  label: string;
  summary: string;
  groups: DocGroup[];
}

const page = (slug: string, title: string, summary: string): DocPage => ({ slug, title, summary });

export const sections: DocSection[] = [
  {
    id: "guides",
    label: "Guides",
    summary: "Install Weft, write a first contract and move existing code over.",
    groups: [
      {
        folder: "getting-started",
        pages: [
          page("introduction", "Introduction", "What Weft is, how a call travels from your code to the real library, and where contracts fit."),
          page("install", "Install", "Run weft init in a service, pick transports and libraries, and see what it writes."),
          page("first-contract", "Your first contract", "Write a contract, run weft gen and call a transport with types."),
        ],
      },
      {
        folder: "migrating",
        pages: [
          page("from-kafkajs", "From KafkaJS", "Move an existing KafkaJS producer and consumer onto weft.kafka without changing the library underneath."),
          page("from-express", "From Express", "Move routes onto weft.http while Express keeps serving them."),
        ],
      },
    ],
  },
  {
    id: "contracts",
    label: "Contracts",
    summary: "The language-neutral .weft files every service reads.",
    groups: [
      {
        folder: "language",
        pages: [
          page("types", "Types", "Records, enums, unions and the validation attached to their fields."),
          page("events", "Events", "An event names something that happened, with a typed payload and a key. Every service that reads the contract gets the same type."),
          page("calls", "Calls", "Operations with a request and a response, across HTTP, gRPC and typed RPC."),
          page("validation", "Validation", "How field rules become runtime validators in every language."),
        ],
      },
      {
        folder: "checking",
        pages: [
          page("weft-check", "weft check", "Compare a contract with its last version and name every service that would break."),
          page("breaking-changes", "Breaking changes", "What counts as safe, risky and breaking, with an example of each."),
        ],
      },
      {
        folder: "importing",
        pages: [
          page("openapi", "OpenAPI", "Convert an OpenAPI document into a contract."),
          page("proto", "Protocol Buffers", "Convert .proto files into a contract."),
          page("graphql", "GraphQL SDL", "Convert a GraphQL schema into a contract."),
          page("asyncapi", "AsyncAPI", "Convert an AsyncAPI document into a contract."),
          page("avro", "Avro", "Convert Avro schemas into a contract."),
          page("json-schema", "JSON Schema", "Convert JSON Schema into a contract."),
        ],
      },
    ],
  },
  {
    id: "transports",
    label: "Transports",
    summary: "One page for each of the thirteen ways services talk.",
    groups: [
      {
        folder: "request-response",
        pages: [
          page("http", "HTTP and REST", "weft.http: routes and calls over Express, Fastify, Hono or NestJS."),
          page("graphql", "GraphQL", "weft.gql: queries, mutations and subscriptions over Apollo Server or GraphQL Yoga."),
          page("grpc", "gRPC", "weft.grpc: unary and streaming calls over grpc-js or Connect."),
          page("rpc", "Typed RPC", "weft.rpc: typed procedure calls over tRPC or the Connect protocol."),
        ],
      },
      {
        folder: "realtime",
        pages: [
          page("websockets", "WebSockets", "weft.ws: rooms and events over Socket.io or ws."),
          page("sse", "Server-sent events", "weft.sse: one-way streams to browsers over the HTTP adapter."),
        ],
      },
      {
        folder: "messaging",
        pages: [
          page("kafka", "Kafka", "weft.kafka: publish and consume over KafkaJS or confluent-kafka-javascript."),
          page("rabbitmq", "RabbitMQ", "weft.mq: queues and exchanges over amqplib."),
          page("nats", "NATS", "weft.nats: subjects and request-reply over the official client."),
          page("redis", "Redis", "weft.redis: pub/sub and streams over ioredis or node-redis."),
          page("cloud-queues", "Cloud queues", "weft.cloud: SQS, Pub/Sub and Service Bus behind one shape."),
          page("mqtt", "MQTT", "weft.mqtt: topics over MQTT.js."),
          page("pulsar", "Pulsar", "weft.pulsar: topics and subscriptions over the official client."),
        ],
      },
    ],
  },
  {
    id: "libraries",
    label: "Libraries",
    summary: "The real library Weft runs for each transport: setup, options and limits.",
    groups: [
      {
        folder: "http",
        pages: [
          page("express", "Express", "Setup, options and limits for Express."),
          page("fastify", "Fastify", "Setup, options and limits for Fastify."),
          page("hono", "Hono", "Setup, options and limits for Hono."),
          page("nestjs", "NestJS", "Setup, options and limits for NestJS."),
        ],
      },
      {
        folder: "graphql",
        pages: [
          page("apollo-server", "Apollo Server", "Setup, options and limits for Apollo Server."),
          page("graphql-yoga", "GraphQL Yoga", "Setup, options and limits for GraphQL Yoga."),
        ],
      },
      {
        folder: "kafka",
        pages: [
          page("kafkajs", "KafkaJS", "Setup, options and limits for KafkaJS."),
          page("confluent-kafka-javascript", "confluent-kafka-javascript", "Setup, options and limits for confluent-kafka-javascript."),
        ],
      },
      {
        folder: "websockets",
        pages: [
          page("socket-io", "Socket.io", "Setup, options and limits for Socket.io."),
          page("ws", "ws", "Setup, options and limits for ws."),
        ],
      },
    ],
  },
  {
    id: "cli",
    label: "CLI",
    summary: "Every weft command and the weft.yaml it reads.",
    groups: [
      {
        folder: "commands",
        pages: [
          page("init", "weft init", "Pick transports and libraries, install packages, write weft.yaml."),
          page("add", "weft add", "Add a transport to an existing project."),
          page("gen", "weft gen", "Generate typed code from contracts."),
          page("check", "weft check", "Detect breaking contract changes."),
          page("fmt", "weft fmt", "Format .weft files."),
          page("import", "weft import", "Convert OpenAPI, .proto, GraphQL SDL, AsyncAPI, Avro or JSON Schema."),
          page("publish", "weft publish", "Push contracts to the dashboard or Weft Cloud."),
          page("pull", "weft pull", "Fetch another service's published contracts."),
          page("map", "weft map", "Write a static HTML map of services from contracts."),
          page("dashboard", "weft dashboard", "Run the dashboard locally."),
          page("doctor", "weft doctor", "Check config, broker connections and package versions."),
        ],
      },
      { folder: "config", pages: [page("weft-yaml", "weft.yaml", "The project file: transports, libraries, languages and where generated code goes.")] },
    ],
  },
  {
    id: "dashboard",
    label: "Dashboard",
    summary: "The service map, live traffic, contracts and traces.",
    groups: [
      {
        folder: "views",
        pages: [
          page("service-map", "Service map", "Every service and every operation between them, built from your contracts."),
          page("live-traffic", "Live traffic", "Throughput, latency, error rate, consumer lag and queue depth."),
          page("contracts", "Contracts", "Browse them, diff versions and see who still uses a deprecated field."),
          page("traces", "Traces", "Follow one request across transports and languages."),
        ],
      },
      {
        folder: "running",
        pages: [
          page("on-your-laptop", "On your laptop", "One developer, with services running locally or in Docker Compose."),
          page("in-your-cluster", "In your cluster", "A shared view for a team, without sending data out."),
          page("weft-cloud", "In Weft Cloud", "Nothing to run, with team features on top."),
        ],
      },
    ],
  },
];

/** Every page with its section and folder, in reading order. */
export interface FlatPage extends DocPage {
  section: DocSection;
  folder: string;
  href: string;
}

export const flat: FlatPage[] = sections.flatMap((section) =>
  section.groups.flatMap((g) => g.pages.map((p) => ({ ...p, section, folder: g.folder, href: `/docs/${section.id}/${p.slug}` }))),
);

export function neighbours(href: string): { prev: FlatPage | undefined; next: FlatPage | undefined } {
  const i = flat.findIndex((p) => p.href === href);
  const same = (p?: FlatPage) => (p && p.section.id === flat[i]?.section.id ? p : undefined);
  return { prev: same(flat[i - 1]), next: same(flat[i + 1]) };
}

/** What the search box sees: one entry per page and per section. */
export const searchIndex = [
  ...sections.map((s) => ({ title: s.label, text: s.summary, where: "Section", href: `/docs/${s.id}` })),
  ...flat.map((p) => ({ title: p.title, text: p.summary, where: `${p.section.label} / ${p.folder}`, href: p.href })),
];
