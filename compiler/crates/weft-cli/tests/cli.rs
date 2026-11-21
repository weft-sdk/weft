use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn weft(args: &[&str], dir: &PathBuf) -> Output {
    Command::new(env!("CARGO_BIN_EXE_weft"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

/// A fresh empty folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

const MESSY: &str = "package p\ntype A{x:int}\n";
const CLEAN: &str = "package p\n\ntype A {\n  x: int\n}\n";

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

#[test]
fn version_is_printed() {
    let o = weft(&["--version"], &scratch("version"));
    assert!(String::from_utf8_lossy(&o.stdout).starts_with("weft "));
}

#[test]
fn fmt_rewrites_files_in_place() {
    let dir = scratch("fmt_rewrites");
    fs::write(dir.join("a.weft"), MESSY).unwrap();
    let o = weft(&["fmt"], &dir);
    assert_eq!(code(&o), 0);
    assert_eq!(fs::read_to_string(dir.join("a.weft")).unwrap(), CLEAN);
    assert!(String::from_utf8_lossy(&o.stdout).contains("formatted ./a.weft"));
    // A second run changes nothing.
    let o = weft(&["fmt"], &dir);
    assert_eq!(code(&o), 0);
    assert!(o.stdout.is_empty());
}

#[test]
fn fmt_check_changes_nothing_and_exits_1() {
    let dir = scratch("fmt_check");
    fs::write(dir.join("a.weft"), MESSY).unwrap();
    let o = weft(&["fmt", "--check"], &dir);
    assert_eq!(code(&o), 1);
    assert!(String::from_utf8_lossy(&o.stdout).contains("would reformat"));
    assert_eq!(fs::read_to_string(dir.join("a.weft")).unwrap(), MESSY);
}

#[test]
fn fmt_check_passes_on_formatted_files() {
    let dir = scratch("fmt_check_ok");
    fs::write(dir.join("a.weft"), CLEAN).unwrap();
    assert_eq!(code(&weft(&["fmt", "--check"], &dir)), 0);
}

#[test]
fn fmt_searches_folders_and_skips_hidden_and_dependency_folders() {
    let dir = scratch("fmt_walk");
    for sub in ["contracts", ".git", "node_modules", "target"] {
        fs::create_dir_all(dir.join(sub)).unwrap();
        fs::write(dir.join(sub).join("a.weft"), MESSY).unwrap();
    }
    assert_eq!(code(&weft(&["fmt"], &dir)), 0);
    assert_eq!(
        fs::read_to_string(dir.join("contracts/a.weft")).unwrap(),
        CLEAN
    );
    for sub in [".git", "node_modules", "target"] {
        assert_eq!(
            fs::read_to_string(dir.join(sub).join("a.weft")).unwrap(),
            MESSY,
            "{sub}"
        );
    }
}

#[test]
fn fmt_takes_explicit_files() {
    let dir = scratch("fmt_explicit");
    fs::write(dir.join("a.weft"), MESSY).unwrap();
    fs::write(dir.join("b.weft"), MESSY).unwrap();
    assert_eq!(code(&weft(&["fmt", "a.weft"], &dir)), 0);
    assert_eq!(fs::read_to_string(dir.join("a.weft")).unwrap(), CLEAN);
    assert_eq!(fs::read_to_string(dir.join("b.weft")).unwrap(), MESSY);
}

#[test]
fn fmt_reports_syntax_errors_with_exit_3_and_formats_the_rest() {
    let dir = scratch("fmt_broken");
    fs::write(dir.join("a.weft"), "package p\ntype A { x int }\n").unwrap();
    fs::write(dir.join("b.weft"), MESSY).unwrap();
    let o = weft(&["fmt"], &dir);
    assert_eq!(code(&o), 3);
    assert!(String::from_utf8_lossy(&o.stderr).contains("expected"));
    assert_eq!(fs::read_to_string(dir.join("b.weft")).unwrap(), CLEAN);
}

#[test]
fn fmt_of_a_missing_path_is_a_usage_error() {
    let dir = scratch("fmt_missing");
    assert_eq!(code(&weft(&["fmt", "nope.weft"], &dir)), 2);
}

#[test]
fn unknown_commands_are_a_usage_error() {
    assert_eq!(code(&weft(&["nope"], &scratch("unknown"))), 2);
}

// ---- weft gen ----

const ORDERS: &str = include_str!("../../../../spec/examples/orders.weft");

/// A project folder with a weft.yaml and the example contract.
fn project(name: &str, yaml: &str) -> PathBuf {
    let dir = scratch(name);
    fs::write(dir.join("weft.yaml"), yaml).unwrap();
    fs::create_dir_all(dir.join("contracts")).unwrap();
    fs::write(dir.join("contracts/orders.weft"), ORDERS).unwrap();
    dir
}

const YAML: &str = "service: orders\nlanguage: typescript\ncontracts:\n  - ./contracts\ncodegen:\n  out: src/weft.gen.ts\n\ntransports:\n  http:\n    library: express\n    port: 3000\n";

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn gen_writes_the_file_named_in_weft_yaml() {
    let dir = project("gen_writes", YAML);
    let o = weft(&["gen"], &dir);
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert!(text(&o).contains("Wrote src/weft.gen.ts"), "{}", text(&o));
    let out = fs::read_to_string(dir.join("src/weft.gen.ts")).unwrap();
    assert!(out.contains("export namespace orders {"));
    assert!(out.contains("export const contracts = {"));
    // Running again changes nothing.
    let o = weft(&["gen"], &dir);
    assert_eq!(code(&o), 0);
    assert!(text(&o).contains("up to date"));
}

#[test]
fn gen_check_passes_then_fails_when_a_contract_changes() {
    let dir = project("gen_check", YAML);
    assert_eq!(code(&weft(&["gen", "--check"], &dir)), 1, "no file yet");
    assert_eq!(code(&weft(&["gen"], &dir)), 0);
    assert_eq!(code(&weft(&["gen", "--check"], &dir)), 0);
    fs::write(
        dir.join("contracts/orders.weft"),
        ORDERS.replace("qty: int", "quantity: int"),
    )
    .unwrap();
    let o = weft(&["gen", "--check"], &dir);
    assert_eq!(code(&o), 1);
    assert!(text(&o).contains("out of date"));
    // --check writes nothing.
    assert!(!fs::read_to_string(dir.join("src/weft.gen.ts"))
        .unwrap()
        .contains("quantity"));
}

#[test]
fn gen_works_from_a_subfolder() {
    let dir = project("gen_subfolder", YAML);
    let sub = dir.join("contracts");
    let o = weft(&["gen"], &sub);
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert!(dir.join("src/weft.gen.ts").exists());
}

#[test]
fn gen_out_flag_overrides_the_config() {
    let dir = project("gen_out", YAML);
    let o = weft(&["gen", "--out", "generated/api.ts"], &dir);
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert!(dir.join("generated/api.ts").exists());
    assert!(!dir.join("src/weft.gen.ts").exists());
}

#[test]
fn gen_reads_int64_from_the_config() {
    let dir = project(
        "gen_int64",
        &YAML.replace("codegen:\n", "codegen:\n  int64: bigint\n"),
    );
    assert_eq!(code(&weft(&["gen"], &dir)), 0);
    assert!(fs::read_to_string(dir.join("src/weft.gen.ts"))
        .unwrap()
        .contains("amount: bigint;"));
}

#[test]
fn gen_with_a_bad_int64_is_a_usage_error() {
    let dir = project(
        "gen_bad_int64",
        &YAML.replace("codegen:\n", "codegen:\n  int64: huge\n"),
    );
    let o = weft(&["gen"], &dir);
    assert_eq!(code(&o), 2);
    assert!(text(&o).contains("huge"));
}

#[test]
fn gen_defaults_to_the_contracts_folder_and_the_default_output() {
    let dir = project("gen_defaults", "language: typescript\n");
    assert_eq!(code(&weft(&["gen"], &dir)), 0);
    assert!(dir.join("src/weft.gen.ts").exists());
}

#[test]
fn gen_reports_contract_errors_with_exit_3_and_writes_nothing() {
    let dir = project("gen_contract_error", YAML);
    fs::write(
        dir.join("contracts/orders.weft"),
        ORDERS.replace("total: Money", "total: Mony"),
    )
    .unwrap();
    let o = weft(&["gen"], &dir);
    assert_eq!(code(&o), 3);
    let t = text(&o);
    assert!(
        t.contains("unknown type `Mony`") && t.contains("did you mean `Money`"),
        "{t}"
    );
    assert!(!dir.join("src/weft.gen.ts").exists());
}

#[test]
fn gen_reports_syntax_errors_with_exit_3() {
    let dir = project("gen_syntax_error", YAML);
    fs::write(
        dir.join("contracts/orders.weft"),
        "package orders\ntype A { x int }",
    )
    .unwrap();
    assert_eq!(code(&weft(&["gen"], &dir)), 3);
}

#[test]
fn gen_reports_names_typescript_cannot_hold_with_exit_3() {
    let dir = project("gen_reserved", YAML);
    fs::write(
        dir.join("contracts/orders.weft"),
        "package orders\ntype Date { x: int }",
    )
    .unwrap();
    let o = weft(&["gen"], &dir);
    assert_eq!(code(&o), 3);
    assert!(text(&o).contains("`Date`"));
}

#[test]
fn gen_without_a_weft_yaml_is_a_usage_error() {
    let o = weft(&["gen"], &scratch("gen_no_config"));
    assert_eq!(code(&o), 2);
    assert!(text(&o).contains("no weft.yaml"));
}

#[test]
fn gen_without_a_language_is_a_usage_error() {
    let dir = project("gen_no_lang", "contracts:\n  - ./contracts\n");
    let o = weft(&["gen"], &dir);
    assert_eq!(code(&o), 2);
    assert!(text(&o).contains("no language"));
    // The flag can supply it.
    assert_eq!(code(&weft(&["gen", "--lang", "ts"], &dir)), 0);
}

#[test]
fn gen_for_a_language_that_is_not_built_yet_says_so() {
    let dir = project("gen_go", YAML);
    let o = weft(&["gen", "--lang", "go"], &dir);
    assert_eq!(code(&o), 2);
    assert!(text(&o).contains("does not generate go yet"));
    let o = weft(&["gen", "--lang", "klingon"], &dir);
    assert_eq!(code(&o), 2);
    assert!(text(&o).contains("unknown language"));
}

#[test]
fn gen_watch_and_git_sources_say_they_are_not_available() {
    let dir = project("gen_not_yet", YAML);
    assert_eq!(code(&weft(&["gen", "--watch"], &dir)), 2);
    let dir = project("gen_git", "language: typescript\ncontracts:\n  - git: github.com/larder/contracts\n    packages:\n      payments: ^1.0\n");
    let o = weft(&["gen"], &dir);
    assert_eq!(code(&o), 2);
    assert!(text(&o).contains("git"));
}

#[test]
fn gen_with_two_packages_in_one_folder() {
    let dir = project("gen_two", YAML);
    fs::write(
        dir.join("contracts/payments.weft"),
        "package payments\nimport orders\ntype Charge { order: orders.Order }\n",
    )
    .unwrap();
    assert_eq!(code(&weft(&["gen"], &dir)), 0);
    let out = fs::read_to_string(dir.join("src/weft.gen.ts")).unwrap();
    assert!(
        out.contains("export namespace orders {") && out.contains("export namespace payments {")
    );
}

// ---- weft check ----

fn git(dir: &PathBuf, args: &[&str]) {
    let o = Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

const V1: &str = "package orders @version(\"1.0.0\")\ntype Order { id: uuid note: string }\nevent order.placed: Order\n";

/// A project with one contract, committed and tagged `v1`.
fn tagged_project(name: &str, contract: &str) -> PathBuf {
    let dir = scratch(name);
    fs::write(dir.join("weft.yaml"), "language: typescript\n").unwrap();
    fs::create_dir_all(dir.join("contracts")).unwrap();
    fs::write(dir.join("contracts/orders.weft"), contract).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    git(&dir, &["tag", "v1"]);
    dir
}

fn write_contract(dir: &Path, text: &str) {
    fs::write(dir.join("contracts/orders.weft"), text).unwrap();
}

#[test]
fn check_with_no_changes_passes() {
    let dir = tagged_project("check_same", V1);
    let o = weft(&["check"], &dir);
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert!(
        text(&o).contains("no changes") && text(&o).contains("git v1"),
        "{}",
        text(&o)
    );
}

#[test]
fn check_fails_on_a_breaking_change_without_a_major_bump() {
    let dir = tagged_project("check_breaking", V1);
    write_contract(&dir, &V1.replace(" note: string", ""));
    let o = weft(&["check"], &dir);
    assert_eq!(code(&o), 1, "{}", text(&o));
    let t = text(&o);
    assert!(t.contains("removed field `note`"), "{t}");
    assert!(t.contains("hint: mark it `@deprecated` first"), "{t}");
    assert!(t.contains("need a major version bump"), "{t}");
}

#[test]
fn check_passes_when_a_major_bump_covers_the_breaking_change() {
    let dir = tagged_project("check_major", V1);
    write_contract(
        &dir,
        &V1.replace(" note: string", "").replace("1.0.0", "2.0.0"),
    );
    let o = weft(&["check"], &dir);
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert!(
        text(&o).contains("covers the breaking changes"),
        "{}",
        text(&o)
    );
}

#[test]
fn check_with_only_safe_changes_passes_and_notes_the_missing_bump() {
    let dir = tagged_project("check_safe", V1);
    write_contract(
        &dir,
        &V1.replace("note: string", "note: string extra?: int"),
    );
    let o = weft(&["check"], &dir);
    assert_eq!(code(&o), 0, "{}", text(&o));
    let t = text(&o);
    assert!(
        t.contains("added optional field `extra`") && t.contains("a minor version bump"),
        "{t}"
    );
}

#[test]
fn check_baseline_flag_picks_the_ref() {
    let dir = tagged_project("check_ref", V1);
    // A second commit with the breaking change, which becomes the new baseline.
    write_contract(
        &dir,
        &V1.replace(" note: string", "").replace("1.0.0", "2.0.0"),
    );
    git(&dir, &["commit", "-q", "-am", "second"]);
    // Against the latest tag (v1) the working tree is a covered breaking change; against HEAD it is unchanged.
    assert_eq!(code(&weft(&["check"], &dir)), 0);
    let o = weft(&["check", "--baseline", "HEAD"], &dir);
    assert_eq!(code(&o), 0);
    assert!(text(&o).contains("no changes"));
    // Break it further, still at 2.0.0: now HEAD as the baseline fails.
    write_contract(&dir, "package orders @version(\"2.0.0\")\ntype Order { id: string }\nevent order.placed: Order\n");
    assert_eq!(code(&weft(&["check", "--baseline", "HEAD"], &dir)), 1);
}

#[test]
fn check_reads_the_baseline_without_touching_the_working_tree() {
    let dir = tagged_project("check_untouched", V1);
    write_contract(&dir, &V1.replace(" note: string", ""));
    weft(&["check"], &dir);
    assert!(!fs::read_to_string(dir.join("contracts/orders.weft"))
        .unwrap()
        .contains("note"));
}

#[test]
fn check_without_a_tag_says_how_to_choose_a_baseline() {
    let dir = scratch("check_no_tag");
    fs::write(dir.join("weft.yaml"), "language: typescript\n").unwrap();
    fs::create_dir_all(dir.join("contracts")).unwrap();
    fs::write(dir.join("contracts/orders.weft"), V1).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let o = weft(&["check"], &dir);
    assert_eq!(code(&o), 2);
    assert!(
        text(&o).contains("no git tag") && text(&o).contains("--baseline"),
        "{}",
        text(&o)
    );
}

#[test]
fn check_with_an_unknown_ref_is_a_usage_error() {
    let dir = tagged_project("check_bad_ref", V1);
    let o = weft(&["check", "--baseline", "nope"], &dir);
    assert_eq!(code(&o), 2);
    assert!(text(&o).contains("unknown git ref `nope`"));
}

#[test]
fn check_treats_contracts_that_did_not_exist_at_the_baseline_as_new() {
    let dir = scratch("check_new_folder");
    fs::write(dir.join("weft.yaml"), "language: typescript\n").unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    git(&dir, &["tag", "v1"]);
    fs::create_dir_all(dir.join("contracts")).unwrap();
    write_contract(&dir, V1);
    let o = weft(&["check"], &dir);
    assert_eq!(code(&o), 0, "{}", text(&o));
    assert!(text(&o).contains("package `orders` is new"), "{}", text(&o));
}

#[test]
fn check_baseline_dir_compares_with_a_folder() {
    let dir = project("check_dir", "language: typescript\n");
    let older = dir.join("old");
    fs::create_dir_all(&older).unwrap();
    fs::write(older.join("orders.weft"), V1).unwrap();
    write_contract(&dir, &V1.replace(" note: string", ""));
    let o = weft(&["check", "--baseline-dir", "old"], &dir);
    assert_eq!(code(&o), 1, "{}", text(&o));
    assert!(text(&o).contains("removed field `note`"));
    write_contract(
        &dir,
        &V1.replace(" note: string", "").replace("1.0.0", "2.0.0"),
    );
    assert_eq!(code(&weft(&["check", "--baseline-dir", "old"], &dir)), 0);
}

#[test]
fn check_reports_contract_errors_in_the_current_contracts_with_exit_3() {
    let dir = tagged_project("check_contract_error", V1);
    write_contract(&dir, "package orders\ntype Order { id: Nope }\n");
    let o = weft(&["check"], &dir);
    assert_eq!(code(&o), 3);
    assert!(text(&o).contains("unknown type `Nope`"));
}

#[test]
fn check_reports_a_broken_baseline_with_exit_3() {
    let dir = tagged_project(
        "check_bad_baseline",
        "package orders\ntype Order { id: Nope }\n",
    );
    write_contract(&dir, V1);
    let o = weft(&["check"], &dir);
    assert_eq!(code(&o), 3);
    assert!(
        text(&o).contains("the baseline (git v1) has contract errors"),
        "{}",
        text(&o)
    );
}

#[test]
fn check_conflicting_baseline_flags_are_a_usage_error() {
    let dir = tagged_project("check_both", V1);
    assert_eq!(
        code(&weft(
            &["check", "--baseline", "v1", "--baseline-dir", "x"],
            &dir
        )),
        2
    );
}

#[test]
fn check_works_from_a_subfolder() {
    let dir = tagged_project("check_sub", V1);
    assert_eq!(code(&weft(&["check"], &dir.join("contracts"))), 0);
}

// ---- weft init ----

/// An empty folder with a given name, so the default service name is predictable.
fn empty(name: &str) -> PathBuf {
    scratch(name)
}

#[test]
fn init_yes_sets_up_an_empty_folder() {
    let dir = empty("init_basic");
    let o = weft(&["init", "--yes"], &dir);
    assert_eq!(code(&o), 0, "{}", text(&o));
    let t = text(&o);
    assert!(
        t.contains("No project detected here") && t.contains("Wrote weft.yaml"),
        "{t}"
    );
    assert!(t.contains("Wrote src/weft.gen.ts"), "{t}");
    let yaml = fs::read_to_string(dir.join("weft.yaml")).unwrap();
    assert!(
        yaml.contains("service: init_basic") && yaml.contains("language: typescript"),
        "{yaml}"
    );
    assert!(
        yaml.contains("http:\n    library: express\n    port: 3000"),
        "{yaml}"
    );
    let contract = fs::read_to_string(dir.join("contracts/init_basic.weft")).unwrap();
    assert!(contract.contains("package init_basic") && contract.contains("http {"));
    assert!(fs::read_to_string(dir.join("src/weft.gen.ts"))
        .unwrap()
        .contains("export namespace init_basic"));
}

#[test]
fn what_init_writes_passes_every_other_command() {
    let dir = empty("init_then_everything");
    assert_eq!(
        code(&weft(
            &["init", "--yes", "--name", "orders", "--http", "--kafka", "--gql", "--ws"],
            &dir
        )),
        0
    );
    assert_eq!(
        code(&weft(&["gen", "--check"], &dir)),
        0,
        "generated file is current"
    );
    assert_eq!(
        code(&weft(&["fmt", "--check"], &dir)),
        0,
        "example contract is already formatted"
    );
    // And it can be checked against a baseline, once there is one.
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "init"]);
    git(&dir, &["tag", "v1"]);
    assert_eq!(code(&weft(&["check"], &dir)), 0);
}

#[test]
fn init_flags_choose_transports_and_libraries() {
    let dir = empty("init_flags");
    let o = weft(
        &[
            "init",
            "--yes",
            "--name",
            "orders",
            "--http",
            "fastify",
            "--kafka",
            "--sse",
            "--redis",
            "node-redis",
        ],
        &dir,
    );
    assert_eq!(code(&o), 0, "{}", text(&o));
    let yaml = fs::read_to_string(dir.join("weft.yaml")).unwrap();
    for expected in [
        "http:\n    library: fastify",
        "kafka:\n    library: kafkajs\n    brokers: ${KAFKA_BROKERS}\n    group: orders",
        "sse:\n    path: /events",
        "redis:\n    library: node-redis",
    ] {
        assert!(yaml.contains(expected), "missing {expected:?} in\n{yaml}");
    }
    assert!(!yaml.contains("gql"), "{yaml}");
    let contract = fs::read_to_string(dir.join("contracts/orders.weft")).unwrap();
    assert!(contract.contains("event item.created") && contract.contains("socket updates"));
}

#[test]
fn init_detects_the_project_and_package_manager() {
    let dir = empty("init_detect");
    fs::write(dir.join("package.json"), "{}").unwrap();
    fs::write(dir.join("pnpm-lock.yaml"), "").unwrap();
    let o = weft(&["init", "--yes"], &dir);
    assert_eq!(code(&o), 0);
    assert!(
        text(&o).contains("Detected: TypeScript (pnpm)"),
        "{}",
        text(&o)
    );
}

#[test]
fn init_uses_the_given_contracts_folder() {
    let dir = empty("init_contracts");
    assert_eq!(
        code(&weft(
            &[
                "init",
                "--yes",
                "--name",
                "api",
                "--contracts",
                "./spec/contracts"
            ],
            &dir
        )),
        0
    );
    assert!(dir.join("spec/contracts/api.weft").exists());
    assert!(fs::read_to_string(dir.join("weft.yaml"))
        .unwrap()
        .contains("- ./spec/contracts"));
    assert_eq!(code(&weft(&["gen", "--check"], &dir)), 0);
}

#[test]
fn init_makes_a_service_name_from_the_folder() {
    let dir = scratch("My Service-2");
    assert_eq!(code(&weft(&["init", "--yes"], &dir)), 0);
    assert!(fs::read_to_string(dir.join("weft.yaml"))
        .unwrap()
        .contains("service: my_service_2"));
}

#[test]
fn init_never_overwrites() {
    let dir = empty("init_twice");
    assert_eq!(code(&weft(&["init", "--yes", "--name", "orders"], &dir)), 0);
    let before = fs::read_to_string(dir.join("weft.yaml")).unwrap();
    fs::write(dir.join("weft.yaml"), format!("{before}# mine\n")).unwrap();
    let o = weft(&["init", "--yes", "--name", "orders"], &dir);
    assert_eq!(code(&o), 2);
    assert!(text(&o).contains("already exists"), "{}", text(&o));
    assert!(fs::read_to_string(dir.join("weft.yaml"))
        .unwrap()
        .ends_with("# mine\n"));

    // An existing contract or generated file also stops it, before anything is written.
    let dir = empty("init_existing_contract");
    fs::create_dir_all(dir.join("contracts")).unwrap();
    fs::write(dir.join("contracts/orders.weft"), "package mine\n").unwrap();
    let o = weft(&["init", "--yes", "--name", "orders"], &dir);
    assert_eq!(code(&o), 2);
    assert!(
        !dir.join("weft.yaml").exists(),
        "nothing is written when it stops"
    );
    assert_eq!(
        fs::read_to_string(dir.join("contracts/orders.weft")).unwrap(),
        "package mine\n"
    );
}

#[test]
fn init_rejects_bad_input_and_writes_nothing() {
    let cases: [(&[&str], &str); 8] = [
        (&["--name", "9lives"], "not a valid service name"),
        (&["--name", "My-Service"], "not a valid service name"),
        (&["--name", "weft"], "reserved"),
        (&["--name", "class"], "cannot be used"),
        (
            &["--http", "nope"],
            "is not a HTTP library. Choose one of: express, fastify, hono, nestjs",
        ),
        (&["--sse", "x"], "has no library to choose"),
        (&["--lang", "go"], "does not set up go yet"),
        (&["--contracts", "x.weft"], "is a folder"),
    ];
    for (i, (flags, needle)) in cases.iter().enumerate() {
        let dir = empty(&format!("init_bad_{i}"));
        let mut args = vec!["init", "--yes"];
        args.extend(*flags);
        let o = weft(&args, &dir);
        assert_eq!(code(&o), 2, "{flags:?}: {}", text(&o));
        assert!(
            text(&o).contains(needle),
            "{flags:?}: wanted {needle:?} in {}",
            text(&o)
        );
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            0,
            "{flags:?} left files behind"
        );
    }
}

#[test]
fn init_without_a_terminal_and_without_yes_says_what_to_do() {
    let dir = empty("init_no_tty");
    let o = weft(&["init"], &dir);
    assert_eq!(code(&o), 2);
    assert!(
        text(&o).contains("needs a terminal") && text(&o).contains("--yes"),
        "{}",
        text(&o)
    );
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
}

#[test]
fn init_every_transport_flag_works() {
    let dir = empty("init_all");
    let o = weft(
        &[
            "init", "--yes", "--name", "all", "--http", "--gql", "--grpc", "--rpc", "--ws",
            "--sse", "--kafka", "--mq", "--nats", "--redis", "--cloud", "--mqtt", "--pulsar",
        ],
        &dir,
    );
    assert_eq!(code(&o), 0, "{}", text(&o));
    let yaml = fs::read_to_string(dir.join("weft.yaml")).unwrap();
    for t in [
        "http", "gql", "grpc", "rpc", "ws", "sse", "kafka", "mq", "nats", "redis", "cloud", "mqtt",
        "pulsar",
    ] {
        assert!(
            yaml.contains(&format!("  {t}:\n")),
            "{t} missing in\n{yaml}"
        );
    }
    assert_eq!(code(&weft(&["gen", "--check"], &dir)), 0);
}
