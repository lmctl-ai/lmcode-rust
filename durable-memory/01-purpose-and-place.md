# Purpose and place

`lmcode-rust` is the dependency-free Rust runtime for lmscript bytecode v6 (rule entries, R49). The workspace contains the `dslvm` library, a shared-vector conformance runner, and an example HTTP router host.

`dslvm/src/lib.rs` contains the value and context types, typed host registry, bytecode loader, structural verifier, and stack VM. `dslvm/src/sha256.rs` binds bytecode to exact schema bytes. Optional host functions live in `dslvm/src/stdlib.rs`, with schema fragments under `stdlib/`. Integration tests live in `dslvm/tests/`.

`conformance/src/main.rs` runs the pinned fixtures in `conformance/vectors/`; `scripts/sync-vectors.sh --verify` checks their digest against `vectors.lock`. `examples/router/` contains the HTTP host, its embedded schema, and end-to-end checks.

The schema names context objects, methods, functions, conversions, operators, and enums. Hosts provide their implementations and retain control over process state and domain objects; the runtime does not infer domain behavior from schema JSON.
