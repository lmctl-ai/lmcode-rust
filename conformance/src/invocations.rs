//! Native fixture adapter, not a shipped application interface.
use dslvm::stdlib::{failure_frame, InvocationResults, ResultContract};
use dslvm::{Context, Error, Program, Registry, Value};
use std::{cell::Cell, rc::Rc};
fn main() {
    let mut out = std::io::stdout().lock();
    let code = match run(&mut out) {
        Ok(code) => code,
        Err(e) => failure_frame(&mut out, "cli", &e).unwrap_or(4),
    };
    std::process::exit(code);
}
fn run(out: &mut impl std::io::Write) -> Result<i32, Error> {
    let args = std::env::args_os()
        .skip(1)
        .map(|s| {
            s.into_string()
                .map_err(|_| Error::new("CLI_USAGE", "lmcode invocation: file must be UTF-8"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if args.len() < 3 {
        return Err(Error::new("CLI_USAGE", "fixture arguments required"));
    }
    let config =
        std::fs::read_to_string(&args[1]).map_err(|_| Error::host("cannot read fixture config"))?;
    // Config is a pinned test fixture, separate from the production metadata reader.
    let order = config
        .split('[')
        .nth(1)
        .and_then(|s| s.split(']').next())
        .ok_or_else(|| Error::host("missing execution array"))?;
    let order = order
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.trim()
                .parse::<usize>()
                .map_err(|_| Error::host("invalid fixture index"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let fixture_dry = config
        .split_whitespace()
        .collect::<String>()
        .contains("\"dry\":true");
    let mut results = InvocationResults::from_command(&args[2..])?;
    if results.dry() != fixture_dry {
        return Err(Error::host(
            "fixture dry mode disagrees with invocation metadata",
        ));
    }
    let mut registry = Registry::new();
    let counter = Rc::new(Cell::new(0));
    registry.function("next_counter", &[], "int", move |_, _| {
        counter.set(counter.get() + 1);
        if let Ok(path) = std::env::var("LMCODE_REAL_SENTINEL") {
            std::fs::write(path, b"real callback ran").map_err(|e| Error::host(e.to_string()))?;
        }
        Ok(Value::Int(counter.get()))
    })?;
    let dry_calls = Rc::new(Cell::new(0));
    let preview_calls = Rc::clone(&dry_calls);
    registry.function_with_dry(
        "entry_fail",
        &[],
        "void",
        |_, _| Err(Error::host("native entry failure")),
        dslvm::dry(move |_: &mut Context, _: &dslvm::Args| {
            preview_calls.set(preview_calls.get() + 1);
            Err(Error::host("preview failed"))
        }),
    )?;
    registry.function("wp90_Json", &[], "Json", |_, _| Err(Error::host("real dummy callback ran")))?;
    registry.function("wp90_consume", &["Json"], "int", |_, _| Err(Error::host("real consumer ran")))?;
    registry.method("Json", "wp90_consume", &["Json"], "int", |_, _, _| Err(Error::host("real method ran")))?;
    registry.method_with_dry("Json", "wp90_preview", &["Json"], "int", |_, _, _| Err(Error::host("real method ran")), dslvm::dry(|_: &mut Context, _, _: &dslvm::Args| Err(Error::host("preview received dry null"))))?;
    let schema = std::fs::read(&args[0]).map_err(|_| Error::host("cannot read fixture schema"))?;
    let contract = ResultContract::from_schema(&schema)?;
    let mut ctx = Context::new();
    for index in order {
        let mut phase = "load";
        let mut attempt = || -> Result<(), Error> {
            let file = &results
                .registrations()
                .iter()
                .find(|r| r.index == index)
                .ok_or_else(|| Error::new("CLI_USAGE", "fixture index absent"))?
                .file;
            let bytes = std::fs::read(file)
                .map_err(|_| Error::new("LOAD_BAD_BYTECODE", "cannot read fixture"))?;
            let p = Program::load(&bytes, &schema, &registry)?;
            results.select(index, &mut ctx, contract.clone())?;
            phase = "run";
            p.run(&mut ctx, &registry)?;
            for i in 0..p.entries().len() {
                if p.call(i, &mut ctx, &registry, vec![])? != Value::Int(7) {
                    return Err(Error::host("entry HALT result changed"));
                }
            }
            results.complete(index)
        };
        if let Err(e) = attempt() {
            results.fail(Some(index), phase, e)?;
            break;
        }
    }
    if config
        .split_whitespace()
        .collect::<String>()
        .contains("\"app_fail\":true")
    {
        results.fail(
            None,
            "app",
            Error::new("APP_FAIL", "native validation failed"),
        )?;
    }
    if let Ok(path) = std::env::var("LMCODE_DRY_CALLS_FILE") {
        std::fs::write(path, dry_calls.get().to_string())
            .map_err(|e| Error::host(e.to_string()))?;
    }
    Ok(results.write_frame(out))
}
