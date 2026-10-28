//! Conformance runner: implements the host stub described in conformance/vectors/manifest.json ("host") and checks every
//! NAME.dslb against NAME.expect (line 1: trace | runtime_error | load_error; then the expected lines or the error substring).
//! `cargo test -p dsl-conformance` runs all vectors; `cargo run -p dsl-conformance -- [VECTOR_DIR]` prints a report.
use dslvm::{format_g15, Context, Error, Program, Registry, Value};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Thing { pub id: i64 }
pub struct IntList { pub values: Vec<i64> }
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
    if !hex.len().is_multiple_of(2) { return Err(Error::host("bytes: expected lowercase hexadecimal pairs")); }
    hex.chunks_exact(2).map(|pair| Ok(digit(pair[0])? * 16 + digit(pair[1])?)).collect()
}

pub fn stub_registry() -> Registry { stub_registry_mode(false) }

fn stub_registry_mode(std_catalog: bool) -> Registry {
    let mut reg = if std_catalog { Registry::new() } else { Registry::without_std() };
    reg.function("bytes", &["string"], "string", |_, a| Ok(Value::string(fixture_bytes(a[0].as_bytes()?)?))).unwrap();
    reg.method("Trace", "hex", &["string"], "void", |ctx, _, a| trace(ctx, format!("hex {}", dslvm::hex(a[0].as_bytes()?)))).unwrap();
    reg.context("t", "Trace").unwrap().context("root", "Thing").unwrap().context("v", "Var").unwrap().context("xs", "IntList").unwrap();
    fn trace(ctx: &mut Context, line: String) -> Result<Value, Error> { let t = ctx.lookup("t").ok_or_else(|| Error::host("no t"))?; ctx.get_mut::<Trace>(t)?.0.push(line); Ok(Value::None) }
    reg.method("Trace", "int", &["int"], "void", |ctx, _, a| trace(ctx, format!("int {}", a[0].as_int()?))).unwrap();
    reg.method("Trace", "float", &["float"], "void", |ctx, _, a| trace(ctx, format!("float {}", format_g15(a[0].as_float()?)))).unwrap();
    reg.method("Trace", "str", &["string"], "void", |ctx, _, a| trace(ctx, format!("str {}", a[0].as_str()?))).unwrap();
    reg.method("Trace", "bool", &["bool"], "void", |ctx, _, a| trace(ctx, format!("bool {}", a[0].as_bool()?))).unwrap();
    reg.method("Trace", "thing", &["Thing"], "void", |ctx, _, a| { let id = ctx.get::<Thing>(a[0].as_obj()?)?.id; trace(ctx, format!("thing {}", id)) }).unwrap();
    reg.method("Thing", "len", &[], "int", |ctx, this, _| { let n=ctx.get::<Thing>(this)?.id; trace(ctx,format!("len {}",n))?; Ok(Value::Int(n)) }).unwrap();
    reg.method("Thing", "at", &["int"], "Thing", |ctx, _, a| { let id=a[0].as_int()?; trace(ctx,format!("at {}",id))?; let r=ctx.insert("Thing",Thing{id}); Ok(Value::object("Thing",r)) }).unwrap();
    if !std_catalog {
        reg.method("IntList", "len", &[], "int", |ctx, this, _| Ok(Value::Int(ctx.get::<IntList>(this)?.values.len() as i64))).unwrap();
        reg.method("IntList", "at", &["int"], "int", |ctx, this, a| { let i = a[0].as_int()?; let value = ctx.get::<IntList>(this)?.values.get(usize::try_from(i).map_err(|_| Error::host("IntList index out of range"))?).copied().ok_or_else(|| Error::host("IntList index out of range"))?; Ok(Value::Int(value)) }).unwrap();
    }
    reg.method("Thing", "id", &[], "int", |ctx, this, _| Ok(Value::Int(ctx.get::<Thing>(this)?.id))).unwrap();
    reg.method("Thing", "next", &[], "Thing", |ctx, this, _| { let id = ctx.get::<Thing>(this)?.id; let r = ctx.insert("Thing", Thing { id: id + 1 }); Ok(Value::object("Thing", r)) }).unwrap();
    reg.method("Thing", "name", &[], "string", |ctx, this, _| Ok(Value::string(format!("thing-{}", ctx.get::<Thing>(this)?.id)))).unwrap();
    reg.method("Thing", "scaled", &["float"], "float", |ctx, this, a| Ok(Value::Float(ctx.get::<Thing>(this)?.id as f64 * a[0].as_float()?))).unwrap();
    reg.method("Trace", "int", &["float"], "void", |ctx, _, a| trace(ctx, format!("float {}", format_g15(a[0].as_float()?)))).unwrap();
    reg.method("Trace", "status", &[], "Status", |_, _, _| Ok(Value::Str("OPTIMAL".into()))).unwrap();
    reg.method("Thing", "plus", &["Thing"], "Thing", |ctx, this, a| { let id = ctx.get::<Thing>(this)?.id.wrapping_add(ctx.get::<Thing>(a[0].as_obj()?)?.id); let r = ctx.insert("Thing", Thing {id}); Ok(Value::object("Thing", r)) }).unwrap();
    reg.method("Thing", "minus", &["Thing"], "Thing", |ctx, this, a| { let id = ctx.get::<Thing>(this)?.id.wrapping_sub(ctx.get::<Thing>(a[0].as_obj()?)?.id); let r = ctx.insert("Thing", Thing {id}); Ok(Value::object("Thing", r)) }).unwrap();
    reg.method("Thing", "times", &["int"], "Thing", |ctx, this, a| { let id = ctx.get::<Thing>(this)?.id.wrapping_mul(a[0].as_int()?); let r = ctx.insert("Thing", Thing {id}); Ok(Value::object("Thing", r)) }).unwrap();
    reg.method("Thing", "neg", &[], "Thing", |ctx, this, _| { let id = ctx.get::<Thing>(this)?.id.wrapping_neg(); let r = ctx.insert("Thing", Thing {id}); Ok(Value::object("Thing", r)) }).unwrap();
    reg.method("Thing", "rminus", &["int"], "Thing", |ctx, this, a| { let id = a[0].as_int()?.wrapping_sub(ctx.get::<Thing>(this)?.id); let r = ctx.insert("Thing", Thing {id}); Ok(Value::object("Thing", r)) }).unwrap();
    reg.method("Thing", "set", &["int"], "Thing", |ctx, this, a| { ctx.get_mut::<Thing>(this)?.id = a[0].as_int()?; Ok(Value::object("Thing", this)) }).unwrap();
    reg.method("Var", "id", &[], "int", |ctx, this, _| Ok(Value::Int(ctx.get::<Thing>(this)?.id))).unwrap();
    reg.method("Var", "expr", &[], "Thing", |ctx, this, _| { let id = ctx.get::<Thing>(this)?.id; trace(ctx, format!("expr {}", id))?; let r = ctx.insert("Thing", Thing {id}); Ok(Value::object("Thing", r)) }).unwrap();
    reg.function("var", &["int"], "Var", |ctx, a| { let id = a[0].as_int()?; trace(ctx, format!("var {}", id))?; let r = ctx.insert("Var", Thing {id}); Ok(Value::object("Var", r)) }).unwrap();
    reg.function("twice", &["float"], "float", |_, a| Ok(Value::Float(a[0].as_float()? * 2.0))).unwrap();
    reg.function("thing", &["int"], "Thing", |ctx, a| { let r = ctx.insert("Thing", Thing { id: a[0].as_int()? }); Ok(Value::object("Thing", r)) }).unwrap();
    reg.function("twice", &["int"], "int", |_, a| Ok(Value::Int(a[0].as_int()? * 2))).unwrap();
    reg.function("bound", &["int"], "int", |ctx, a| { let value = a[0].as_int()?; trace(ctx, format!("bound {}", value))?; Ok(Value::Int(value)) }).unwrap();
    reg.function("half", &["float"], "float", |_, a| Ok(Value::Float(a[0].as_float()? / 2.0))).unwrap();
    reg.function("yes", &[], "bool", |ctx, _| { trace(ctx, "yes".into())?; Ok(Value::Bool(true)) }).unwrap();
    reg.function("fail", &["string"], "void", |_, a| Err(Error::host(a[0].as_str()?.to_string()))).unwrap();
    reg.function("bad_int", &[], "int", |_, _| Ok(Value::Str("x".into()))).unwrap();
    reg
}

#[derive(Debug, PartialEq)]
pub enum Outcome { Trace(Vec<String>), RuntimeError(Error), LoadError(String) }

#[derive(Debug, Clone)]
enum JsonArg {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Object(String, i64),
}

fn parse_arg_json(text: &str) -> Result<Vec<JsonArg>, String> {
    let text = text.trim();
    if !text.starts_with('[') || !text.ends_with(']') {
        return Err("arguments must be a JSON array".into());
    }
    let inner = &text[1..text.len() - 1];
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (i, c) in inner.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                quoted = false;
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(inner[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(inner[start..].trim());
    parts.into_iter().map(parse_one_arg).collect()
}

fn parse_one_arg(text: &str) -> Result<JsonArg, String> {
    if text == "true" {
        return Ok(JsonArg::Bool(true));
    }
    if text == "false" {
        return Ok(JsonArg::Bool(false));
    }
    if text.starts_with('"') && text.ends_with('"') {
        return Ok(JsonArg::String(
            text[1..text.len() - 1]
                .replace("\\\"", "\"")
                .replace("\\n", "\n"),
        ));
    }
    if text.starts_with('{') {
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let marker = "\"id\":";
        let id = compact
            .find(marker)
            .and_then(|p| {
                compact[p + marker.len()..]
                    .trim_end_matches('}')
                    .parse()
                    .ok()
            })
            .ok_or_else(|| "object argument needs integer id".to_string())?;
        let tmark = "\"type\":\"";
        let ty = compact.find(tmark).map(|p| { let rest = &compact[p + tmark.len()..]; rest[..rest.find('"').unwrap_or(rest.len())].to_string() }).unwrap_or_default();
        return Ok(JsonArg::Object(ty, id));
    }
    if text.contains('.') {
        return text
            .parse::<f64>()
            .map(JsonArg::Float)
            .map_err(|e| e.to_string());
    }
    text.parse::<i64>()
        .map(JsonArg::Int)
        .map_err(|e| e.to_string())
}

fn arg_value(arg: JsonArg, ty: &str, ctx: &mut Context) -> Result<Value, Error> {
    // WP11 F5: a JSON kind that does not match the declared parameter type is a runner error, never a VM input.
    let bad = |what: &str| Error::host(format!("conformance: bad call args: {} does not match parameter type {}", what, ty));
    match (arg, ty) {
        (JsonArg::Int(v), "int") => Ok(Value::Int(v)),
        (JsonArg::Int(v), "float") => Ok(Value::Float(v as f64)),
        (JsonArg::Float(v), "float") => Ok(Value::Float(v)),
        (JsonArg::String(v), "string") => Ok(Value::string(v)),
        (JsonArg::String(v), t) if t != "int" && t != "float" && t != "bool" && t != "Thing" => Ok(Value::string(v)),
        (JsonArg::Bool(v), "bool") => Ok(Value::Bool(v)),
        (JsonArg::Object(t, id), "Thing") if t == "Thing" => { let slot = ctx.insert("Thing", Thing { id }); Ok(Value::object("Thing", slot)) }
        (JsonArg::Object(t, _), _) => Err(bad(&format!("object of type {}", t))),
        (JsonArg::Int(_), _) => Err(bad("int")),
        (JsonArg::Float(_), _) => Err(bad("float")),
        (JsonArg::String(_), _) => Err(bad("string")),
        (JsonArg::Bool(_), _) => Err(bad("bool")),
    }
}

fn catalog_registry(schema: &[u8], temp_dir: Option<String>) -> (Registry, Rc<RefCell<Vec<u8>>>) {
    let std_catalog = [b"\"str_len\"".as_slice(), b"\"math_abs\"", b"\"str_list\"", b"\"print\""].iter().any(|needle| schema.windows(needle.len()).any(|w| w == *needle));
    let mut reg = stub_registry_mode(std_catalog);
    let printed = Rc::new(RefCell::new(Vec::new()));
    if std_catalog {
        let sink: dslvm::stdlib::testing::Sink = printed.clone();
        dslvm::stdlib::register_testing(&mut reg, Some(sink)).unwrap();
    }
    if std_catalog {
        let captured = Rc::clone(&printed);
        reg.function("printed", &[], "string", move |_, _| Ok(Value::string(captured.borrow().clone()))).unwrap();
        if let Some(path) = temp_dir {
            let long_path = path.clone();
            reg.function("test_temp_dir", &[], "string", move |_, _| Ok(Value::string(path.clone()))).unwrap();
            reg.function("test_long_path", &[], "string", move |_, _| {
                let mut dir = std::path::PathBuf::from(&long_path);
                for i in 0..7 { dir.push(format!("segment-{}{}", i, "a".repeat(40))); }
                std::fs::create_dir_all(&dir).map_err(|e| Error::host(format!("fixture: cannot create long-path directory: {}", e)))?;
                dir.push("résumé-文件-😀.bin");
                Ok(Value::string(dir.to_string_lossy().into_owned()))
            }).unwrap();
        }
    }
    (reg, printed)
}

/// Runs one vector: load (phase 1) then run (phase 2) with a fresh context.
pub fn run_vector(bytecode: &[u8], schema: &[u8]) -> Outcome {
    let temp_dir = if schema.windows(b"\"test_temp_dir\"".len()).any(|w| w == b"\"test_temp_dir\"") {
        let path = std::env::temp_dir().join(format!("lmcode-file-vector-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&path).map_err(|e| Outcome::RuntimeError(Error::host(format!("fixture: cannot create fresh temp directory: {}", e)))).ok();
        Some(path.to_string_lossy().into_owned())
    } else { None };
    let (reg, _) = catalog_registry(schema, temp_dir.clone());
    let program = match Program::load(bytecode, schema, &reg) { Ok(p) => p, Err(e) => return Outcome::LoadError(e.message) };
    let mut ctx = Context::new();
    let t = ctx.set_object("t", "Trace", Trace(Vec::new()));
    ctx.set_object("root", "Thing", Thing { id: 0 });
    ctx.set_object("v", "Var", Thing { id: 0 });
    ctx.set_object("xs", "IntList", IntList { values: vec![0, 1, 2, 3] });
    let outcome = match program.run(&mut ctx, &reg) { Ok(()) => Outcome::Trace(ctx.take::<Trace>(t).map(|t| t.0).unwrap_or_default()), Err(e) => Outcome::RuntimeError(e) };
    if let Some(path) = temp_dir { let _ = std::fs::remove_dir_all(path); }
    outcome
}

enum EntryOutcome {
    Success { result: String, trace: Vec<String> },
    Error(Error),
    Load(String),
}

fn value_text(value: &Value, ctx: &Context) -> String {
    match value {
        Value::Int(v) => format!("int {}", v),
        Value::Float(v) => format!("float {}", format_g15(*v)),
        Value::Str(v) => format!("string {}", v),
        Value::Bool(v) => format!("bool {}", v),
        Value::None => "void".into(),
        Value::Obj { obj, .. } => ctx
            .get::<Thing>(*obj)
            .map(|t| format!("thing {}", t.id))
            .unwrap_or_else(|_| "thing ?".into()),
    }
}

fn run_entry_vector(
    bytecode: &[u8],
    schema: &[u8],
    entry_name: &str,
    args_json: &str,
) -> EntryOutcome {
    let reg = stub_registry_mode(false);
    let program = match Program::load(bytecode, schema, &reg) {
        Ok(p) => p,
        Err(e) => return EntryOutcome::Load(e.message),
    };
    let entry_index = match program.entries().iter().position(|e| e.name == entry_name) {
        Some(i) => i,
        None => {
            return EntryOutcome::Error(Error::host(format!(
                "entry '{}' is not present",
                entry_name
            )))
        }
    };
    let entry = &program.entries()[entry_index];
    let raw = match parse_arg_json(args_json) {
        Ok(v) => v,
        Err(e) => return EntryOutcome::Error(Error::host(e)),
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
    if let Err(e) = program.run(&mut ctx, &reg) {
        return EntryOutcome::Error(e);
    }
    let args = raw
        .into_iter()
        .enumerate()
        .map(|(i, arg)| arg_value(arg, entry.params.get(i).map(String::as_str).unwrap_or(""), &mut ctx))
        .collect::<Result<Vec<_>, _>>();
    let args = match args {
        Ok(v) => v,
        Err(e) => return EntryOutcome::Error(e),
    };
    match program.call(entry_index, &mut ctx, &reg, args) {
        Ok(result) => EntryOutcome::Success {
            result: value_text(&result, &ctx),
            trace: ctx.take::<Trace>(t).map(|v| v.0).unwrap_or_default(),
        },
        Err(e) => EntryOutcome::Error(e),
    }
}

pub struct Expect { pub kind: String, pub lines: Vec<String> }
pub fn read_expect(path: &Path) -> Expect {
    let text = std::fs::read_to_string(path).expect("expect file");
    let mut it = text.lines(); let kind = it.next().unwrap_or("").to_string();
    Expect { kind, lines: it.map(|l| l.replace("\\n", "\n")).collect() }
}
/// A trace line beginning with `~ ` compares its final numeric field within the shared math tolerance.
/// The text before that field (for example `float`) must still match exactly.
fn trace_line_matches(expected: &str, actual: &str) -> bool {
    let Some(expected) = expected.strip_prefix("~ ") else { return expected == actual };
    let Some((eprefix, enumber)) = expected.rsplit_once(' ') else { return false };
    let Some((aprefix, anumber)) = actual.rsplit_once(' ') else { return false };
    let (Ok(e), Ok(a)) = (enumber.parse::<f64>(), anumber.parse::<f64>()) else { return false };
    eprefix == aprefix && e.is_finite() && a.is_finite() && (a - e).abs() <= 1e-12 * e.abs().max(1e-300)
}

fn trace_matches(expected: &[String], actual: &[String]) -> bool {
    expected.len() == actual.len() && expected.iter().zip(actual).all(|(e, a)| trace_line_matches(e, a))
}
fn bare_error(e: &Error) -> String {
    let Some(line) = e.line else { return e.message.clone() };
    let prefix = format!("dsl line {}{}: ", line, e.column.map(|c| format!(":{}", c)).unwrap_or_default());
    e.message.strip_prefix(&prefix).unwrap_or(&e.message).to_string()
}
/// The manifest names the schema file per vector (`"schema": "schema-extra.json"` and the like); default schema.json.
pub fn schema_for(dir: &Path, name: &str) -> PathBuf {
    let manifest = std::fs::read_to_string(dir.join("manifest.json")).unwrap_or_default();
    if let Some(p) = manifest.find(&format!("\"name\": \"{}\"", name)) {
        let rest = &manifest[p..]; let end = rest[1..].find("\"name\": \"").map(|e| e + 1).unwrap_or(rest.len());
        if let Some(q) = rest[..end].find("\"schema\": \"") { let v = &rest[q + 11..end]; if let Some(e) = v.find('"') { return dir.join(&v[..e]); } }
    }
    dir.join("schema.json")
}
pub fn vector_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).expect("vector dir").filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".expect")).map(|n| n.trim_end_matches(".expect").to_string()).collect();
    names.sort(); names
}
/// Checks one vector; returns an error description when it does not match.
pub fn check_vector(dir: &Path, name: &str) -> Result<String, String> {
    let expect = read_expect(&dir.join(format!("{}.expect", name)));
    let bytecode = std::fs::read(dir.join(format!("{}.dslb", name))).map_err(|e| e.to_string())?;
    let schema = std::fs::read(schema_for(dir, name)).map_err(|e| e.to_string())?;
    let outcome = if expect.kind == "entries" {
        Outcome::Trace(Vec::new())
    } else if expect.kind == "call"
        || expect.kind == "call_error"
        || expect.kind == "call_error_at"
    {
        let first = expect.lines.first().cloned().unwrap_or_default();
        let mut split = first.splitn(2, ' ');
        let entry = split.next().unwrap_or("");
        let args = split.next().unwrap_or("[]");
        match run_entry_vector(&bytecode, &schema, entry, args) {
            EntryOutcome::Success { result, trace } => Outcome::Trace({
                let mut lines = vec![format!("result {}", result)];
                lines.extend(trace);
                lines
            }),
            EntryOutcome::Error(e) => Outcome::RuntimeError(e),
            EntryOutcome::Load(m) => Outcome::LoadError(m),
        }
    } else {
        run_vector(&bytecode, &schema)
    };
    if std::env::var_os("LMCODE_TRACE").is_some() {
        match &outcome {
            Outcome::Trace(lines) => {
                for line in lines {
                    println!("TRACE\t{}\t{}", name, line);
                }
            }
            Outcome::RuntimeError(error) => {
                println!("ERROR\t{}\t{}\t{}", name, error.code, bare_error(error))
            }
            _ => {}
        }
    }
    let ok = match (expect.kind.as_str(), &outcome) {
        ("trace", Outcome::Trace(lines)) => trace_matches(&expect.lines, lines),
        ("entries", Outcome::Trace(_lines)) => {
            let program = Program::load(&bytecode, &schema, &stub_registry_mode(false))
                .map_err(|e| e.message)?;
            let actual = program.entries().iter().map(|e| format!("{} {} {}",e.name,e.annotation,e.iface)).collect::<Vec<_>>();
            expect.lines == actual
        }
        ("call", Outcome::Trace(lines)) => expect.lines.len() >= 2 && trace_matches(&expect.lines[1..], lines),
        ("call_error", Outcome::RuntimeError(e)) => expect.lines.len() == 3 && e.code == expect.lines[1] && e.message.contains(&expect.lines[2]),
        ("call_error_at", Outcome::RuntimeError(e)) => {
            expect.lines.len() >= 5
                && e.code == expect.lines[1]
                && e.line.map(|x| x.to_string()) == Some(expect.lines[2].clone())
                && e.column.map(|x| x.to_string()) == Some(expect.lines[3].clone())
                && e.message.contains(&expect.lines[4])
        }
        ("runtime_error", Outcome::RuntimeError(e)) => {
            expect.lines.len() == 1 && e.message.contains(&expect.lines[0])
        }
        ("runtime_error_exact", Outcome::RuntimeError(e)) => {
            expect.lines.len() == 2 && e.code == expect.lines[0] && bare_error(e) == expect.lines[1]
        }
        ("runtime_error_file", Outcome::RuntimeError(e)) => {
            let function = expect.lines.get(1).cloned().unwrap_or_default();
            let path_suffix = expect
                .lines
                .get(2)
                .map(|p| format!("/{}: ", p))
                .unwrap_or_default();
            let message = bare_error(e);
            e.code == expect.lines.first().map(String::as_str).unwrap_or("")
                && message.contains(&format!("{}: ", function))
                && message.contains(&path_suffix)
                && message
                    .rsplit_once(": ")
                    .map(|(_, reason)| reason.chars().any(|c| !c.is_whitespace()))
                    .unwrap_or(false)
        }
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
pub fn default_dir() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("vectors") }

fn main() {
    let dir = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(default_dir);
    let names = vector_names(&dir);
    if names.is_empty() {
        eprintln!("no vectors in {}", dir.display());
        std::process::exit(2);
    }
    let mut failed = 0;
    for name in &names {
        match check_vector(&dir, name) { Ok(kind) => println!("ok - {} ({})", name, kind), Err(e) => { failed += 1; println!("FAIL - {}", e); } }
    }
    println!("{} {}/{} vectors", if failed > 0 { "FAIL" } else { "PASS" }, names.len() - failed, names.len());
    if failed > 0 { std::process::exit(1); }
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
        // Same digest as scripts/sync-vectors.sh: sha256 over the `sha256sum` lines of every vector file in byte-sorted order.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        for (dir_name, lock_name) in [("conformance/vectors", "vectors.lock"), ("conformance/stdlib-vectors", "stdlib-vectors.lock")] {
            let lock = std::fs::read_to_string(root.join(lock_name)).expect("vector lock");
            let want = lock.lines().find_map(|l| l.strip_prefix("sha256=")).expect("sha256 line");
            let dir = root.join(dir_name);
            let mut names: Vec<String> = std::fs::read_dir(&dir).expect("vectors dir").map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
            names.sort();
            assert!(!names.is_empty(), "{} is empty", dir_name);
            let mut listing = String::new();
            for n in &names { listing.push_str(&format!("{}  ./{}\n", dslvm::hex(&dslvm::sha256(&std::fs::read(dir.join(n)).unwrap())), n)); }
            assert_eq!(dslvm::hex(&dslvm::sha256(listing.as_bytes())), want, "{} differs from {}: run scripts/sync-vectors.sh", dir_name, lock_name);
        }
    }
    #[test]
    fn every_vector_gives_the_expected_result_class() {
        let dir = default_dir(); let names = vector_names(&dir);
        assert_eq!(names.len(),70,"core v6 vector count");
        let failures: Vec<String> = names.iter().filter_map(|n| check_vector(&dir, n).err()).collect();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
    #[test]
    fn every_stdlib_vector_gives_the_expected_result_class() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib-vectors"); let names = vector_names(&dir);
        assert!(names.len() >= 267, "standard property vectors are missing");
        for seed in ["fuzz-0000000007", "fuzz-0000000042", "fuzz-0020261024", "fuzz-large-file", "fuzz-map-churn"] {
            assert!(names.iter().any(|name| name == seed), "missing corpus seed {}", seed);
        }
        let failures: Vec<String> = names.iter().filter_map(|n| check_vector(&dir, n).err()).collect();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
    #[test]
    fn every_stdlib_schema_uses_the_default_catalog_surface() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("stdlib-vectors");
        for name in ["schema.json", "math.schema.json", "collections.schema.json"] {
            let schema = std::fs::read(dir.join(name)).unwrap();
            let (registry, _) = catalog_registry(&schema, None);
            assert_eq!(registry.std_signatures().len(), 181, "{}", name);
        }
    }
    #[test]
    fn a_wrong_expectation_is_detected() {
        let dir = default_dir();
        let bytecode = std::fs::read(dir.join("literals.dslb")).unwrap(); let schema = std::fs::read(dir.join("schema.json")).unwrap();
        match run_vector(&bytecode, &schema) { Outcome::Trace(lines) => assert_ne!(lines, vec!["int 41".to_string()]), other => panic!("{:?}", other) }
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
