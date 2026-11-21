//! The text of the files `weft init` writes.

use crate::catalog::{transport, TransportKind};
use crate::detect::Language;

/// What the user chose.
#[derive(Clone, Debug)]
pub struct Choices {
    pub language: Language,
    /// The service, which is also the name of its contract package.
    pub service: String,
    /// Where contracts live, as written in `weft.yaml`: `./contracts`.
    pub contracts: String,
    /// Where generated code goes: `src/weft.gen.ts`.
    pub out: String,
    /// Each chosen transport and its library (empty for transports that have none).
    pub transports: Vec<(String, String)>,
}

/// Why `name` cannot be a service name, if it cannot. A service is also a contract package, so it is an identifier
/// of lower-case letters, digits and `_`.
pub fn validate_service_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let ok = chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !ok {
        return Err(format!(
            "`{name}` is not a valid service name: start with a lower-case letter, then use lower-case letters, digits and `_`"
        ));
    }
    if name == "weft" {
        return Err("`weft` is reserved".into());
    }
    if name.len() > 40 {
        return Err("a service name is at most 40 characters".into());
    }
    Ok(())
}

/// A service name made from a folder name: `My Service-2` becomes `my_service_2`.
pub fn sanitize_service_name(folder: &str) -> String {
    let mut out = String::new();
    for c in folder.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    match out.chars().next() {
        None => "service".to_string(),
        _ if out == "weft" => "weft_service".to_string(),
        Some(c) if c.is_ascii_digit() => format!("s_{out}"),
        _ => out.chars().take(40).collect(),
    }
}

/// The settings each transport starts with. Values like `${KAFKA_BROKERS}` are read from the environment.
fn settings(id: &str, library: &str, service: &str) -> Vec<(&'static str, String)> {
    let env = |name: &str| format!("${{{name}}}");
    match id {
        "http" => vec![("port", "3000".into())],
        "gql" => vec![("path", "/graphql".into())],
        "grpc" => vec![("port", "50051".into())],
        "rpc" => vec![("path", "/rpc".into())],
        "ws" => vec![("path", "/ws".into())],
        "sse" => vec![("path", "/events".into())],
        "kafka" => vec![
            ("brokers", env("KAFKA_BROKERS")),
            ("group", service.to_string()),
        ],
        "mq" => vec![("url", env("AMQP_URL"))],
        "nats" => vec![("url", env("NATS_URL"))],
        "redis" => vec![("url", env("REDIS_URL"))],
        "cloud" => match library {
            "google-cloud" => vec![("project", env("GOOGLE_CLOUD_PROJECT"))],
            "azure-sdk" => vec![("connection", env("AZURE_SERVICEBUS_CONNECTION"))],
            _ => vec![("region", env("AWS_REGION"))],
        },
        "mqtt" => vec![("url", env("MQTT_URL"))],
        "pulsar" => vec![("url", env("PULSAR_URL"))],
        _ => vec![],
    }
}

/// The contents of `weft.yaml`.
pub fn weft_yaml(c: &Choices) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "service: {}\nlanguage: {}\n\n",
        c.service,
        c.language.id()
    ));
    s.push_str(&format!("contracts:\n  - {}\n\n", c.contracts));
    s.push_str(&format!("codegen:\n  out: {}\n\n", c.out));
    s.push_str("transports:\n");
    for (id, library) in &c.transports {
        s.push_str(&format!("  {id}:\n"));
        if !library.is_empty() {
            s.push_str(&format!("    library: {library}\n"));
        }
        for (key, value) in settings(id, library, &c.service) {
            s.push_str(&format!("    {key}: {value}\n"));
        }
    }
    s
}

/// A first contract that uses every chosen transport, in the formatter's style. It is only a starting point.
pub fn example_contract(c: &Choices) -> String {
    let has = |id: &str| c.transports.iter().any(|(t, _)| t == id);
    let messaging = c.transports.iter().any(|(id, _)| {
        transport(c.language, id).is_some_and(|t| t.kind == TransportKind::Messaging)
    });

    let mut s = String::new();
    s.push_str(&format!("// contracts/{}.weft\n", c.service));
    s.push_str(
        "// An example. Replace it with your own types and operations, then run `weft gen`.\n",
    );
    s.push_str(&format!("package {}\n\n", c.service));
    s.push_str(
        "/// Something your service knows about.\ntype Item {\n  id: uuid\n  name: string\n}\n",
    );
    if messaging {
        s.push_str(
            "\n// Sent through the broker (topic `item.created`).\nevent item.created: Item\n",
        );
    }
    if has("http") {
        s.push_str("\nhttp {\n  GET \"/items/{id}\" getItem(id: uuid) -> Item?\n}\n");
    }
    if has("grpc") || has("rpc") {
        s.push_str("\nrpc Items {\n  getItem(id: uuid) -> Item\n}\n");
    }
    if has("gql") {
        s.push_str("\ngraphql {\n  query item(id: uuid) -> Item?\n}\n");
    }
    if has("ws") || has("sse") {
        s.push_str("\nsocket updates {\n  server item.updated: Item\n}\n");
    }
    weft_syntax::format("example.weft", &s).expect("the example contract is valid syntax")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_names() {
        for ok in ["orders", "a", "order_service2", "x_"] {
            assert!(validate_service_name(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "Orders",
            "2fast",
            "my-service",
            "weft",
            "a b",
            "_x",
            &"a".repeat(41),
        ] {
            assert!(validate_service_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn folder_names_become_service_names() {
        for (folder, name) in [
            ("orders", "orders"),
            ("My Service-2", "my_service_2"),
            ("--api--", "api"),
            ("2fast", "s_2fast"),
            ("", "service"),
            ("日本", "service"),
        ] {
            let got = sanitize_service_name(folder);
            assert_eq!(got, name, "{folder}");
            assert!(validate_service_name(&got).is_ok(), "{got}");
        }
        // `weft` is reserved, so a folder with that name gets another.
        assert_eq!(sanitize_service_name("weft"), "weft_service");
    }
}
