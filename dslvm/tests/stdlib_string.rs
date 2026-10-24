//! The string catalog: behaviour of the functions and parity of the registry with the shipped schema fragment and the compiler.
use dslvm::{stdlib, Registry};
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..") }

fn registry() -> Registry { let mut r = Registry::new(); stdlib::register_string(&mut r).unwrap(); r }

#[test]
fn str_len_counts_bytes() {
    assert_eq!(stdlib::str_len(""), 0);
    assert_eq!(stdlib::str_len("abc"), 3);
    assert_eq!(stdlib::str_len("a\0b"), 3);
    assert_eq!(stdlib::str_len("é"), 2);
    assert_eq!(stdlib::str_len("日本"), 6);
}

#[test]
fn registry_signatures_match_fragment() {
    assert_eq!(registry().signatures(), vec!["fn str_len(string) -> int".to_string()]);
    let fragment = std::fs::read_to_string(root().join("stdlib/string.schema-fragment.json")).unwrap();
    assert!(fragment.contains("\"str_len\": [{ \"params\": [\"string\"], \"returns\": \"int\" }]"));
    let fixture = std::fs::read_to_string(root().join("dslvm/tests/fixtures/string.schema.json")).unwrap();
    assert!(fixture.contains("\"str_len\": [{ \"params\": [\"string\"], \"returns\": \"int\" }]"));
}

#[test]
fn compiler_signatures_match_registry_when_lmscript_is_available() {
    let schema = root().join("dslvm/tests/fixtures/string.schema.json");
    let Ok(out) = Command::new(std::env::var("LMSCRIPT_BIN").unwrap_or_else(|_| "lmscript".into())).arg("--dsl-signatures").arg(&schema).output() else {
        eprintln!("lmscript not found: compiler parity skipped"); return;
    };
    assert!(out.status.success(), "lmscript --dsl-signatures failed: {}", String::from_utf8_lossy(&out.stderr));
    let compiler: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect();
    assert_eq!(compiler, registry().signatures());
}
