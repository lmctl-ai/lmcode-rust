# rust-dsl-sdk

The Rust runtime for lmscript `_dsl_` bytecode (format v5): crate **`dslvm`** (loader, structural verifier, stack VM, host registry),
the **conformance runner** over the `cpp-dsl-sdk` vectors (the contract between runtimes), and an example host **`examples/router`**
that serves the same schema and the same bytecode file as poc-web's C++ server. Second runtime after the C++ reference
`lmctlhq/cpp-dsl-sdk`; same semantics and error classes; **no external crates** (SHA-256 is in the crate, checked against FIPS vectors).
Status: EXPERIMENTAL (operator requirement R21; team guide lmctl-mono `lmscript/durable-memory/dsl-framework.md`; spec
`lmscript/durable-memory/design/dsl-language.md`). Rust 1.92, edition 2021.

## Embedding it in twenty lines

```rust
use dslvm::{Context, Program, Registry, Value};
struct Greeter { last: String }

fn main() -> Result<(), dslvm::Error> {
    let mut reg = Registry::new();
    reg.context("g", "Greeter")?;                                                    // the schema's context variable and its type
    reg.method("Greeter", "say", &["string"], "void", |ctx, this, a| {               // Greeter.say(string) -> void, declared as in the schema
        ctx.get_mut::<Greeter>(this)?.last = a[0].as_str()?.to_string(); Ok(Value::None) })?;
    reg.function("now", &[], "string", |_, _| Ok(Value::Str("2026-10-18".into())))?; // now() -> string
    if std::env::args().nth(1).as_deref() == Some("--dsl-signatures") { for s in reg.signatures() { println!("{}", s); } return Ok(()); }

    let bytecode = std::fs::read("prog.dslb").unwrap(); let schema = std::fs::read("greeter.dsl.json").unwrap();
    let program = Program::load(&bytecode, &schema, &reg)?;                         // refused before anything runs

    let mut ctx = Context::new();                                                   // the objects of ONE run
    let g = ctx.set_object("g", "Greeter", Greeter { last: String::new() });
    program.run(&mut ctx, &reg)?;                                                   // the chunk is the callback
    println!("{}", ctx.take::<Greeter>(g)?.last);                                   // the host continues with what the chunk did
    Ok(())
}
```

`Registry` is declared with the schema's types (parameter type names and the return type); entries are keyed by complete canonical
signatures, so a name may have several overloads with distinct parameter tuples while duplicate parameter tuples are refused.
`load()` binds the exact canonical signature and refuses a file whose import the registry does not provide, before anything runs.
`reg.signatures()` lists the sorted
canonical strings (`context g: Greeter`, `fn now() -> string`, `method Greeter.say(string) -> void`), exactly what
`lmscript --dsl-signatures SCHEMA` prints, so a host test diffs the two sources of truth (spec v4 S0, A02). It is a consistency
check, not a defense: per operator decisions R33/R34 the VM performs no call-time type checks. `Value` is `None | Int(i64) |
Float(f64) | Str | Bool | Obj{type_name, obj}`; a void method returns `Value::None`; host code returns `Err(dslvm::Error::host(..))`
for a run-time failure and the VM propagates it.

**Result line and exit codes (spec v5 D7, R26).** `Error { code, message }` with codes `LOAD_BAD_BYTECODE`, `LOAD_SCHEMA_MISMATCH`,
`LOAD_UNBOUND`, `RUN_ERROR` (VM), `RUN_HOST` (host closure), `APP_FAIL`; CLI hosts print exactly one JSON line through
`report_ok(json)`, `report_error("load"|"run", &e)` (returns 3) or `report_app_failure(msg)` (returns 4) and exit with the returned
code; compile failures are lmscript's exit 2.

## Host-object ownership (why `Context` is an arena)

A `Context` owns every object of one run as `Box<dyn Any>`; the DSL only sees an `ObjRef` (index plus schema type name).
Methods receive `&mut Context` and borrow their receiver with `ctx.get_mut::<T>(this)`, functions receive `&mut Context` to
create new objects (`ctx.insert`). One mutable path, no `Rc<RefCell<_>>`, no lifetimes in the public API, and handles are valid
for exactly one run, which is what the language promises (R12: the host owns lifetime; the DSL cannot store or return handles).
The host takes its objects back with `ctx.take::<T>(obj)` or drops the context.

## Versioning rule

`dslvm::VERSION` is the bytecode format version this runtime reads; the crate's major version equals it (5.x). The loader refuses
every other version: there is no backward compatibility. When the compiler's format changes, compiler, both runtimes and every
host move together; the history is in the living spec.

## What load() guarantees (identical to the C++ runtime)

Wrong magic, another version (v4 included), a schema-hash mismatch, truncation, trailing bytes, unknown constant tags, undecodable
instructions, non-UTF-8 constants, out-of-range constant/local/import/context indexes, calls whose import kind or arity differ, ordinary jumps
that miss an instruction start, go backwards, or cross a structured loop region, malformed FOR_RANGE/FOR_NEXT and WHILE_BEGIN/WHILE_TEST/LOOP_BACK pairs, a missing HALT, and context
declarations or import signatures the registry does not provide (the message names the host's own signature) are refused with a
message naming the problem. `run()` retains existing value-kind errors; every-truncation tests cover structural loading failures
and imposes no limits (operator decision 2026-10-18: no stack, locals or string cap; `max_stack` is a reservation hint). Ints are
i64 with wrapping arithmetic (R34); division and remainder by zero are the run-time errors.

`MATH (0x32)` is the compiler's built-in numeric library. Its byte operand selects sin, cos, sqrt, abs, floor, pow,
min, max, clamp, lerp, smoothstep/ease variants, truncating `int(float)`, or the integer abs/min/max/clamp forms.
The VM uses the platform Rust/libm implementations for transcendental functions; shared vectors allow finite sin/cos/pow
trace values within relative tolerance `1e-12`. Float min/max propagate NaN and retain the first equal operand, including
signed zero; integer abs wraps at `i64::MIN`.

## Tests and conformance

    cargo test                               # dslvm: 2 unit + 68 integration tests (mirror of the C++ checks plus robustness, signatures, codes);
                                             # dsl-conformance: vectors.lock pin check + all 43 vectors + a negative control
    cargo run -p dsl-conformance --bin dsl-conformance # prints ok/FAIL per vector: PASS 43/43
    cargo run --release -p dsl-conformance --bin bench [VECTOR] [RUNS]   # runs/s on one vector

`conformance/vectors` is a verbatim copy of `cpp-dsl-sdk/conformance/vectors` pinned by `vectors.lock` (SDK commit + sha256 over
every file); `scripts/sync-vectors.sh` refreshes it from a sibling checkout `../cpp-dsl-sdk` (or `CPP_DSL_SDK=path`) and
`--verify` runs inside `cargo test`. A new compiler release regenerates the vectors in the C++ repo first; this repo then syncs.

Historical v3 performance sanity (release build, this shared 4-core box, 2026-10-18, 200,000 runs of one vector each, discarding stub):

| vector | Rust runs/s | C++ runs/s | Rust instructions/s |
| --- | --- | --- | --- |
| int-arith (39 instructions) | 883,000 | 893,000 | 34 M |
| handles-and-chains (36) | 468,000 | 626,000 | 17 M |
| strings (27) | 832,000 | 1,065,000 | 23 M |

Rust is within 1.0 to 1.3x of C++; the gap is `String` cloning on `PUSH_STR` and `Box<dyn Any>` allocation per handle versus the
C++ arena of raw pointers. Nothing was tuned.

## The example host

`examples/router`: a single-threaded `std::net` HTTP/1.1 server that knows nothing about routing. `schema/router.dsl.json` and
`dsl/build_router.lms` are byte-identical copies of poc-web's files, so one bytecode file runs on both runtimes:

    cd examples/router && lmscript dsl/build_router.lms build/router.dslb            # released lmscript >= 0.2.328
    cargo run -p router -- --port 0 --dsl build/router.dslb --schema schema/router.dsl.json
    curl -i http://127.0.0.1:PORT/            # 200, today's date      curl -i -X POST http://127.0.0.1:PORT/   # 405
    bash examples/router/test.sh              # 11 checks, including the same bytecode on ../poc-web/build/server when present

## Depending on it

A host team adds `dslvm = { path = "../rust-dsl-sdk/dslvm" }` (sibling checkout) or
`dslvm = { git = "https://github.com/lmctlhq/rust-dsl-sdk", branch = "main" }` to its `Cargo.toml` and commits the resolved revision in `Cargo.lock`; no v4 release tag is published by this slice. The crate has no dependencies
and no features. Workspace layout: `dslvm/` (the library), `conformance/` (runner, `bench`, pinned vectors), `examples/router/`
(host example with its own `test.sh`), `scripts/sync-vectors.sh`, `vectors.lock`, `durable-memory/` (team memory, start at its index).
