//! Serves a compiled program's REST routes over stdin/stdout (the opt-in `rest` std module; no network).
//!   rest-serve PROGRAM.dslb HOST.schema.json [--max-body-bytes N]
//! stdout carries only HTTP responses; handler failures go to stderr. Exit 0 after serving, 2 usage, 3 error.
use dslvm::rest::{register_rest, RestOptions};
use dslvm::{Context, Program, Registry};

fn serve(args: &[String]) -> Result<(), dslvm::Error> {
    let mut options = RestOptions::default();
    if let [flag, value] = &args[2..] {
        if flag != "--max-body-bytes" { return Err(dslvm::Error::host(format!("unknown flag {}", flag))); }
        options.max_body_bytes = value.parse().map_err(|_| dslvm::Error::host("--max-body-bytes needs a number"))?;
    }
    let read = |path: &str| std::fs::read(path).map_err(|e| dslvm::Error::host(format!("{}: {}", path, e)));
    let mut registry = Registry::new();
    let mut rest = register_rest(&mut registry, options)?;
    let program = Program::load(&read(&args[0])?, &read(&args[1])?, &registry)?;
    rest.run(&program, &registry, &mut Context::new())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 || (args.len() != 2 && args.len() != 4) {
        eprintln!("usage: rest-serve PROGRAM.dslb HOST.schema.json [--max-body-bytes N]");
        std::process::exit(2);
    }
    if let Err(error) = serve(&args) { eprintln!("rest-serve: {}", error.message); std::process::exit(3); }
}
