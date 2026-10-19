# rust-dsl-sdk

Status: experimental; bytecode compatibility is not promised. Requires a Rust toolchain; the router checks also use curl and lmscript.

`rust-dsl-sdk` is the Rust runtime for lmscript bytecode v5. It provides the `dslvm`
crate, a loader, structural verifier, stack VM, typed host registry, and a conformance
runner over the shared SDK vectors. The workspace has no external Rust crate
dependencies.

Install the released compiler for the router example:

```sh
npm install -g @lmctl-ai/lmctl@0.2.347
```

Build and run the checks:

```sh
cargo test --workspace
bash examples/router/test.sh
```

The test requires `lmscript` 0.2.346 or newer from `PATH`. It compiles the public
router example, runs it on the Rust host, checks the shared vectors, and uses a sibling
`lmweb` checkout when available for cross-runtime comparison. The test accepts
`LMWEB_DIR`; before the repository move it also recognizes the legacy sibling checkout.

The runtime API is small: register context objects, typed methods, and typed host
functions; load bytecode with the exact schema bytes; create a host context; and run
the program. Handles remain owned by the host. Schema 2 supports overloads, declared
conversions, operators, enums, and `returns: "self"`; compiler-inlined functions do
not add a runtime function table.

The common language includes typed locals, branching, range and collection loops,
`foreach`, `switch`, `while`, `break`, `continue`, and source-located runtime errors.
Collections remain schema objects with methods such as `len()` and `at(i)`. The Rust
runtime and the C++ reference runtime consume the same portable schema-bound format.

The workspace metadata identifies the public repository and MIT license. See
[LICENSE](LICENSE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Related
projects are [cpp-dsl-sdk](https://github.com/lmctl-ai/cpp-dsl-sdk),
[lmapi](https://github.com/lmctl-ai/lmapi), [lmrust](https://github.com/lmctl-ai/lmrust),
and [lmcpp](https://github.com/lmctl-ai/lmcpp).
