//! Performance sanity: load one conformance vector and run it many times with a discarding stub. Not a benchmark suite;
//! a number to compare with the C++ runtime on the same bytecode. Usage: cargo run --release -p dsl-conformance --bin bench [VECTOR] [RUNS]
use dslvm::{Context, Program, Registry, Value};
use std::path::Path;
struct Thing { id: i64 }
fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("vectors");
    let name = std::env::args().nth(1).unwrap_or_else(|| "int-arith".into());
    let runs: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(200_000);
    let bytecode = std::fs::read(dir.join(format!("{}.dslb", name))).expect("vector");
    let schema = std::fs::read(dir.join("schema.json")).expect("schema");
    let mut reg = Registry::new();
    reg.context("t", "Trace").unwrap().context("root", "Thing").unwrap();
    for (m, t) in [("int", "int"), ("float", "float"), ("str", "string"), ("bool", "bool"), ("thing", "Thing")] { reg.method("Trace", m, &[t], "void", |_, _, _| Ok(Value::None)).unwrap(); }
    reg.method("Thing", "id", &[], "int", |ctx, this, _| Ok(Value::Int(ctx.get::<Thing>(this)?.id))).unwrap();
    reg.method("Thing", "next", &[], "Thing", |ctx, this, _| { let id = ctx.get::<Thing>(this)?.id; let r = ctx.insert("Thing", Thing { id: id + 1 }); Ok(Value::object("Thing", r)) }).unwrap();
    reg.method("Thing", "name", &[], "string", |ctx, this, _| Ok(Value::string(format!("thing-{}", ctx.get::<Thing>(this)?.id)))).unwrap();
    reg.method("Thing", "scaled", &["float"], "float", |ctx, this, a| Ok(Value::Float(ctx.get::<Thing>(this)?.id as f64 * a[0].as_float()?))).unwrap();
    reg.function("thing", &["int"], "Thing", |ctx, a| { let r = ctx.insert("Thing", Thing { id: a[0].as_int()? }); Ok(Value::object("Thing", r)) }).unwrap();
    reg.function("twice", &["int"], "int", |_, a| Ok(Value::Int(a[0].as_int()? * 2))).unwrap();
    reg.function("bound", &["int"], "int", |_, a| Ok(Value::Int(a[0].as_int()?))).unwrap();
    reg.function("half", &["float"], "float", |_, a| Ok(Value::Float(a[0].as_float()? / 2.0))).unwrap();
    reg.function("yes", &[], "bool", |_, _| Ok(Value::Bool(true))).unwrap();
    reg.function("fail", &["string"], "void", |_, _| Ok(Value::None)).unwrap();
    let program = Program::load(&bytecode, &schema, &reg).expect("load");
    // Count statically encoded operations once. A loop executes its body repeatedly, so this is not a dynamic
    // instruction count and is intentionally not multiplied into the throughput report.
    let static_ops = count_ops(&bytecode);
    let t0 = std::time::Instant::now();
    for _ in 0..runs { let mut ctx = Context::new(); ctx.set_object("t", "Trace", ()); ctx.set_object("root", "Thing", Thing { id: 0 }); program.run(&mut ctx, &reg).expect("run"); }
    let secs = t0.elapsed().as_secs_f64();
    println!("rust {}: {} runs x {} static ops in {:.3} s = {:.0} runs/s", name, runs, static_ops, secs, runs as f64 / secs);
}
fn count_ops(file: &[u8]) -> usize {  // v5 header: constants, imports, contexts, locals, max_stack, then code
    let mut p = 38;
    let n = u32::from_be_bytes(file[p..p + 4].try_into().unwrap()) as usize; p += 4;
    for _ in 0..n { assert_eq!(file[p], 1, "bench only understands string constant tags"); let l = u32::from_be_bytes(file[p + 1..p + 5].try_into().unwrap()) as usize; p += 5 + l; }
    let n = u16::from_be_bytes(file[p..p + 2].try_into().unwrap()) as usize; p += 2 + 4 * n;
    let n = u16::from_be_bytes(file[p..p + 2].try_into().unwrap()) as usize; p += 2 + 8 * n;
    let n = u16::from_be_bytes(file[p..p + 2].try_into().unwrap()) as usize; p += 2 + 5 * n + 2;
    let len = u32::from_be_bytes(file[p..p + 4].try_into().unwrap()) as usize; p += 4;
    let code = &file[p..p + len]; let mut i = 0; let mut ops = 0;
    while i < code.len() { ops += 1; i += 1 + match code[i] { 0x01 | 0x12 => 8, 0x02 => 4, 0x04..=0x06 => 2, 0x03 => 1, 0x07 | 0x08 => 3, 0x0C..=0x0E | 0x35 | 0x36 => 4, 0x2F => 8, 0x31 => 6, 0x32 => 1, _ => 0 }; }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn while_operands_are_not_counted_as_instructions() {
        // while false {}: begin, push false, test/exit, back-edge, halt.
        let code = [0x34, 0x03, 0, 0x35, 0, 0, 0, 5, 0x36, 0xff, 0xff, 0xff, 0xf3, 0x11];
        let mut file = b"DSLB".to_vec();
        file.extend_from_slice(&dslvm::VERSION.to_be_bytes());
        file.extend_from_slice(&dslvm::sha256(b""));
        file.extend_from_slice(&0u32.to_be_bytes()); // constants
        for _ in 0..3 { file.extend_from_slice(&0u16.to_be_bytes()); } // imports, contexts, locals
        file.extend_from_slice(&1u16.to_be_bytes()); // stack hint
        file.extend_from_slice(&(code.len() as u32).to_be_bytes());
        file.extend_from_slice(&code);
        file.extend_from_slice(&0u32.to_be_bytes()); // line table
        let registry = Registry::new();
        Program::load(&file, b"", &registry).unwrap().run(&mut Context::new(), &registry).unwrap();
        assert_eq!(count_ops(&file), 5);
    }
}
