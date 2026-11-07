# REST server over stdio (`rest` std module)

Opt-in module of [spec/std-rest.md](https://github.com/lmctl-ai/lmcode/blob/main/spec/std-rest.md); `Registry::new()` still has
exactly the 206 default signatures. There is no network listener: the process reads HTTP/1.1 requests from stdin and writes
responses to stdout.

```rust
use dslvm::rest::{register_rest, RestOptions};
use dslvm::{Context, Program, Registry};

let mut registry = Registry::new();
let mut rest = register_rest(&mut registry, RestOptions {
    max_body_bytes: 1 << 20,
    on_handler_error: Some(std::rc::Rc::new(|entry, msg| eprintln!("{entry}: {msg}"))),
    ..RestOptions::default()          // input/output None = process stdin/stdout
})?;
// host schema = lmcode-rest.schema.json (+ your own); the program calls rest_route(...) then rest_serve()
let program = Program::load(&bytecode, &host_schema_bytes, &registry)?;
rest.run(&program, &registry, &mut Context::new())?;   // runs the body, then serves until EOF, rest_stop() or a framing error
```

* `rest.dispatch(&program, &registry, &mut ctx, &request)` is the transport-independent core used by the pump.
* Everything runs on the calling thread; handlers run one at a time and never nest inside the body's run.
* While serving, stdout belongs to the protocol: send diagnostics (`print`) to stderr.
* `cargo run -p dslvm --bin rest-serve -- PROGRAM.dslb HOST.schema.json [--max-body-bytes N]` serves a compiled program over
  stdin/stdout (exit 0 after serving, 2 usage, 3 error).
* Tests: `cargo test -p dslvm rest` (unit tests in `dslvm/src/rest/tests.rs`; `dslvm/tests/rest_suite.rs` replays the pinned
  shared suite in `conformance/rest-suite` in memory and through the `rest-serve` child process). `scripts/sync-rest.sh`
  re-copies the suite from `../lmcode` and rewrites `rest-suite.lock`; `--verify` checks the copy (run by CI).
