//! Mirrors cpp-dsl-sdk tests/dslvm_test.cpp (36 checks) plus robustness: every truncation length and bit flips of a valid
//! file must produce an `Err`, never a panic.
use dslvm::*;

const SCHEMA: &[u8] = br#"{"dsl.schema":1,"name":"t"}"#;
#[derive(Default)]
struct Asm { consts: Vec<String>, code: Vec<u8>, locals: u16 }
impl Asm {
    fn k(&mut self, s: &str) -> u16 { if let Some(i) = self.consts.iter().position(|c| c == s) { return i as u16; } self.consts.push(s.to_string()); (self.consts.len() - 1) as u16 }
    fn u8(mut self, v: u8) -> Self { self.code.push(v); self }
    fn u16(mut self, v: u16) -> Self { self.code.extend_from_slice(&v.to_be_bytes()); self }
    fn i32(mut self, v: i32) -> Self { self.code.extend_from_slice(&v.to_be_bytes()); self }
    fn f64(mut self, v: f64) -> Self { self.code.extend_from_slice(&v.to_be_bytes()); self }
    fn push_int(self, v: i32) -> Self { self.u8(0x01).i32(v) }
    fn push_float(self, v: f64) -> Self { self.u8(0x12).f64(v) }
    fn push_str(mut self, s: &str) -> Self { let k = self.k(s); self.u8(0x02).u16(k) }
    fn push_bool(self, b: bool) -> Self { self.u8(0x03).u8(b as u8) }
    fn ctx(mut self, n: &str) -> Self { let k = self.k(n); self.u8(0x06).u16(k) }
    fn func(mut self, n: &str, argc: u8) -> Self { let k = self.k(n); self.u8(0x07).u16(k).u8(argc) }
    fn method(mut self, t: &str, n: &str, argc: u8) -> Self { let kt = self.k(t); let kn = self.k(n); self.u8(0x08).u16(kt).u16(kn).u8(argc) }
    fn halt(self) -> Self { self.u8(0x11) }
    fn file_v(&self, schema: &[u8], version: u16, magic: &[u8]) -> Vec<u8> {
        let mut out = magic.to_vec(); out.extend_from_slice(&version.to_be_bytes()); out.extend_from_slice(&sha256(schema));
        out.extend_from_slice(&(self.consts.len() as u16).to_be_bytes());
        for c in &self.consts { out.extend_from_slice(&(c.len() as u16).to_be_bytes()); out.extend_from_slice(c.as_bytes()); }
        out.extend_from_slice(&self.locals.to_be_bytes()); out.extend_from_slice(&(self.code.len() as u32).to_be_bytes()); out.extend_from_slice(&self.code); out
    }
    fn file(&self) -> Vec<u8> { self.file_v(SCHEMA, VERSION, b"DSLB") }
}
struct Thing { id: i32 }
struct Trace(Vec<String>);
fn registry() -> Registry {
    let mut reg = Registry::new();
    reg.context("t", "Trace").unwrap().context("root", "Thing").unwrap();
    reg.method("Trace", "int", &["int"], "void", |ctx, this, a| { let v = a[0].as_int()?; ctx.get_mut::<Trace>(this)?.0.push(format!("int {}", v)); Ok(Value::None) }).unwrap();
    reg.method("Trace", "str", &["string"], "void", |ctx, this, a| { let v = a[0].as_str()?.to_string(); ctx.get_mut::<Trace>(this)?.0.push(format!("str {}", v)); Ok(Value::None) }).unwrap();
    reg.method("Thing", "id", &[], "int", |ctx, this, _| Ok(Value::Int(ctx.get::<Thing>(this)?.id))).unwrap();
    reg.function("twice", &["int"], "int", |_, a| Ok(Value::Int(a[0].as_int()? * 2))).unwrap();
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

#[test] fn arithmetic_concat_methods() { let a = Asm::default().ctx("t").push_int(1).push_int(2).u8(0x13).method("Trace", "int", 1).ctx("t").push_str("a").push_str("b").u8(0x26).method("Trace", "str", 1).halt(); assert_eq!(run(&a.file()).unwrap(), ["int 3", "str ab"]); }
#[test] fn host_function_and_locals() { let mut a = Asm::default(); a.locals = 1; let a = a.push_int(21).func("twice", 1).u8(0x05).u16(0).ctx("t").u8(0x04).u16(0).method("Trace", "int", 1).halt(); assert_eq!(run(&a.file()).unwrap(), ["int 42"]); }
#[test] fn forward_jumps_select_branch() { let a = Asm::default().push_bool(true).u8(0x0D).i32(19).ctx("t").push_int(1).method("Trace", "int", 1).u8(0x0C).i32(14).ctx("t").push_int(2).method("Trace", "int", 1).halt(); assert_eq!(run(&a.file()).unwrap(), ["int 1"]); }
#[test] fn context_floats_conversions_equality() { let a = Asm::default().ctx("t").ctx("root").method("Thing", "id", 0).method("Trace", "int", 1).ctx("t").push_float(2.5).u8(0x28).method("Trace", "str", 1).ctx("t").push_int(7).push_int(2).u8(0x16).u8(0x25).push_float(0.5).u8(0x17).u8(0x28).method("Trace", "str", 1).ctx("t").push_int(3).push_int(3).u8(0x09).u8(0x29).method("Trace", "str", 1).halt(); assert_eq!(run(&a.file()).unwrap(), ["int 0", "str 2.5", "str 3.5", "str true"]); }
#[test] fn empty_program_loads() { let p = Program::load(&Asm::default().halt().file(), SCHEMA, &registry()).unwrap(); assert_eq!((p.constant_count(), p.code_size()), (0, 1)); }
#[test] fn bad_magic() { assert!(load_err(&Asm::default().halt().file_v(SCHEMA, VERSION, b"XXXX")).contains("bad magic")); }
#[test] fn other_version() { assert!(load_err(&Asm::default().halt().file_v(SCHEMA, 2, b"DSLB")).contains("unsupported bytecode version 2 (this runtime reads version 3)")); }
#[test] fn schema_mismatch() { assert!(load_err(&Asm::default().halt().file_v(b"other", VERSION, b"DSLB")).contains("schema mismatch")); }
#[test] fn truncated() { let f = Asm::default().ctx("t").halt().file(); assert!(load_err(&f[..f.len() - 3]).contains("bytecode is truncated")); }
#[test] fn trailing_bytes() { let mut f = Asm::default().halt().file(); f.push(0); assert!(load_err(&f).contains("trailing byte")); }
#[test] fn constant_out_of_range() { assert!(load_err(&Asm::default().u8(0x02).u16(9).halt().file()).contains("constant index 9 out of range")); }
#[test] fn local_out_of_range() { assert!(load_err(&Asm::default().u8(0x04).u16(0).halt().file()).contains("local slot 0 out of range")); }
#[test] fn undeclared_context() { assert!(load_err(&Asm::default().ctx("nope").halt().file()).contains("context variable 'nope' is not declared")); }
#[test] fn unbound_function() { assert!(load_err(&Asm::default().func("missing", 0).halt().file()).contains("host function 'missing' is not bound")); }
#[test] fn function_arity() { assert!(load_err(&Asm::default().push_int(1).func("twice", 2).halt().file()).contains("takes 1 argument(s), bytecode passes 2")); }
#[test] fn unbound_method() { assert!(load_err(&Asm::default().ctx("root").method("Thing", "name", 0).halt().file()).contains("method 'Thing.name' is not bound")); }
#[test] fn method_arity() { assert!(load_err(&Asm::default().ctx("root").push_int(1).method("Thing", "id", 1).halt().file()).contains("method 'Thing.id' takes 0 argument(s), bytecode passes 1")); }
#[test] fn jump_into_operand() { assert!(load_err(&Asm::default().u8(0x0C).i32(1).halt().file()).contains("not an instruction start")); }
#[test] fn backward_jump() { assert!(load_err(&Asm::default().u8(0x0C).i32(-5).halt().file()).contains("goes backwards")); }
#[test] fn missing_halt() { assert!(load_err(&Asm::default().push_int(1).file()).contains("does not end with HALT")); }
#[test] fn unknown_opcode() { assert!(load_err(&Asm::default().u8(0x7f).halt().file()).contains("unknown opcode 127")); }
#[test] fn bad_bool_operand() { assert!(load_err(&Asm::default().u8(0x03).u8(2).halt().file()).contains("PUSH_BOOL operand must be 0 or 1")); }
#[test] fn too_many_locals() { let mut a = Asm::default(); a.locals = 5000; assert!(load_err(&a.halt().file()).contains("too many locals")); }
#[test] fn invalid_utf8_constant() { let mut f = Asm::default().push_str("ab").halt().file(); let p = f.windows(2).position(|w| w == b"ab").unwrap(); f[p] = 0xff; assert!(load_err(&f).contains("not valid UTF-8")); }
#[test] fn int_division_by_zero() { assert!(run_err(&Asm::default().push_int(1).push_int(0).u8(0x16).halt().file()).contains("int division by zero")); }
#[test] fn int_overflow() { assert!(run_err(&Asm::default().push_int(i32::MAX).push_int(1).u8(0x13).halt().file()).contains("int overflow")); }
#[test] fn negating_smallest_int() { assert!(run_err(&Asm::default().push_int(i32::MIN).u8(0x1B).halt().file()).contains("negating the smallest int")); }
#[test] fn mixed_kind_equality() { assert!(run_err(&Asm::default().push_int(1).push_str("x").u8(0x09).halt().file()).contains("cannot compare int with string")); }
#[test] fn float_op_on_int() { assert!(run_err(&Asm::default().push_int(1).push_float(1.0).u8(0x17).halt().file()).contains("float operator applied to int and float")); }
#[test] fn local_read_before_store() { let mut a = Asm::default(); a.locals = 1; assert!(run_err(&a.u8(0x04).u16(0).halt().file()).contains("read before assignment")); }
#[test] fn non_bool_condition() { assert!(run_err(&Asm::default().push_int(1).u8(0x0D).i32(0).halt().file()).contains("condition is a int, not a bool")); }
#[test] fn stack_underflow() { assert!(run_err(&Asm::default().u8(0x10).halt().file()).contains("stack underflow")); }
#[test] fn host_error_propagates() { assert!(run_err(&Asm::default().func("boom", 0).halt().file()).contains("boom from the host")); }
#[test] fn context_object_missing_at_run() { let reg = registry(); let p = Program::load(&Asm::default().ctx("t").halt().file(), SCHEMA, &reg).unwrap(); let mut empty = Context::new(); assert!(p.run(&mut empty, &reg).unwrap_err().message.contains("context variable 't' was not provided by the host for this run")); }
#[test] fn method_on_wrong_object_type() { assert!(run_err(&Asm::default().ctx("t").method("Thing", "id", 0).halt().file()).contains("method Thing.id called on a Trace")); }
#[test] fn operand_stack_limit() { let mut a = Asm::default(); for _ in 0..=MAX_STACK { a = a.push_int(1); } assert!(run_err(&a.halt().file()).contains("operand stack exceeds 1024 values")); }
#[test] fn concatenation_size_limit() { let mut a = Asm::default().push_str(&"x".repeat(65535)); for _ in 0..9 { a = a.u8(0x0F).u8(0x26); } assert!(run_err(&a.halt().file()).contains("string + result exceeds")); }
#[test] fn every_truncation_and_bit_flip_is_an_error_not_a_panic() {
    let f = Asm::default().ctx("t").push_int(1).push_int(2).u8(0x13).method("Trace", "int", 1).push_bool(true).u8(0x0D).i32(0).halt().file();
    for n in 0..f.len() { assert!(Program::load(&f[..n], SCHEMA, &registry()).is_err(), "truncation at {} must fail", n); }
    for i in 0..f.len() { for bit in [0x01u8, 0x10, 0x80] { let mut g = f.clone(); g[i] ^= bit; let _ = run(&g); } }  // any Result is fine; a panic is not
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
    assert_eq!(Program::load(&Asm::default().func("missing", 0).halt().file(), SCHEMA, &registry()).unwrap_err().code, "LOAD_UNBOUND");
    assert_eq!(run(&Asm::default().func("boom", 0).halt().file()).unwrap_err().code, "RUN_HOST");
    assert_eq!(run(&Asm::default().push_int(1).push_int(0).u8(0x16).halt().file()).unwrap_err().code, "RUN_ERROR");
}
#[test] fn result_line_exit_codes() { assert_eq!(report_error("load", &Error::new("LOAD_UNBOUND", "x")), 3); assert_eq!(report_error("run", &Error::host("no")), 3); assert_eq!(report_app_failure("nothing"), 4); assert_eq!(report_ok("42"), 0); assert_eq!(json_escape("a\"b\n"), "\"a\\\"b\\n\""); }
