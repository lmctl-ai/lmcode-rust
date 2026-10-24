use dslvm::{stdlib, Registry};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..") }

fn compiler_signatures(fragment: &str, name: &str) -> Option<Vec<String>> {
    let bin = std::env::var("LMSCRIPT_BIN").unwrap_or_else(|_| "lmscript".into());
    let path = std::env::temp_dir().join(format!("lmcode-rust-{}-{}.schema.json", name, std::process::id()));
    let schema = format!(r#"{{"dsl.schema":2,"name":"{}","context":{}, {}}}"#, name, "{}", fragment.trim().trim_start_matches('{').trim_end_matches('}'));
    fs::write(&path, schema).unwrap();
    let result = Command::new(bin).arg("--dsl-signatures").arg(&path).output();
    let _ = fs::remove_file(&path);
    let output = result.ok()?;
    if !output.status.success() { panic!("lmscript --dsl-signatures failed: {}", String::from_utf8_lossy(&output.stderr)); }
    Some(String::from_utf8(output.stdout).unwrap().lines().map(str::to_owned).collect())
}

fn assert_catalog(path: &str, name: &str, registry: Registry) {
    let fragment = fs::read_to_string(root().join(path)).unwrap();
    let Some(compiler) = compiler_signatures(&fragment, name) else {
        eprintln!("lmscript not found; compiler parity skipped for {}", name);
        return;
    };
    assert_eq!(compiler, registry.signatures(), "compiler/registry parity for {}", name);
}

#[test]
fn collections_fragment_matches_registry_and_compiler() {
    let mut registry = Registry::new();
    stdlib::register_collections(&mut registry).unwrap();
    assert_catalog("stdlib/collections.schema-fragment.json", "rust_collections_catalog", registry);
}

#[test]
fn testing_fragment_matches_registry_and_compiler() {
    let mut registry = Registry::new();
    stdlib::register_testing(&mut registry, None).unwrap();
    assert_catalog("stdlib/testing.schema-fragment.json", "rust_testing_catalog", registry);
}
