//! Conformance runner: implements the host stub described in conformance/vectors/manifest.json ("host") and checks every
//! NAME.dslb against NAME.expect (line 1: trace | runtime_error | load_error; then the expected lines or the error substring).
//! `cargo test -p dsl-conformance` runs all vectors; `cargo run -p dsl-conformance -- [VECTOR_DIR]` prints a report.
use dslvm::{format_g15, Context, Error, Program, Registry, Value};
use std::path::{Path, PathBuf};

pub struct Thing {
    pub id: i64,
}
pub struct IntList {
    pub values: Vec<i64>,
}
pub struct Trace(pub Vec<String>);

/// The manifest's host stub: `t` (Trace) records calls; `root` is Thing 0; thing/twice/half/yes/fail.
// Decode the language-neutral catalog fixture adapter without UTF-8 conversion.
fn fixture_bytes(hex: &[u8]) -> Result<Vec<u8>, Error> {
    fn digit(byte: u8) -> Result<u8, Error> {
        match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            _ => Err(Error::host("bytes: expected lowercase hexadecimal pairs")),
        }
    }
    if !hex.len().is_multiple_of(2) {
        return Err(Error::host("bytes: expected lowercase hexadecimal pairs"));
    }
    hex.chunks_exact(2)
        .map(|pair| Ok(digit(pair[0])? * 16 + digit(pair[1])?))
        .collect()
}

pub fn stub_registry() -> Registry {
    let mut reg = Registry::new();
    dslvm::stdlib::register_string(&mut reg).unwrap();
    dslvm::stdlib::register_math(&mut reg).unwrap();
    reg.function("bytes", &["string"], "string", |_, a| {
        Ok(Value::string(fixture_bytes(a[0].as_bytes()?)?))
    })
    .unwrap();
    reg.method("Trace", "hex", &["string"], "void", |ctx, _, a| {
        trace(ctx, format!("hex {}", dslvm::hex(a[0].as_bytes()?)))
    })
    .unwrap();
    reg.context("t", "Trace")
        .unwrap()
        .context("root", "Thing")
        .unwrap()
        .context("v", "Var")
        .unwrap()
        .context("xs", "IntList")
        .unwrap();
    fn trace(ctx: &mut Context, line: String) -> Result<Value, Error> {
        let t = ctx.lookup("t").ok_or_else(|| Error::host("no t"))?;
        ctx.get_mut::<Trace>(t)?.0.push(line);
        Ok(Value::None)
    }
    reg.method("Trace", "int", &["int"], "void", |ctx, _, a| {
        trace(ctx, format!("int {}", a[0].as_int()?))
    })
    .unwrap();
    reg.method("Trace", "float", &["float"], "void", |ctx, _, a| {
        trace(ctx, format!("float {}", format_g15(a[0].as_float()?)))
    })
    .unwrap();
    reg.method("Trace", "str", &["string"], "void", |ctx, _, a| {
        trace(ctx, format!("str {}", a[0].as_str()?))
    })
    .unwrap();
    reg.method("Trace", "bool", &["bool"], "void", |ctx, _, a| {
        trace(ctx, format!("bool {}", a[0].as_bool()?))
    })
    .unwrap();
    reg.method("Trace", "thing", &["Thing"], "void", |ctx, _, a| {
        let id = ctx.get::<Thing>(a[0].as_obj()?)?.id;
        trace(ctx, format!("thing {}", id))
    })
    .unwrap();
    reg.method("Thing", "len", &[], "int", |ctx, this, _| {
        let n = ctx.get::<Thing>(this)?.id;
        trace(ctx, format!("len {}", n))?;
        Ok(Value::Int(n))
    })
    .unwrap();
    reg.method("Thing", "at", &["int"], "Thing", |ctx, _, a| {
        let id = a[0].as_int()?;
        trace(ctx, format!("at {}", id))?;
        let r = ctx.insert("Thing", Thing { id });
        Ok(Value::object("Thing", r))
    })
    .unwrap();
    reg.method("IntList", "len", &[], "int", |ctx, this, _| {
        Ok(Value::Int(ctx.get::<IntList>(this)?.values.len() as i64))
    })
    .unwrap();
    reg.method("IntList", "at", &["int"], "int", |ctx, this, a| {
        let i = a[0].as_int()?;
        let value = ctx
            .get::<IntList>(this)?
            .values
            .get(usize::try_from(i).map_err(|_| Error::host("IntList index out of range"))?)
            .copied()
            .ok_or_else(|| Error::host("IntList index out of range"))?;
        Ok(Value::Int(value))
    })
    .unwrap();
    reg.method("Thing", "id", &[], "int", |ctx, this, _| {
        Ok(Value::Int(ctx.get::<Thing>(this)?.id))
    })
    .unwrap();
    reg.method("Thing", "next", &[], "Thing", |ctx, this, _| {
        let id = ctx.get::<Thing>(this)?.id;
        let r = ctx.insert("Thing", Thing { id: id + 1 });
        Ok(Value::object("Thing", r))
    })
    .unwrap();
    reg.method("Thing", "name", &[], "string", |ctx, this, _| {
        Ok(Value::string(format!(
            "thing-{}",
            ctx.get::<Thing>(this)?.id
        )))
    })
    .unwrap();
    reg.method("Thing", "scaled", &["float"], "float", |ctx, this, a| {
        Ok(Value::Float(
            ctx.get::<Thing>(this)?.id as f64 * a[0].as_float()?,
        ))
    })
    .unwrap();
    reg.method("Trace", "int", &["float"], "void", |ctx, _, a| {
        trace(ctx, format!("float {}", format_g15(a[0].as_float()?)))
    })
    .unwrap();
    reg.method("Trace", "status", &[], "Status", |_, _, _| {
        Ok(Value::Str("OPTIMAL".into()))
    })
    .unwrap();
    reg.method("Thing", "plus", &["Thing"], "Thing", |ctx, this, a| {
        let id = ctx
            .get::<Thing>(this)?
            .id
            .wrapping_add(ctx.get::<Thing>(a[0].as_obj()?)?.id);
        let r = ctx.insert("Thing", Thing { id });
        Ok(Value::object("Thing", r))
    })
    .unwrap();
    reg.method("Thing", "minus", &["Thing"], "Thing", |ctx, this, a| {
        let id = ctx
            .get::<Thing>(this)?
            .id
            .wrapping_sub(ctx.get::<Thing>(a[0].as_obj()?)?.id);
        let r = ctx.insert("Thing", Thing { id });
        Ok(Value::object("Thing", r))
    })
    .unwrap();
    reg.method("Thing", "times", &["int"], "Thing", |ctx, this, a| {
        let id = ctx.get::<Thing>(this)?.id.wrapping_mul(a[0].as_int()?);
        let r = ctx.insert("Thing", Thing { id });
        Ok(Value::object("Thing", r))
    })
    .unwrap();
    reg.method("Thing", "neg", &[], "Thing", |ctx, this, _| {
        let id = ctx.get::<Thing>(this)?.id.wrapping_neg();
        let r = ctx.insert("Thing", Thing { id });
        Ok(Value::object("Thing", r))
    })
    .unwrap();
    reg.method("Thing", "rminus", &["int"], "Thing", |ctx, this, a| {
        let id = a[0].as_int()?.wrapping_sub(ctx.get::<Thing>(this)?.id);
        let r = ctx.insert("Thing", Thing { id });
        Ok(Value::object("Thing", r))
    })
    .unwrap();
    reg.method("Thing", "set", &["int"], "Thing", |ctx, this, a| {
        ctx.get_mut::<Thing>(this)?.id = a[0].as_int()?;
        Ok(Value::object("Thing", this))
    })
    .unwrap();
    reg.method("Var", "id", &[], "int", |ctx, this, _| {
        Ok(Value::Int(ctx.get::<Thing>(this)?.id))
    })
    .unwrap();
    reg.method("Var", "expr", &[], "Thing", |ctx, this, _| {
        let id = ctx.get::<Thing>(this)?.id;
        trace(ctx, format!("expr {}", id))?;
        let r = ctx.insert("Thing", Thing { id });
        Ok(Value::object("Thing", r))
    })
    .unwrap();
    reg.function("var", &["int"], "Var", |ctx, a| {
        let id = a[0].as_int()?;
        trace(ctx, format!("var {}", id))?;
        let r = ctx.insert("Var", Thing { id });
        Ok(Value::object("Var", r))
    })
    .unwrap();
    reg.function("twice", &["float"], "float", |_, a| {
        Ok(Value::Float(a[0].as_float()? * 2.0))
    })
    .unwrap();
    reg.function("thing", &["int"], "Thing", |ctx, a| {
        let r = ctx.insert("Thing", Thing { id: a[0].as_int()? });
        Ok(Value::object("Thing", r))
    })
    .unwrap();
    reg.function("twice", &["int"], "int", |_, a| {
        Ok(Value::Int(a[0].as_int()? * 2))
    })
    .unwrap();
    reg.function("bound", &["int"], "int", |ctx, a| {
        let value = a[0].as_int()?;
        trace(ctx, format!("bound {}", value))?;
        Ok(Value::Int(value))
    })
    .unwrap();
    reg.function("half", &["float"], "float", |_, a| {
        Ok(Value::Float(a[0].as_float()? / 2.0))
    })
    .unwrap();
    reg.function("yes", &[], "bool", |ctx, _| {
        trace(ctx, "yes".into())?;
        Ok(Value::Bool(true))
    })
    .unwrap();
    reg.function("fail", &["string"], "void", |_, a| {
        Err(Error::host(a[0].as_str()?.to_string()))
    })
    .unwrap();
    reg.function("bad_int", &[], "int", |_, _| Ok(Value::Str("x".into())))
        .unwrap();
    reg
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Trace(Vec<String>),
    RuntimeError(Error),
    LoadError(String),
}

/// Runs one vector: load (phase 1) then run (phase 2) with a fresh context.
pub fn run_vector(bytecode: &[u8], schema: &[u8]) -> Outcome {
    let reg = stub_registry();
    let program = match Program::load(bytecode, schema, &reg) {
        Ok(p) => p,
        Err(e) => return Outcome::LoadError(e.message),
    };
    let mut ctx = Context::new();
    let t = ctx.set_object("t", "Trace", Trace(Vec::new()));
    ctx.set_object("root", "Thing", Thing { id: 0 });
    ctx.set_object("v", "Var", Thing { id: 0 });
    ctx.set_object(
        "xs",
        "IntList",
        IntList {
            values: vec![0, 1, 2, 3],
        },
    );
    match program.run(&mut ctx, &reg) {
        Ok(()) => Outcome::Trace(ctx.take::<Trace>(t).map(|t| t.0).unwrap_or_default()),
        Err(e) => Outcome::RuntimeError(e),
    }
}

pub struct Expect {
    pub kind: String,
    pub lines: Vec<String>,
}
pub fn read_expect(path: &Path) -> Expect {
    let text = std::fs::read_to_string(path).expect("expect file");
    let mut it = text.lines();
    let kind = it.next().unwrap_or("").to_string();
    Expect {
        kind,
        lines: it.map(|l| l.replace("\\n", "\n")).collect(),
    }
}
/// A trace line beginning with `~ ` compares its final numeric field within the shared math tolerance.
/// The text before that field (for example `float`) must still match exactly.
fn trace_line_matches(expected: &str, actual: &str) -> bool {
    let Some(expected) = expected.strip_prefix("~ ") else {
        return expected == actual;
    };
    let Some((eprefix, enumber)) = expected.rsplit_once(' ') else {
        return false;
    };
    let Some((aprefix, anumber)) = actual.rsplit_once(' ') else {
        return false;
    };
    let (Ok(e), Ok(a)) = (enumber.parse::<f64>(), anumber.parse::<f64>()) else {
        return false;
    };
    eprefix == aprefix
        && e.is_finite()
        && a.is_finite()
        && (a - e).abs() <= 1e-12 * e.abs().max(1e-300)
}

fn trace_matches(expected: &[String], actual: &[String]) -> bool {
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual)
            .all(|(e, a)| trace_line_matches(e, a))
}
/// The manifest names the schema file per vector (`"schema": "schema-extra.json"` and the like); default schema.json.
pub fn schema_for(dir: &Path, name: &str) -> PathBuf {
    let manifest = std::fs::read_to_string(dir.join("manifest.json")).unwrap_or_default();
    if let Some(p) = manifest.find(&format!("\"name\": \"{}\"", name)) {
        let rest = &manifest[p..];
        let end = rest[1..]
            .find("\"name\": \"")
            .map(|e| e + 1)
            .unwrap_or(rest.len());
        if let Some(q) = rest[..end].find("\"schema\": \"") {
            let v = &rest[q + 11..end];
            if let Some(e) = v.find('"') {
                return dir.join(&v[..e]);
            }
        }
    }
    dir.join("schema.json")
}
pub fn vector_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("vector dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".expect"))
        .map(|n| n.trim_end_matches(".expect").to_string())
        .collect();
    names.sort();
    names
}
/// Checks one vector; returns an error description when it does not match.
pub fn check_vector(dir: &Path, name: &str) -> Result<String, String> {
    let expect = read_expect(&dir.join(format!("{}.expect", name)));
    let bytecode = std::fs::read(dir.join(format!("{}.dslb", name))).map_err(|e| e.to_string())?;
    let schema = std::fs::read(schema_for(dir, name)).map_err(|e| e.to_string())?;
    let outcome = run_vector(&bytecode, &schema);
    let ok = match (expect.kind.as_str(), &outcome) {
        ("trace", Outcome::Trace(lines)) => trace_matches(&expect.lines, lines),
        ("runtime_error", Outcome::RuntimeError(e)) => e.message.contains(&expect.lines[0]),
        ("runtime_error_at", Outcome::RuntimeError(e)) => {
            let line = expect
                .lines
                .get(1)
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|v| *v != 0);
            let column = expect
                .lines
                .get(2)
                .and_then(|s| s.parse::<u16>().ok())
                .filter(|v| *v != 0);
            expect.lines.len() >= 4
                && e.code == expect.lines[0]
                && e.line == line
                && e.column == column
                && e.message.contains(&expect.lines[3])
        }
        ("load_error", Outcome::LoadError(m)) => m.contains(&expect.lines[0]),
        _ => false,
    };
    if ok {
        Ok(expect.kind)
    } else {
        Err(format!(
            "{}: expected {} {:?}, got {:?}",
            name, expect.kind, expect.lines, outcome
        ))
    }
}
pub fn default_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("vectors")
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_dir);
    let names = vector_names(&dir);
    if names.is_empty() {
        eprintln!("no vectors in {}", dir.display());
        std::process::exit(2);
    }
    let mut failed = 0;
    for name in &names {
        match check_vector(&dir, name) {
            Ok(kind) => println!("ok - {} ({})", name, kind),
            Err(e) => {
                failed += 1;
                println!("FAIL - {}", e);
            }
        }
    }
    println!(
        "{} {}/{} vectors",
        if failed > 0 { "FAIL" } else { "PASS" },
        names.len() - failed,
        names.len()
    );
    if failed > 0 {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_fixture_bytes_preserve_binary_values() {
        assert_eq!(fixture_bytes(b"ff00c3a9").unwrap(), [0xff, 0, 0xc3, 0xa9]);
        assert_eq!(fixture_bytes(b"").unwrap(), []);
        for bad in [b"0".as_slice(), b"gg", b"FF", b"\xff\x00"] {
            let error = fixture_bytes(bad).unwrap_err();
            assert_eq!(error.code, "RUN_HOST");
            assert_eq!(error.message, "bytes: expected lowercase hexadecimal pairs");
        }
    }
    #[test]
    fn pinned_vectors_match_the_lock() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        for (dir_name, lock_name) in [
            ("conformance/vectors", "vectors.lock"),
            ("conformance/stdlib-vectors", "stdlib-vectors.lock"),
        ] {
            let lock = std::fs::read_to_string(root.join(lock_name)).expect("vector lock");
            let want = lock
                .lines()
                .find_map(|l| l.strip_prefix("sha256="))
                .expect("sha256 line");
            let dir = root.join(dir_name);
            let mut names: Vec<String> = std::fs::read_dir(&dir)
                .expect("vectors dir")
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            assert!(!names.is_empty(), "{dir_name} is empty");
            let mut listing = String::new();
            for n in &names {
                listing.push_str(&format!(
                    "{}  ./{}\n",
                    dslvm::hex(&dslvm::sha256(&std::fs::read(dir.join(n)).unwrap())),
                    n
                ));
            }
            assert_eq!(
                dslvm::hex(&dslvm::sha256(listing.as_bytes())),
                want,
                "{dir_name} differs from {lock_name}: run scripts/sync-vectors.sh"
            );
        }
    }
    #[test]
    fn every_vector_gives_the_expected_result_class() {
        let dir = default_dir();
        let names = vector_names(&dir);
        assert!(names.len() >= 21, "{} vectors", names.len());
        let failures: Vec<String> = names
            .iter()
            .filter_map(|n| check_vector(&dir, n).err())
            .collect();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
    #[test]
    fn every_stdlib_vector_gives_the_expected_result_class() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib-vectors");
        let names = vector_names(&dir);
        assert!(!names.is_empty(), "stdlib vectors are empty");
        let failures: Vec<String> = names
            .iter()
            .filter_map(|n| check_vector(&dir, n).err())
            .collect();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
    #[test]
    fn a_wrong_expectation_is_detected() {
        let dir = default_dir();
        let bytecode = std::fs::read(dir.join("literals.dslb")).unwrap();
        let schema = std::fs::read(dir.join("schema.json")).unwrap();
        match run_vector(&bytecode, &schema) {
            Outcome::Trace(lines) => assert_ne!(lines, vec!["int 41".to_string()]),
            other => panic!("{:?}", other),
        }
    }
    #[test]
    fn approximate_trace_lines_use_prefix_and_relative_tolerance() {
        assert!(trace_line_matches("~ float 1.0", "float 1.0000000000005"));
        assert!(!trace_line_matches("~ float 1.0", "int 1.0"));
        assert!(!trace_line_matches("~ float 1.0", "float 1.00000000001"));
        assert!(!trace_line_matches("~ float nan", "float nan"));
        assert!(!trace_line_matches("~ float 1.0", "float inf"));
        assert!(!trace_line_matches("~ float 1.0", "float 1.0 extra"));
        assert!(trace_line_matches("float 1", "float 1"));
    }
}
