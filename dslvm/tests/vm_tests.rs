//! Mirrors cpp-dsl-sdk tests/dslvm_test.cpp (bytecode v4) plus robustness: every truncation length and bit flips of a valid
//! file must produce an `Err`, never a panic.
use dslvm::*;

const SCHEMA: &[u8] = br#"{"dsl.schema":1,"name":"t"}"#;
const T_INT: &str = "method Trace.int(int) -> void";
const T_STR: &str = "method Trace.str(string) -> void";
const THING_ID: &str = "method Thing.id() -> int";
const TWICE: &str = "fn twice(int) -> int";
const BOOM: &str = "fn boom() -> void";

#[derive(Default)]
struct Asm { consts: Vec<String>, imports: Vec<u32>, contexts: Vec<(u32, u32)>, locals: Vec<(u32, u8)>, max_stack: u16, code: Vec<u8>, lines: Vec<(u32, u32, u16)> }
impl Asm {
    fn k(&mut self, s: &str) -> u32 { if let Some(i) = self.consts.iter().position(|c| c == s) { return i as u32; } self.consts.push(s.to_string()); (self.consts.len() - 1) as u32 }
    fn imp(&mut self, canonical: &str) -> u16 { let c = self.k(canonical); if let Some(i) = self.imports.iter().position(|x| *x == c) { return i as u16; } self.imports.push(c); (self.imports.len() - 1) as u16 }
    fn ctx_index(&mut self, name: &str, type_name: &str) -> u16 { let e = (self.k(name), self.k(type_name)); if let Some(i) = self.contexts.iter().position(|x| *x == e) { return i as u16; } self.contexts.push(e); (self.contexts.len() - 1) as u16 }
    fn local(mut self, type_name: &str, hidden: u8) -> Self { let k = self.k(type_name); self.locals.push((k, hidden)); self }
    fn line(mut self, pc: u32, line: u32, col: u16) -> Self { self.lines.push((pc, line, col)); self }
    fn u8(mut self, v: u8) -> Self { self.code.push(v); self }
    fn u16(mut self, v: u16) -> Self { self.code.extend_from_slice(&v.to_be_bytes()); self }
    fn u32(mut self, v: u32) -> Self { self.code.extend_from_slice(&v.to_be_bytes()); self }
    fn i32(mut self, v: i32) -> Self { self.code.extend_from_slice(&v.to_be_bytes()); self }
    fn i64(mut self, v: i64) -> Self { self.code.extend_from_slice(&v.to_be_bytes()); self }
    fn f64(mut self, v: f64) -> Self { self.code.extend_from_slice(&v.to_be_bytes()); self }
    fn push_int(self, v: i64) -> Self { self.u8(0x01).i64(v) }
    fn push_float(self, v: f64) -> Self { self.u8(0x12).f64(v) }
    fn push_str(mut self, s: &str) -> Self { let k = self.k(s); self.u8(0x02).u32(k) }
    fn push_bool(self, b: bool) -> Self { self.u8(0x03).u8(b as u8) }
    fn ctx(mut self, n: &str, t: &str) -> Self { let i = self.ctx_index(n, t); self.u8(0x06).u16(i) }
    fn func(mut self, canonical: &str, argc: u8) -> Self { let i = self.imp(canonical); self.u8(0x07).u16(i).u8(argc) }
    fn method(mut self, canonical: &str, argc: u8) -> Self { let i = self.imp(canonical); self.u8(0x08).u16(i).u8(argc) }
    fn halt(self) -> Self { self.u8(0x11) }
    fn file_v(&self, schema: &[u8], version: u16, magic: &[u8]) -> Vec<u8> {
        let mut out = magic.to_vec(); out.extend_from_slice(&version.to_be_bytes()); out.extend_from_slice(&sha256(schema));
        out.extend_from_slice(&(self.consts.len() as u32).to_be_bytes());
        for c in &self.consts { out.push(1); out.extend_from_slice(&(c.len() as u32).to_be_bytes()); out.extend_from_slice(c.as_bytes()); }
        out.extend_from_slice(&(self.imports.len() as u16).to_be_bytes()); for i in &self.imports { out.extend_from_slice(&i.to_be_bytes()); }
        out.extend_from_slice(&(self.contexts.len() as u16).to_be_bytes()); for (n, t) in &self.contexts { out.extend_from_slice(&n.to_be_bytes()); out.extend_from_slice(&t.to_be_bytes()); }
        out.extend_from_slice(&(self.locals.len() as u16).to_be_bytes()); for (t, h) in &self.locals { out.extend_from_slice(&t.to_be_bytes()); out.push(*h); }
        out.extend_from_slice(&self.max_stack.to_be_bytes());
        out.extend_from_slice(&(self.code.len() as u32).to_be_bytes()); out.extend_from_slice(&self.code);
        out.extend_from_slice(&(self.lines.len() as u32).to_be_bytes()); for (pc, l, c) in &self.lines { out.extend_from_slice(&pc.to_be_bytes()); out.extend_from_slice(&l.to_be_bytes()); out.extend_from_slice(&c.to_be_bytes()); }
        out
    }
    fn file(&self) -> Vec<u8> { self.file_v(SCHEMA, VERSION, b"DSLB") }
}
struct Thing { id: i64 }
struct Trace(Vec<String>);
fn registry() -> Registry {
    let mut reg = Registry::new();
    reg.context("t", "Trace").unwrap().context("root", "Thing").unwrap();
    reg.method("Trace", "int", &["int"], "void", |ctx, this, a| { let v = a[0].as_int()?; ctx.get_mut::<Trace>(this)?.0.push(format!("int {}", v)); Ok(Value::None) }).unwrap();
    reg.method("Trace", "str", &["string"], "void", |ctx, this, a| { let v = a[0].as_str()?.to_string(); ctx.get_mut::<Trace>(this)?.0.push(format!("str {}", v)); Ok(Value::None) }).unwrap();
    reg.method("Thing", "id", &[], "int", |ctx, this, _| Ok(Value::Int(ctx.get::<Thing>(this)?.id))).unwrap();
    reg.function("twice", &["int"], "int", |_, a| Ok(Value::Int(a[0].as_int()?.wrapping_mul(2)))).unwrap();
    reg.function("boom", &[], "void", |_, _| Err(Error::host("boom from the host"))).unwrap();
    reg
}
fn run(bytes: &[u8]) -> Result<Vec<String>, Error> {
    let reg = registry(); let p = Program::load(bytes, SCHEMA, &reg)?;
    let mut ctx = Context::new(); let t = ctx.set_object("t", "Trace", Trace(Vec::new())); ctx.set_object("root", "Thing", Thing { id: 0 });
    p.run(&mut ctx, &reg)?; Ok(ctx.take::<Trace>(t)?.0)
}
fn load_err(bytes: &[u8]) -> String { Program::load(bytes, SCHEMA, &registry()).err().expect("load must fail").message }
fn run_err(bytes: &[u8]) -> String { run(bytes).err().expect("run must fail").message }
fn t() -> Asm { Asm::default().ctx("t", "Trace") }

#[test] fn arithmetic_concat_methods() { let a = t().push_int(1).push_int(2).u8(0x13).method(T_INT, 1).ctx("t", "Trace").push_str("a").push_str("b").u8(0x26).method(T_STR, 1).halt(); assert_eq!(run(&a.file()).unwrap(), ["int 3", "str ab"]); }
#[test] fn host_function_and_locals() { let a = Asm::default().local("int", 0).push_int(21).func(TWICE, 1).u8(0x05).u16(0).ctx("t", "Trace").u8(0x04).u16(0).method(T_INT, 1).halt(); assert_eq!(run(&a.file()).unwrap(), ["int 42"]); }
#[test] fn forward_jumps_select_branch() { let a = Asm::default().push_bool(true).u8(0x0D).i32(21).ctx("t", "Trace").push_int(1).method(T_INT, 1).u8(0x0C).i32(16).ctx("t", "Trace").push_int(2).method(T_INT, 1).halt(); assert_eq!(run(&a.file()).unwrap(), ["int 1"]); }
#[test] fn context_floats_conversions_equality() { let a = t().ctx("root", "Thing").method(THING_ID, 0).method(T_INT, 1).ctx("t", "Trace").push_float(2.5).u8(0x28).method(T_STR, 1).ctx("t", "Trace").push_int(7).push_int(2).u8(0x16).u8(0x25).push_float(0.5).u8(0x17).u8(0x28).method(T_STR, 1).ctx("t", "Trace").push_int(3).push_int(3).u8(0x09).u8(0x29).method(T_STR, 1).halt(); assert_eq!(run(&a.file()).unwrap(), ["int 0", "str 2.5", "str 3.5", "str true"]); }
#[test] fn empty_program_loads() { let p = Program::load(&Asm::default().halt().file(), SCHEMA, &registry()).unwrap(); assert_eq!((p.constant_count(), p.code_size(), p.import_count(), p.locals().len(), p.lines().len()), (0, 1, 0, 0, 0)); }
#[test] fn int64_literals_and_wraparound() {
    let a = t().push_int(i64::MAX).method(T_INT, 1).ctx("t", "Trace").push_int(i64::MIN).method(T_INT, 1).ctx("t", "Trace").push_int(i64::MAX).push_int(1).u8(0x13).method(T_INT, 1).ctx("t", "Trace").push_int(i64::MIN).u8(0x1B).method(T_INT, 1).ctx("t", "Trace").push_int(i64::MIN).push_int(-1).u8(0x16).method(T_INT, 1).ctx("t", "Trace").push_int(i64::MIN).push_int(-1).u8(0x33).method(T_INT, 1).ctx("t", "Trace").push_int(i64::MAX).push_int(i64::MAX).u8(0x15).method(T_INT, 1).halt();
    assert_eq!(run(&a.file()).unwrap(), ["int 9223372036854775807", "int -9223372036854775808", "int -9223372036854775808", "int -9223372036854775808", "int -9223372036854775808", "int 0", "int 1"]);
}
#[test] fn mod_takes_the_dividends_sign() { let a = t().push_int(7).push_int(3).u8(0x33).method(T_INT, 1).ctx("t", "Trace").push_int(-7).push_int(3).u8(0x33).method(T_INT, 1).ctx("t", "Trace").push_int(7).push_int(-3).u8(0x33).method(T_INT, 1).ctx("t", "Trace").push_int(-7).push_int(2).u8(0x16).method(T_INT, 1).halt(); assert_eq!(run(&a.file()).unwrap(), ["int 1", "int -1", "int 1", "int -3"]); }
#[test] fn sections_are_read_back() {
    let mut a = Asm::default().local("int", 0).local("Thing", 1).line(0, 3, 5).line(9, 4, 1).push_int(1).u8(0x05).u16(0).halt(); a.max_stack = 7;
    let p = Program::load(&a.file(), SCHEMA, &registry()).unwrap();
    assert_eq!(p.max_stack(), 7); assert_eq!(p.locals(), &[Local { type_name: "int".into(), hidden: false }, Local { type_name: "Thing".into(), hidden: true }]);
    assert_eq!(p.lines().len(), 2); assert_eq!(p.line_of(0).unwrap().line, 3); assert_eq!(p.line_of(8).unwrap().line, 3); assert_eq!(p.line_of(9).unwrap().line, 4); assert_eq!(p.line_of(12).unwrap().column, 1);
    assert_eq!(Program::load(&Asm::default().halt().file(), SCHEMA, &registry()).unwrap().line_of(0), None);
}
#[test] fn bad_magic() { assert!(load_err(&Asm::default().halt().file_v(SCHEMA, VERSION, b"XXXX")).contains("bad magic")); }
#[test] fn version_3_refused() { assert!(load_err(&Asm::default().halt().file_v(SCHEMA, 3, b"DSLB")).contains("unsupported bytecode version 3 (this runtime reads version 4)")); }
#[test] fn schema_mismatch() { assert!(load_err(&Asm::default().halt().file_v(b"other", VERSION, b"DSLB")).contains("schema mismatch")); }
#[test] fn truncated() { let f = t().halt().file(); assert!(load_err(&f[..f.len() - 3]).contains("bytecode is truncated")); }
#[test] fn trailing_bytes() { let mut f = Asm::default().halt().file(); f.push(0); assert!(load_err(&f).contains("trailing byte(s) after the line table")); }
#[test] fn unknown_constant_tag() { let mut f = Asm::default().push_str("x").halt().file(); f[4 + 2 + 32 + 4] = 7; assert!(load_err(&f).contains("constant 0 has unknown tag 7 (1 = string)")); }
#[test] fn constant_out_of_range() { assert!(load_err(&Asm::default().u8(0x02).u32(9).halt().file()).contains("constant index 9 out of range")); }
#[test] fn local_out_of_range() { assert!(load_err(&Asm::default().u8(0x04).u16(0).halt().file()).contains("local slot index 0 out of range")); }
#[test] fn context_index_out_of_range() { assert!(load_err(&Asm::default().u8(0x06).u16(0).halt().file()).contains("context index 0 out of range")); }
#[test] fn undeclared_context() { assert!(load_err(&Asm::default().ctx("nope", "Trace").halt().file()).contains("context variable 'nope' is not declared")); }
#[test] fn context_type_differs() { assert!(load_err(&Asm::default().ctx("t", "Thing").halt().file()).contains("context variable 't' is declared as Thing by the bytecode, the host provides Trace")); }
#[test] fn unbound_function_import() { assert!(load_err(&Asm::default().func("fn missing() -> int", 0).halt().file()).contains("import 'fn missing() -> int' is not provided by the host")); }
#[test] fn import_with_other_types_names_the_hosts() { assert!(load_err(&Asm::default().push_float(1.0).func("fn twice(float) -> int", 1).halt().file()).contains("import 'fn twice(float) -> int' is not provided by the host (the host declares fn twice(int) -> int)")); }
#[test] fn unbound_method_import() { assert!(load_err(&Asm::default().ctx("root", "Thing").method("method Thing.name() -> string", 0).halt().file()).contains("import 'method Thing.name() -> string' is not provided by the host")); }
#[test] fn method_import_with_other_types() { assert!(load_err(&Asm::default().ctx("root", "Thing").method("method Thing.id(int) -> int", 1).halt().file()).contains("(the host declares method Thing.id() -> int)")); }
#[test] fn call_arity_differs_from_import() { assert!(load_err(&Asm::default().push_int(1).push_int(2).func(TWICE, 2).halt().file()).contains("passes 2 argument(s), the import takes 1")); }
#[test] fn call_fn_on_method_import() { assert!(load_err(&Asm::default().ctx("root", "Thing").func(THING_ID, 0).halt().file()).contains("which is a method")); }
#[test] fn call_method_on_function_import() { assert!(load_err(&Asm::default().push_int(1).method(TWICE, 1).halt().file()).contains("which is a function")); }
#[test] fn import_index_out_of_range() { assert!(load_err(&Asm::default().u8(0x07).u16(4).u8(0).halt().file()).contains("import index 4 out of range")); }
#[test] fn jump_into_operand() { assert!(load_err(&Asm::default().u8(0x0C).i32(1).halt().file()).contains("not an instruction start")); }
#[test] fn backward_jump() { assert!(load_err(&Asm::default().u8(0x0C).i32(-5).halt().file()).contains("goes backwards")); }
#[test] fn missing_halt() { assert!(load_err(&Asm::default().push_int(1).file()).contains("does not end with HALT")); }
#[test] fn unknown_opcode() { assert!(load_err(&Asm::default().u8(0x7f).halt().file()).contains("unknown opcode 127")); }
#[test] fn bad_bool_operand() { assert!(load_err(&Asm::default().u8(0x03).u8(2).halt().file()).contains("PUSH_BOOL operand must be 0 or 1")); }
#[test] fn bad_hidden_flag() { assert!(load_err(&Asm::default().local("int", 2).halt().file()).contains("hidden flag must be 0 or 1")); }
#[test] fn unsorted_line_table() { assert!(load_err(&Asm::default().line(5, 1, 1).line(2, 2, 1).push_int(1).u8(0x10).halt().file()).contains("line table is not sorted")); }
#[test] fn line_entry_beyond_code() { assert!(load_err(&Asm::default().line(99, 1, 1).halt().file()).contains("beyond the code")); }
#[test] fn invalid_utf8_constant() { let mut f = Asm::default().push_str("ab").halt().file(); let p = f.windows(2).position(|w| w == b"ab").unwrap(); f[p] = 0xff; assert!(load_err(&f).contains("not valid UTF-8")); }
#[test] fn int_division_by_zero() { assert!(run_err(&Asm::default().push_int(1).push_int(0).u8(0x16).halt().file()).contains("int division by zero")); }
#[test] fn int_modulo_by_zero() { assert!(run_err(&Asm::default().push_int(1).push_int(0).u8(0x33).halt().file()).contains("int modulo by zero")); }
#[test] fn mixed_kind_equality() { assert!(run_err(&Asm::default().push_int(1).push_str("x").u8(0x09).halt().file()).contains("cannot compare int with string")); }
#[test] fn float_op_on_int() { assert!(run_err(&Asm::default().push_int(1).push_float(1.0).u8(0x17).halt().file()).contains("float operator applied to int and float")); }
#[test] fn local_read_before_store() { assert!(run_err(&Asm::default().local("int", 0).u8(0x04).u16(0).halt().file()).contains("read before assignment")); }
#[test] fn non_bool_condition() { assert!(run_err(&Asm::default().push_int(1).u8(0x0D).i32(0).halt().file()).contains("condition is a int, not a bool")); }
#[test] fn stack_underflow() { assert!(run_err(&Asm::default().u8(0x10).halt().file()).contains("stack underflow")); }
#[test] fn host_error_propagates() { assert!(run_err(&Asm::default().func(BOOM, 0).halt().file()).contains("boom from the host")); }
#[test] fn context_object_missing_at_run() { let reg = registry(); let p = Program::load(&t().halt().file(), SCHEMA, &reg).unwrap(); let mut empty = Context::new(); assert!(p.run(&mut empty, &reg).unwrap_err().message.contains("context variable 't' was not provided by the host for this run")); }
#[test] fn method_on_wrong_object_type() { assert!(run_err(&t().method(THING_ID, 0).halt().file()).contains("method Thing.id called on a Trace")); }
#[test] fn no_stack_cap() { let mut a = Asm::default(); a.max_stack = 1; for _ in 0..5000 { a = a.push_int(1); } for _ in 0..4999 { a = a.u8(0x13); } let a = a.ctx("t", "Trace").u8(0x0F).u8(0x10).halt(); assert!(run(&a.file()).is_ok()); }
#[test] fn no_string_cap() { let mut a = t().push_str(&"x".repeat(65536)); for _ in 0..9 { a = a.u8(0x0F).u8(0x26); } let got = run(&a.method(T_STR, 1).halt().file()).unwrap(); assert_eq!(got[0].len(), 4 + 65536 * 512); }
#[test] fn every_truncation_is_refused() {
    let f = t().push_int(1).push_int(2).u8(0x13).method(T_INT, 1).push_bool(true).u8(0x0D).i32(0).halt().file();
    for n in 0..f.len() { assert!(Program::load(&f[..n], SCHEMA, &registry()).is_err(), "truncation at {} must fail", n); }
}
#[test] fn context_arena_take_and_type_checks() {
    let mut ctx = Context::new(); let t = ctx.insert("Thing", Thing { id: 7 });
    assert!(ctx.get::<Trace>(t).is_err()); assert_eq!(ctx.get::<Thing>(t).unwrap().id, 7); assert_eq!(ctx.take::<Thing>(t).unwrap().id, 7); assert!(ctx.take::<Thing>(t).is_err());
}
#[test] fn signatures_are_sorted_canonical_strings() { assert_eq!(registry().signatures(), ["context root: Thing", "context t: Trace", "fn boom() -> void", "fn twice(int) -> int", "method Thing.id() -> int", "method Trace.int(int) -> void", "method Trace.str(string) -> void"]); }
#[test] fn duplicate_registration_is_refused() { let mut reg = Registry::new(); reg.function("f", &[], "void", |_, _| Ok(Value::None)).unwrap(); let e = reg.function("f", &[], "void", |_, _| Ok(Value::None)).err().unwrap(); assert_eq!(e.code, "LOAD_UNBOUND"); assert!(e.message.contains("registered twice")); }
#[test] fn error_codes_classify_the_existing_errors() {
    assert_eq!(Program::load(&Asm::default().halt().file_v(SCHEMA, VERSION, b"XXXX"), SCHEMA, &registry()).unwrap_err().code, "LOAD_BAD_BYTECODE");
    assert_eq!(Program::load(&Asm::default().halt().file_v(b"other", VERSION, b"DSLB"), SCHEMA, &registry()).unwrap_err().code, "LOAD_SCHEMA_MISMATCH");
    assert_eq!(Program::load(&Asm::default().func("fn missing() -> void", 0).halt().file(), SCHEMA, &registry()).unwrap_err().code, "LOAD_UNBOUND");
    assert_eq!(Program::load(&Asm::default().ctx("t", "Thing").halt().file(), SCHEMA, &registry()).unwrap_err().code, "LOAD_UNBOUND");
    assert_eq!(run(&Asm::default().func(BOOM, 0).halt().file()).unwrap_err().code, "RUN_HOST");
    assert_eq!(run(&Asm::default().push_int(1).push_int(0).u8(0x16).halt().file()).unwrap_err().code, "RUN_ERROR");
}
#[test] fn result_line_exit_codes() { assert_eq!(report_error("load", &Error::new("LOAD_UNBOUND", "x")), 3); assert_eq!(report_error("run", &Error::host("no")), 3); assert_eq!(report_app_failure("nothing"), 4); assert_eq!(report_ok("42"), 0); assert_eq!(json_escape("a\"b\n"), "\"a\\\"b\\n\""); }
