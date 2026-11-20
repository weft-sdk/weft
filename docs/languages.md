# Languages and libraries

Every language Weft supports, and the library it runs underneath for each transport. The list of libraries per cell is what adapters are written for. A user picks one per transport during `weft init`.

## Languages

**Backend** — every transport:

| Language | Runtimes |
| --- | --- |
| TypeScript / JavaScript | Node, Bun, Deno |
| Go | |
| Python | CPython 3.10+ |
| JVM: Java, Kotlin, Scala, Clojure | One package for all JVM languages |
| .NET: C#, F# | One package for both |
| Rust | Tokio |
| Ruby | |
| PHP | 8.2+ |
| Elixir / Erlang | BEAM |

**Frontend and mobile** — client-side transports only (HTTP, GraphQL, gRPC-Web, WebSockets, SSE):

| Platform | Language |
| --- | --- |
| Browser | TypeScript / JavaScript |
| iOS, macOS | Swift (also server-side via Vapor) |
| Android | Kotlin (JVM package) |
| Flutter | Dart |

## Request and response

| Language | `weft.http` (serve · call) | `weft.gql` (serve · call) | `weft.grpc` |
| --- | --- | --- | --- |
| TypeScript | Express, Fastify, Hono, NestJS · fetch, axios | Apollo Server, GraphQL Yoga · Apollo Client, urql | grpc-js, Connect |
| Go | net/http, chi, gin, echo, fiber · net/http | gqlgen · genqlient | grpc-go, connect-go |
| Python | FastAPI, Flask, Django · httpx, requests | Strawberry, Graphene, Ariadne · gql | grpcio |
| JVM | Spring Web, Ktor, Quarkus, Vert.x · OkHttp, WebClient | Spring GraphQL, DGS, graphql-java · Apollo Kotlin | grpc-java, grpc-kotlin |
| .NET | ASP.NET Core · HttpClient | Hot Chocolate · Strawberry Shake | Grpc.Net |
| Rust | axum, actix-web · reqwest | async-graphql | tonic |
| Ruby | Rails, Sinatra · Faraday | graphql-ruby | grpc |
| PHP | Laravel, Symfony · Guzzle | Lighthouse, graphql-php | grpc (client), RoadRunner (server) |
| Elixir | Phoenix, Plug · Req, Finch | Absinthe | grpc-elixir |
| Swift | Vapor · URLSession | Apollo iOS | grpc-swift |
| Dart | shelf · dio, http | ferry, graphql_flutter | grpc-dart |

## Events and messages

| Language | `weft.kafka` | `weft.mq` (RabbitMQ) | `weft.ws` |
| --- | --- | --- | --- |
| TypeScript | KafkaJS, confluent-kafka-javascript | amqplib | Socket.io, ws |
| Go | franz-go, sarama, kafka-go | amqp091-go | gorilla/websocket, coder/websocket |
| Python | confluent-kafka, aiokafka | pika, aio-pika | websockets, python-socketio |
| JVM | kafka-clients, spring-kafka | RabbitMQ Java client, spring-amqp | Spring WebSocket, Ktor |
| .NET | Confluent.Kafka | RabbitMQ.Client, MassTransit | SignalR, System.Net.WebSockets |
| Rust | rust-rdkafka | lapin | tokio-tungstenite |
| Ruby | Karafka, rdkafka-ruby | Bunny | Action Cable |
| PHP | php-rdkafka | php-amqplib | Ratchet, Laravel Reverb |
| Elixir | brod, Broadway | amqp, Broadway | Phoenix Channels |
| Swift | swift-kafka-client | limited | URLSessionWebSocketTask, Starscream |
| Dart | — | — | web_socket_channel |

## Other transports

| Transport | Libraries |
| --- | --- |
| `weft.rpc` | tRPC in TypeScript; Weft's own HTTP-based protocol in every other language |
| `weft.sse` | Built on each language's HTTP adapter |
| `weft.nats` | Official NATS clients (TS, Go, Python, Java, .NET, Rust, Ruby, Elixir, Swift) |
| `weft.redis` | ioredis/node-redis, go-redis, redis-py, Lettuce/Jedis, StackExchange.Redis, redis-rs, redis-rb, Predis/phpredis, Redix |
| `weft.cloud` | Official AWS, Google Cloud and Azure SDKs per language |
| `weft.mqtt` | MQTT.js, paho (Go, Python, Java), MQTTnet, rumqttc, ruby-mqtt, php-mqtt, Tortoise |
| `weft.pulsar` | Official Pulsar clients (TS, Go, Python, Java, .NET, Rust) |

## Limits

These come from the ecosystems, not from Weft. Weft reports them as clear errors at `weft init` or startup.

- **tRPC exists only in TypeScript.** `weft.rpc` gives the same experience elsewhere using Weft's protocol, and the two interoperate.
- **Browsers and mobile apps don't connect to brokers.** Kafka, RabbitMQ and the like are server-side only.
- **Edge runtimes** (Cloudflare Workers, Vercel Edge) can't open raw TCP, so no Kafka, RabbitMQ, NATS or Redis there. HTTP, WebSockets and HTTP-based cloud queues work.
- **PHP gRPC servers** need RoadRunner. Clients work everywhere.
- **Swift and Dart** have thin messaging client libraries. Weft support there is only as good as those libraries.

## JavaScript without TypeScript

The npm package ships compiled JavaScript with type definitions, both ESM and CommonJS. Plain JavaScript projects use the same API. `weft gen --lang js` emits JSDoc-typed JavaScript for editor autocomplete, and payloads are validated at runtime.
