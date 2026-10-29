use dslvm::{Context, Program, Registry, Value};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).expect("usage: rule-entries FILE.dslb");
    let registry = Registry::new();
    let program = Program::load(&std::fs::read(path)?, include_bytes!("../schema.json"), &registry)?;
    let (index, entry) = program.entries().iter().enumerate()
        .find(|(_, entry)| entry.annotation == "increment").expect("@increment entry");
    assert_eq!(entry.params, ["int"]);
    let value = program.call(index, &mut Context::new(), &registry, vec![Value::Int(41)])?;
    assert_eq!(value.as_int()?, 42);
    println!("PASS @increment implements Score: 41 -> 42");
    Ok(())
}
