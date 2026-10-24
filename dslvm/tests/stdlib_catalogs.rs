use dslvm::{stdlib, Registry, Value};
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
    let mut registry = Registry::without_std();
    stdlib::register_collections(&mut registry).unwrap();
    assert_catalog("stdlib/collections.schema-fragment.json", "rust_collections_catalog", registry);
}

#[test]
fn testing_fragment_matches_registry_and_compiler() {
    let mut registry = Registry::without_std();
    stdlib::register_testing(&mut registry, None).unwrap();
    assert_catalog("stdlib/testing.schema-fragment.json", "rust_testing_catalog", registry);
}

#[test]
fn registry_defaults_to_std_and_supports_opt_out() {
    let mut default = Registry::new();
    assert_eq!(default.signatures().len(), 69);
    assert_eq!(default.std_signatures().len(), 69);
    assert!(default.host_signatures().is_empty());
    assert!(default.function("str_len", &["string"], "int", |_, _| Ok(Value::Int(0))).is_err());
    let empty = Registry::without_std();
    assert!(empty.signatures().is_empty());
    assert!(empty.std_signatures().is_empty());
    assert!(empty.host_signatures().is_empty());
}

#[test]
fn explicit_std_registration_is_idempotent_and_schema_normalized() {
    let mut registry = Registry::without_std();
    stdlib::register_std(&mut registry).unwrap();
    let signatures = registry.signatures();
    stdlib::register_std(&mut registry).unwrap();
    assert_eq!(registry.signatures(), signatures);
    registry.check_schema_signatures(&[]).unwrap();
    registry.check_schema_signatures(&signatures).unwrap();
    assert!(registry.matches_schema_signatures(&[]));
    assert!(!registry.matches_schema_signatures(&["fn wrong() -> void".into()]));
    let mut host = Registry::without_std();
    host.function("host_only", &[], "void", |_, _| Ok(Value::None)).unwrap();
    assert!(host.check_schema_signatures(&["fn host_only() -> void".into()]).is_ok());
}

#[test]
fn canonical_std_schema_is_pinned_and_matches_default_registry() {
    let root = root();
    let schema = fs::read(root.join("std/lmcode-std.schema.json")).unwrap();
    let lock = fs::read_to_string(root.join("std.lock")).unwrap();
    let want = lock.lines().find_map(|line| line.strip_prefix("sha256=")).unwrap();
    assert_eq!(dslvm::hex(&dslvm::sha256(&schema)), want);
    let bin = std::env::var("LMSCRIPT_BIN").unwrap_or_else(|_| "lmscript".into());
    let configured = std::env::var_os("LMSCRIPT_BIN").is_some();
    let output = Command::new(bin).arg("--dsl-signatures").arg(root.join("std/lmcode-std.schema.json")).output();
    let output = match output {
        Ok(output) => output,
        Err(_error) if !configured => { eprintln!("lmscript not found; canonical std parity skipped"); return; }
        Err(error) => panic!("configured LMSCRIPT_BIN failed to launch: {}", error),
    };
    assert!(output.status.success(), "lmscript canonical std signatures failed: {}", String::from_utf8_lossy(&output.stderr));
    let compiler: Vec<String> = String::from_utf8(output.stdout).unwrap().lines().map(str::to_owned).collect();
    assert_eq!(compiler.len(), 69);
    assert_eq!(compiler, Registry::new().signatures());
}
