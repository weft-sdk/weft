use weft_ir::*;

const SCHEMA_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../spec/ir.schema.json");

#[test]
fn schema_file_is_up_to_date() {
    let generated = json_schema();
    if std::env::var_os("UPDATE_SCHEMA").is_some() {
        std::fs::write(SCHEMA_PATH, &generated).unwrap();
    }
    let on_disk = std::fs::read_to_string(SCHEMA_PATH).unwrap_or_default();
    assert!(
        on_disk == generated,
        "spec/ir.schema.json is stale. Regenerate it with:\n  UPDATE_SCHEMA=1 cargo test -p weft-ir --test schema"
    );
}

fn prim(name: Primitive) -> TypeRef {
    TypeRef::Primitive { name }
}

fn named(name: &str) -> TypeRef {
    TypeRef::Named {
        package: "orders".into(),
        name: name.into(),
        args: vec![],
    }
}

fn field(name: &str, ty: TypeRef) -> FieldDef {
    FieldDef {
        name: name.into(),
        wire_name: name.into(),
        ty,
        optional: false,
        doc: vec![],
        deprecated: None,
        rules: vec![],
    }
}

/// An IR that uses every part of the format at least once.
pub fn sample() -> Ir {
    let mut ir = Ir::new("orders");
    ir.version = Some("1.0.0".into());
    ir.imports = vec!["payments".into()];
    let mut qty = field("qty", prim(Primitive::Int));
    qty.rules = vec![
        Rule::Min { value: "1".into() },
        Rule::Max { value: "100".into() },
        Rule::Length { min: 1, max: 2 },
        Rule::Pattern { regex: "^a$".into() },
        Rule::Format { format: Format::Email },
    ];
    let mut note = field("note", prim(Primitive::String));
    note.optional = true;
    note.deprecated = Some("use memo".into());
    ir.types = vec![
        TypeDef {
            name: "Currency".into(),
            doc: vec!["A currency.".into()],
            params: vec![],
            inline: false,
            deprecated: None,
            body: TypeBody::Enum {
                variants: vec![VariantDef {
                    name: "EUR".into(),
                    doc: vec![],
                    deprecated: None,
                }],
            },
        },
        TypeDef {
            name: "Page".into(),
            doc: vec![],
            params: vec!["T".into()],
            inline: false,
            deprecated: None,
            body: TypeBody::Record {
                fields: vec![
                    field("items", TypeRef::List { item: Box::new(TypeRef::Param { name: "T".into() }) }),
                    field(
                        "tags",
                        TypeRef::Map {
                            key: Box::new(prim(Primitive::String)),
                            value: Box::new(TypeRef::Named {
                                package: "payments".into(),
                                name: "Page".into(),
                                args: vec![named("Order")],
                            }),
                        },
                    ),
                ],
            },
        },
        TypeDef {
            name: "OrderCancelled".into(),
            doc: vec![],
            params: vec![],
            inline: true,
            deprecated: None,
            body: TypeBody::Record { fields: vec![qty, note] },
        },
    ];
    ir.errors = vec![ErrorDef {
        name: "OutOfStock".into(),
        doc: vec![],
        status: 422,
        fields: vec![field("sku", prim(Primitive::String))],
    }];
    let result = ResultDef {
        value: named("Order"),
        optional: true,
        stream: false,
        errors: vec![
            ErrorRef { package: "orders".into(), name: "OutOfStock".into() },
            ErrorRef { package: BUILTIN_PACKAGE.into(), name: "NotFound".into() },
        ],
    };
    ir.events = vec![EventDef {
        name: "order.placed".into(),
        doc: vec![],
        payload: named("Order"),
        key: vec!["id".into()],
        headers: Some(named("OrderPlacedHeaders")),
        deprecated: None,
    }];
    ir.routes = vec![RouteDef {
        name: "getOrder".into(),
        doc: vec![],
        method: HttpMethod::Get,
        base: Some("/v1".into()),
        path: "/orders/{id}".into(),
        path_params: vec![ParamDef { name: "id".into(), ty: prim(Primitive::Uuid), optional: false }],
        query: Some(named("GetOrderQuery")),
        body: Some(named("CreateOrder")),
        headers: None,
        result: result.clone(),
        deprecated: None,
    }];
    ir.rpc_services = vec![RpcServiceDef {
        name: "Orders".into(),
        methods: vec![RpcMethodDef {
            name: "streamUpdates".into(),
            doc: vec![],
            request: named("StreamUpdatesRequest"),
            client_streaming: true,
            result: ResultDef { stream: true, ..result.clone() },
            deprecated: None,
        }],
    }];
    ir.graphql = vec![GraphqlFieldDef {
        kind: GraphqlKind::Query,
        name: "order".into(),
        doc: vec![],
        args: vec![ParamDef { name: "id".into(), ty: prim(Primitive::Uuid), optional: true }],
        result,
        deprecated: None,
    }];
    ir.sockets = vec![SocketDef {
        name: "tracking".into(),
        events: vec![SocketEventDef {
            name: "courier.moved".into(),
            doc: vec![],
            side: Side::Server,
            payload: named("CourierMoved"),
            deprecated: None,
        }],
    }];
    ir
}

fn validator() -> jsonschema::Validator {
    let schema: serde_json::Value = serde_json::from_str(&json_schema()).unwrap();
    jsonschema::validator_for(&schema).expect("the schema is a valid JSON Schema")
}

#[test]
fn sample_validates_against_the_schema() {
    let value: serde_json::Value = serde_json::from_str(&sample().to_json()).unwrap();
    let v = validator();
    let errors: Vec<String> = v.iter_errors(&value).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn round_trips_through_json() {
    let ir = sample();
    assert_eq!(Ir::from_json(&ir.to_json()).unwrap(), ir);
}

#[test]
fn schema_rejects_bad_documents() {
    let v = validator();
    for bad in [
        r#"{}"#,
        r#"{"irVersion":"0.1"}"#,
        r#"{"irVersion":"0.1","package":"p","types":[{"name":"T","kind":"tuple"}]}"#,
        r#"{"irVersion":"0.1","package":"p","routes":[{"name":"a","method":"FETCH","path":"/","result":{"value":{"kind":"primitive","name":"int"}}}]}"#,
    ] {
        let value: serde_json::Value = serde_json::from_str(bad).unwrap();
        assert!(!v.is_valid(&value), "should be rejected: {bad}");
    }
}

#[test]
fn minimal_document_is_valid() {
    let value: serde_json::Value = serde_json::from_str(&Ir::new("p").to_json()).unwrap();
    assert!(validator().is_valid(&value));
}

#[test]
fn json_uses_camel_case_and_tags() {
    let json = sample().to_json();
    for expected in ["\"irVersion\": \"0.1\"", "\"wireName\"", "\"rpcServices\"", "\"kind\": \"record\"", "\"method\": \"GET\"", "\"rule\": \"min\""] {
        assert!(json.contains(expected), "missing {expected}");
    }
}
