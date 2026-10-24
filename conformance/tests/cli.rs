use std::process::Command;

#[test]
fn empty_vector_directory_is_not_a_successful_conformance_run() {
    let dir = std::env::temp_dir().join(format!("lmcode-empty-vectors-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_dsl-conformance")).arg(&dir).output();
    std::fs::remove_dir(&dir).unwrap();
    let output = output.unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("no vectors in"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("PASS"));
}
