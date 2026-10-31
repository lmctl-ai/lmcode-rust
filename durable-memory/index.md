# lmcode-rust notes

Rust runtime and SDK for lmscript `_lmcode_` bytecode. The workspace provides a dependency-free loader, structural verifier, stack VM, typed host registry, and shared conformance runner.
Repository: https://github.com/lmctl-ai/lmcode-rust. Crate: `dslvm`. Toolchain: Rust 1.80+.

## Document map

| File | Contents |
| --- | --- |
| [01-purpose-and-place.md](01-purpose-and-place.md) | Workspace layout, dslvm crate, router host example |
| [02-decisions.md](02-decisions.md) | Schema 2 arrays, overloads, single-step conversions |
| [03-build-run-test.md](03-build-run-test.md) | Building, running workspace tests, router end-to-end checks |
| [04-gotchas.md](04-gotchas.md) | Exact schema hash binding, opaque handles, resource policy |
| [05-open-items.md](05-open-items.md) | Planned improvements and multi-runtime parity |
| [06-stdlib-catalog.md](06-stdlib-catalog.md) | Standard library math, string, and collections catalogs |
| [09-result-sink.md](09-result-sink.md) | Invocation-owned result sink, return signatures, validation, and frame contract |

## Adding an example or host function

### Adding an example
1. Create `examples/<name>/` with a Rust embedding crate (`Cargo.toml`, `src/main.rs`, and `.lms` script).
2. Compile bytecode with `lmscript <script>.lms <output>.dslb`.
3. Add verification script `test.sh` in the example directory.

### Adding a host function/method in Rust
1. Use `reg = Registry::new()`.
2. Register methods: `reg.register_method(type, name, param_types, return_type, callback)`.
3. Register functions: `reg.register_function(name, param_types, return_type, callback)`.
4. Register context objects: `reg.register_context(name, type)`.

## Releases
Releases are built and published only from an explicitly dispatched workflow; pushes do not run CI or publish.
Platform binaries: `router-linux-x64`, `router-linux-arm64`, `router-macos-arm64`, `router-macos-x64`, `router-windows-x64.exe`.
- [update-sample-suite.md](update-sample-suite.md): canonical sample-suite update and all-port verification rule.

- [Rule entries v6](07-rule-entries.md): R49 loader/API contract, gates and vector provenance; supersedes earlier v5-only notes.

- [lmscript tests and releases](08-lmscript-releases.md)
