//! The intermediate representation (IR): the checked, fully resolved form of a package's contracts.
//!
//! Code generators, `weft check`, the dashboard and third-party tools read the IR and never the `.weft`
//! syntax, so the syntax can change without breaking them. The JSON form is described by
//! `spec/ir.schema.json`, which is generated from the types in this crate.
//!
//! One [`Ir`] describes one package. Other packages are referred to by name and never copied in.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The version of the IR format. Bumped when the JSON shape changes.
pub const IR_VERSION: &str = "0.1";

/// The package that owns the built-in errors (`weft.NotFound` and so on). No contract may use it as a package name.
pub const BUILTIN_PACKAGE: &str = "weft";

/// The built-in errors that exist in every package, with their HTTP status.
pub const BUILTIN_ERRORS: [(&str, u16); 8] = [
    ("NotFound", 404),
    ("InvalidInput", 400),
    ("Unauthorized", 401),
    ("Forbidden", 403),
    ("Conflict", 409),
    ("Unavailable", 503),
    ("Timeout", 504),
    ("Internal", 500),
];

/// The default HTTP status of an error declared with `error`.
pub const DEFAULT_CUSTOM_ERROR_STATUS: u16 = 422;

/// One package's contracts, checked and resolved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(title = "Weft IR")]
pub struct Ir {
    /// The IR format version this document uses ([`IR_VERSION`]).
    pub ir_version: String,
    /// The package name.
    pub package: String,
    /// The package's semantic version, from `@version`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Packages this one imports, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
    /// Records and enums, including types generated for inline types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<TypeDef>,
    /// Errors declared with `error`. Built-in errors are implicit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ErrorDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<EventDef>,
    /// HTTP routes of every `http` block, flattened.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<RouteDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rpc_services: Vec<RpcServiceDef>,
    /// GraphQL root fields of every `graphql` block, flattened.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graphql: Vec<GraphqlFieldDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sockets: Vec<SocketDef>,
}

impl Ir {
    pub fn new(package: impl Into<String>) -> Self {
        Ir {
            ir_version: IR_VERSION.to_string(),
            package: package.into(),
            version: None,
            imports: Vec::new(),
            types: Vec::new(),
            errors: Vec::new(),
            events: Vec::new(),
            routes: Vec::new(),
            rpc_services: Vec::new(),
            graphql: Vec::new(),
            sockets: Vec::new(),
        }
    }

    /// The IR as pretty-printed JSON with a trailing newline.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("the IR always serialises");
        s.push('\n');
        s
    }

    pub fn from_json(json: &str) -> Result<Ir, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// The JSON Schema of the IR, pretty-printed with a trailing newline. Written to `spec/ir.schema.json`.
pub fn json_schema() -> String {
    let schema = schemars::schema_for!(Ir);
    let mut s = serde_json::to_string_pretty(&schema).expect("the schema always serialises");
    s.push('\n');
    s
}

// ---- types ----

/// A reference to a type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TypeRef {
    /// A built-in scalar.
    Primitive { name: Primitive },
    /// A record or enum, by package and name, with type arguments for generic types.
    Named {
        package: String,
        name: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<TypeRef>,
    },
    /// A type parameter of the enclosing generic type, such as `T`.
    Param { name: String },
    /// `T[]`
    List { item: Box<TypeRef> },
    /// `map<K, V>`
    Map {
        key: Box<TypeRef>,
        value: Box<TypeRef>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Primitive {
    String,
    Bool,
    /// 64-bit.
    Int,
    Int32,
    Int64,
    /// 64-bit.
    Float,
    Float32,
    Float64,
    Decimal,
    Uuid,
    Timestamp,
    Bytes,
}

/// A record or an enum.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TypeDef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doc: Vec<String>,
    /// Type parameters, as in `Page<T>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<String>,
    /// Set when the type was written inline and given a generated name.
    #[serde(default, skip_serializing_if = "is_false")]
    pub inline: bool,
    /// `@deprecated`: the message, or an empty string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    #[serde(flatten)]
    pub body: TypeBody,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TypeBody {
    Record { fields: Vec<FieldDef> },
    Enum { variants: Vec<VariantDef> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VariantDef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doc: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FieldDef {
    pub name: String,
    /// The name on the wire. Equal to `name` unless `@wire` says otherwise.
    pub wire_name: String,
    #[serde(rename = "type")]
    pub ty: TypeRef,
    /// `name?: T`: the value may be missing or null.
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doc: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<Rule>,
}

/// A validation rule. Numbers are kept as written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "rule", rename_all = "camelCase")]
pub enum Rule {
    Min { value: String },
    Max { value: String },
    Length { min: u64, max: u64 },
    Pattern { regex: String },
    Format { format: Format },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Email,
    Url,
    Uri,
    Hostname,
    Ipv4,
    Ipv6,
    Date,
    Time,
}

impl Format {
    pub const ALL: [Format; 8] = [
        Format::Email,
        Format::Url,
        Format::Uri,
        Format::Hostname,
        Format::Ipv4,
        Format::Ipv6,
        Format::Date,
        Format::Time,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Format::Email => "email",
            Format::Url => "url",
            Format::Uri => "uri",
            Format::Hostname => "hostname",
            Format::Ipv4 => "ipv4",
            Format::Ipv6 => "ipv6",
            Format::Date => "date",
            Format::Time => "time",
        }
    }
}

/// An error declared with `error`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doc: Vec<String>,
    /// HTTP status. 422 unless `@status` says otherwise.
    pub status: u16,
    pub fields: Vec<FieldDef>,
}

/// A reference to an error: a declared one, or a built-in one in the package `weft`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorRef {
    pub package: String,
    pub name: String,
}

// ---- operations ----

/// What an operation gives back: a value, or one of the listed errors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResultDef {
    pub value: TypeRef,
    /// `-> T?`: the result may be empty (HTTP 404, GraphQL null, gRPC NOT_FOUND).
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
    /// `-> stream T`: a stream of values.
    #[serde(default, skip_serializing_if = "is_false")]
    pub stream: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ErrorRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParamDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: TypeRef,
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventDef {
    /// The event name, which is the topic, queue or subject unless `weft.yaml` remaps it.
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doc: Vec<String>,
    pub payload: TypeRef,
    /// The routing key as a field path into the payload (`key: id` is `["id"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key: Vec<String>,
    /// Message headers, a record type with string fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<TypeRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteDef {
    /// The function name in generated clients.
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doc: Vec<String>,
    pub method: HttpMethod,
    /// The base path of the enclosing `http` block, such as `/v1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// The route's own path, such as `/orders/{id}`.
    pub path: String,
    /// Parameters that appear in the path, in path order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path_params: Vec<ParamDef>,
    /// The `query` parameter: a record type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<TypeRef>,
    /// The `body` parameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<TypeRef>,
    /// The `headers` parameter: a record type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<TypeRef>,
    pub result: ResultDef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RpcServiceDef {
    pub name: String,
    pub methods: Vec<RpcMethodDef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RpcMethodDef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doc: Vec<String>,
    /// The request message. Named parameters become a generated record type.
    pub request: TypeRef,
    /// `(stream Req)`: the client streams requests.
    #[serde(default, skip_serializing_if = "is_false")]
    pub client_streaming: bool,
    /// `result.stream` says whether the server streams responses.
    pub result: ResultDef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum GraphqlKind {
    Query,
    Mutation,
    Subscription,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphqlFieldDef {
    pub kind: GraphqlKind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doc: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<ParamDef>,
    pub result: ResultDef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SocketDef {
    pub name: String,
    pub events: Vec<SocketEventDef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Server,
    Client,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SocketEventDef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doc: Vec<String>,
    /// Which side sends the event.
    pub side: Side,
    pub payload: TypeRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}
