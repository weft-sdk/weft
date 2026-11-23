//! The thirteen transports and, for each language, the libraries Weft can run under each ([05-languages.md]).
//!
//! [05-languages.md]: ../../../../docs/05-languages.md

use crate::detect::Language;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportKind {
    /// Messages through a broker: `kafka`, `mq`, `nats`, `redis`, `cloud`, `mqtt`, `pulsar`.
    Messaging,
    /// Request and response: `http`, `gql`, `grpc`, `rpc`.
    Request,
    /// A long-lived connection to a client: `ws`, `sse`.
    Realtime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Library {
    /// The name used in `weft.yaml` and on the command line.
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transport {
    /// `http`, `kafka`, ... as in `weft.http` and `weft.yaml`.
    pub id: &'static str,
    pub label: &'static str,
    pub kind: TransportKind,
    /// The libraries to choose from. The first is the default. Empty when the transport is built on another one
    /// (`sse` runs on the HTTP library).
    pub libraries: &'static [Library],
}

const fn lib(id: &'static str, label: &'static str) -> Library {
    Library { id, label }
}

const fn t(
    id: &'static str,
    label: &'static str,
    kind: TransportKind,
    libraries: &'static [Library],
) -> Transport {
    Transport {
        id,
        label,
        kind,
        libraries,
    }
}

use TransportKind::{Messaging, Realtime, Request};

const TYPESCRIPT: &[Transport] = &[
    t(
        "http",
        "HTTP",
        Request,
        &[
            lib("express", "Express"),
            lib("fastify", "Fastify"),
            lib("hono", "Hono"),
            lib("nestjs", "NestJS"),
        ],
    ),
    t(
        "gql",
        "GraphQL",
        Request,
        &[
            lib("apollo-server", "Apollo Server"),
            lib("graphql-yoga", "GraphQL Yoga"),
        ],
    ),
    t(
        "grpc",
        "gRPC",
        Request,
        &[lib("grpc-js", "grpc-js"), lib("connect", "Connect")],
    ),
    t("rpc", "Typed RPC", Request, &[lib("trpc", "tRPC")]),
    t(
        "ws",
        "WebSockets",
        Realtime,
        &[lib("socket.io", "Socket.io"), lib("ws", "ws")],
    ),
    t("sse", "Server-sent events", Realtime, &[]),
    t(
        "kafka",
        "Kafka",
        Messaging,
        &[
            lib("kafkajs", "KafkaJS"),
            lib("confluent-kafka-javascript", "confluent-kafka-javascript"),
        ],
    ),
    t("mq", "RabbitMQ", Messaging, &[lib("amqplib", "amqplib")]),
    t("nats", "NATS", Messaging, &[lib("nats", "nats")]),
    t(
        "redis",
        "Redis",
        Messaging,
        &[lib("ioredis", "ioredis"), lib("node-redis", "node-redis")],
    ),
    t(
        "cloud",
        "Cloud queues",
        Messaging,
        &[
            lib("aws-sdk", "AWS SDK"),
            lib("google-cloud", "Google Cloud"),
            lib("azure-sdk", "Azure SDK"),
        ],
    ),
    t("mqtt", "MQTT", Messaging, &[lib("mqtt.js", "MQTT.js")]),
    t(
        "pulsar",
        "Pulsar",
        Messaging,
        &[lib("pulsar-client", "pulsar-client")],
    ),
];

const GO: &[Transport] = &[
    t(
        "http",
        "HTTP",
        Request,
        &[
            lib("nethttp", "net/http"),
            lib("chi", "chi"),
            lib("gin", "gin"),
            lib("echo", "echo"),
            lib("fiber", "fiber"),
        ],
    ),
    t("gql", "GraphQL", Request, &[lib("gqlgen", "gqlgen")]),
    t(
        "grpc",
        "gRPC",
        Request,
        &[lib("grpc-go", "grpc-go"), lib("connect-go", "connect-go")],
    ),
    t("rpc", "Typed RPC", Request, &[]),
    t(
        "ws",
        "WebSockets",
        Realtime,
        &[
            lib("gorilla", "gorilla/websocket"),
            lib("coder", "coder/websocket"),
        ],
    ),
    t("sse", "Server-sent events", Realtime, &[]),
    t(
        "kafka",
        "Kafka",
        Messaging,
        &[
            lib("franz", "franz-go"),
            lib("sarama", "sarama"),
            lib("kafka-go", "kafka-go"),
        ],
    ),
    t("mq", "RabbitMQ", Messaging, &[lib("amqp091", "amqp091-go")]),
    t("nats", "NATS", Messaging, &[lib("nats", "nats.go")]),
    t("redis", "Redis", Messaging, &[lib("go-redis", "go-redis")]),
    t(
        "cloud",
        "Cloud queues",
        Messaging,
        &[
            lib("aws-sdk", "AWS SDK"),
            lib("google-cloud", "Google Cloud"),
            lib("azure-sdk", "Azure SDK"),
        ],
    ),
    t("mqtt", "MQTT", Messaging, &[lib("paho", "paho")]),
    t(
        "pulsar",
        "Pulsar",
        Messaging,
        &[lib("pulsar-client", "pulsar-client-go")],
    ),
];

/// The transports available for a language, in the order they are offered.
pub fn transports(language: Language) -> &'static [Transport] {
    match language {
        Language::TypeScript => TYPESCRIPT,
        Language::Go => GO,
    }
}

pub fn transport(language: Language, id: &str) -> Option<&'static Transport> {
    transports(language).iter().find(|t| t.id == id)
}

pub fn library(
    language: Language,
    transport_id: &str,
    library_id: &str,
) -> Option<&'static Library> {
    transport(language, transport_id)?
        .libraries
        .iter()
        .find(|l| l.id == library_id)
}

/// The packages to install for the chosen transports and libraries: always the core package, then for
/// each transport that has a Weft package for its library, that package and the library itself.
/// `chosen` is `(transport id, library id)`. Transports with no Weft package yet add nothing.
pub fn packages(language: Language, chosen: &[(String, String)]) -> Vec<String> {
    if language == Language::Go {
        // One module holds the core and every adapter ([ADR 0022]); the libraries come with it.
        //
        // [ADR 0022]: ../../../../docs/decisions/0022-go-package-lives-in-go.md
        return vec!["github.com/weft-sdk/weft/go@latest".to_string()];
    }
    let mut out = vec!["@weft-sdk/core".to_string()];
    for (transport, library) in chosen {
        let (adapter, library_package) = match (transport.as_str(), library.as_str()) {
            ("http", "express") => ("@weft-sdk/http-express", "express"),
            ("kafka", "kafkajs") => ("@weft-sdk/kafka-kafkajs", "kafkajs"),
            _ => continue,
        };
        out.push(adapter.to_string());
        out.push(library_package.to_string());
        // Express ships no types, and the generated file imports them.
        if library_package == "express" {
            out.push("@types/express".to_string());
        }
    }
    out
}
