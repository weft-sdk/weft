use std::fs;
use std::path::PathBuf;
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
