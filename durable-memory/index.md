# lmcode-rust public facts

Rust bytecode runtime for the lmscript `_dsl_` framework. The crate provides a dependency-free loader, structural verifier, stack VM, typed host registry, and shared conformance runner.

Status: experimental public v5 integration. Install `@lmctl-ai/lmctl` 0.2.346 or newer; the target PATH release is 0.2.347. No source checkout override is required for the normal test command.

The runtime format is owned by the public `lmcode-cpp` and `lmcode-rust` projects. Bytecode is schema-bound by SHA-256, big-endian, and intentionally has no backward compatibility promise. Schema 2 uses signature arrays, overloads, declared one-step conversions, enums, operators, and `returns: "self"`.

The DSL has no bytecode collection values. Hosts expose collections as schema objects with `len()` and `at(i)`. The common language includes loops, compiler-inlined functions, `foreach`, `switch`, `while`, `break`, `continue`, and common numeric math. Source line information is carried into runtime errors by the v5 SDK.

## Public files

- `README.md`: setup, embedding, and examples.
- `Cargo.toml`: workspace for `dslvm`, `conformance`, and `examples/router`.
- `dslvm/src/lib.rs`: registry, loader, verifier, and VM.
- `dslvm/src/stdlib.rs` and `stdlib/`: optional host functions and schema fragments.
- `examples/router/test.sh`: end-to-end checks using the released compiler on `PATH`.
- `conformance/vectors/` and `vectors.lock`: pinned cross-runtime fixtures.
- `durable-memory/`: public design facts and operational notes.
- `LICENSE`: MIT License.
- `THIRD_PARTY_NOTICES.md`: dependency and attribution notes.

Public repository: https://github.com/lmctl-ai/lmcode-rust
