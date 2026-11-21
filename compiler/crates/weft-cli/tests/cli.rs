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
