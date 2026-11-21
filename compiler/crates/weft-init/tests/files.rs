use weft_check::{check, Source};
use weft_init::*;

fn choices(transports: &[(&str, &str)]) -> Choices {
    Choices {
        language: Language::TypeScript,
        service: "orders".into(),
        contracts: "./contracts".into(),
        out: "src/weft.gen.ts".into(),
        transports: transports
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect(),
    }
}

/// The default library of every transport.
fn defaults() -> Vec<(String, String)> {
    transports(Language::TypeScript)
        .iter()
        .map(|t| {
            (
                t.id.to_string(),
                t.libraries
                    .first()
                    .map_or(String::new(), |l| l.id.to_string()),
            )
        })
        .collect()
}

fn assert_works(c: &Choices) {
    let contract = example_contract(c);

    // It is valid, already formatted, and generates TypeScript.
    let irs = check(&[Source::new("example.weft", contract.clone())])
        .unwrap_or_else(|e| panic!("{e:?}\n{contract}"));
    assert_eq!(
        weft_syntax::format("x", &contract).unwrap(),
        contract,
        "the example is not formatted:\n{contract}"
    );
    weft_codegen::typescript(&irs, &weft_codegen::Options::default()).unwrap();

    // weft.yaml parses and says what was chosen.
    let yaml = weft_yaml(c);
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&yaml).unwrap_or_else(|e| panic!("{e}\n{yaml}"));
    assert_eq!(doc["service"], "orders");
    assert_eq!(doc["language"], "typescript");
    for (id, lib) in &c.transports {
        let t = &doc["transports"][id.as_str()];
        assert!(t.is_mapping(), "{id} missing in\n{yaml}");
        if !lib.is_empty() {
            assert_eq!(t["library"], lib.as_str(), "{id}");
        }
    }
}

#[test]
fn each_transport_alone_gives_a_working_project() {
    for t in transports(Language::TypeScript) {
        for lib in t
            .libraries
            .iter()
            .map(|l| l.id)
            .chain(t.libraries.is_empty().then_some(""))
        {
            assert_works(&choices(&[(t.id, lib)]));
        }
    }
}

#[test]
fn every_transport_together_gives_a_working_project() {
    let all = defaults();
    let borrowed: Vec<(&str, &str)> = all.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    assert_works(&choices(&borrowed));
}

#[test]
fn the_example_uses_what_was_chosen() {
    let c = example_contract(&choices(&[("http", "express"), ("kafka", "kafkajs")]));
    assert!(c.contains("http {") && c.contains("event item.created: Item"));
    assert!(!c.contains("graphql") && !c.contains("rpc") && !c.contains("socket"));
    let c = example_contract(&choices(&[("gql", "apollo-server"), ("ws", "ws")]));
    assert!(c.contains("graphql {") && c.contains("socket updates") && !c.contains("event "));
}

#[test]
fn weft_yaml_matches_the_documented_shape() {
    let yaml = weft_yaml(&choices(&[("http", "express"), ("kafka", "kafkajs")]));
    assert_eq!(
        yaml,
        "service: orders\nlanguage: typescript\n\ncontracts:\n  - ./contracts\n\ncodegen:\n  out: src/weft.gen.ts\n\ntransports:\n  http:\n    library: express\n    port: 3000\n  kafka:\n    library: kafkajs\n    brokers: ${KAFKA_BROKERS}\n    group: orders\n"
    );
}

#[test]
fn catalogue_lookups() {
    assert_eq!(transports(Language::TypeScript).len(), 13);
    assert_eq!(
        transport(Language::TypeScript, "kafka").unwrap().libraries[0].id,
        "kafkajs"
    );
    assert!(library(Language::TypeScript, "http", "fastify").is_some());
    assert!(library(Language::TypeScript, "http", "kafkajs").is_none());
    assert!(transport(Language::TypeScript, "carrier-pigeon").is_none());
    assert!(transport(Language::TypeScript, "sse")
        .unwrap()
        .libraries
        .is_empty());
}

#[test]
fn detection() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("detect");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(detect(&dir), Detected::default());
    for (file, pm) in [
        ("package-lock.json", PackageManager::Npm),
        ("yarn.lock", PackageManager::Yarn),
        ("bun.lockb", PackageManager::Bun),
        ("pnpm-lock.yaml", PackageManager::Pnpm),
    ] {
        let d = dir.join(file.replace('.', "_"));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("package.json"), "{}").unwrap();
        std::fs::write(d.join(file), "").unwrap();
        assert_eq!(
            detect(&d),
            Detected {
                language: Some(Language::TypeScript),
                package_manager: Some(pm)
            },
            "{file}"
        );
    }
    let d = dir.join("bare");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("package.json"), "{}").unwrap();
    assert_eq!(detect(&d).package_manager, Some(PackageManager::Npm));
    let d = dir.join("deno");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("deno.json"), "{}").unwrap();
    assert_eq!(detect(&d).package_manager, Some(PackageManager::Deno));
}
