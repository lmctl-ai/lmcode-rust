# lmcode-rust

Status: experimental; bytecode compatibility is not promised. Requires a Rust toolchain; the router checks also use curl and lmscript.

`lmcode-rust` is the Rust runtime for lmscript bytecode v5. It provides the `dslvm`
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

The test requires `lmscript` (or `lmctl`) 0.2.346 or newer from `PATH`. It compiles the public
router example, runs it on the Rust host, checks the shared vectors, and uses a sibling
`lmweb` checkout when available for cross-runtime comparison. The test accepts
`LMWEB_DIR`; before the repository move it also recognizes the legacy sibling checkout.

The router follows the DSL host flag standard (lmscript R42). Its schema is embedded at
build time (`include_bytes!`), and bytecode is verified against those exact bytes:

```sh
router --dsl FILE [--port N]     # load, verify against the embedded schema, serve
router --dsl-schema              # human summary: context/fn lines, then Owner: and "  Owner.method(...)" lines
router --dsl-schema --json       # the embedded schema bytes, exactly
router --dsl-signatures          # one canonical signature per line (as `lmscript --dsl-signatures SCHEMA`)
```

The former `--schema FILE` flag is refused. Any other combination of these flags exits 2
with empty stdout. `Registry::schema_summary()` produces the summary, so every Rust host
prints the same format.

The runtime API is small: register context objects, typed methods, and typed host
functions; load bytecode with the exact schema bytes (embed them in the host binary); create a host context; and run
the program. Handles remain owned by the host. Schema 2 supports overloads, declared
conversions, operators, enums, and `returns: "self"`; compiler-inlined functions do
not add a runtime function table.

The common language includes typed locals, branching, range and collection loops,
`foreach`, `switch`, `while`, `break`, `continue`, and source-located runtime errors.
Collections remain schema objects with methods such as `len()` and `at(i)`. The Rust
runtime and the C++ reference runtime consume the same portable schema-bound format.

The workspace metadata identifies the public repository and MIT license. See
[LICENSE](LICENSE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Related
projects are [lmcode-cpp](https://github.com/lmctl-ai/lmcode-cpp),
[lmapi](https://github.com/lmctl-ai/lmapi), [lmrust](https://github.com/lmctl-ai/lmrust),
and [lmcpp](https://github.com/lmctl-ai/lmcpp).

## Sibling runtimes

The same bytecode v5 and the same conformance vectors are implemented by [lmcode-cpp](https://github.com/lmctl-ai/lmcode-cpp),
[lmcode-rust](https://github.com/lmctl-ai/lmcode-rust), [lmcode-go](https://github.com/lmctl-ai/lmcode-go) and
[lmcode-csharp](https://github.com/lmctl-ai/lmcode-csharp). Each is a library that turns an AOT executable into an lmscript host.

## Standard library catalog

`dslvm::stdlib` holds optional host functions shared with the other lmcode SDKs; copy the matching `functions` fragment from
`stdlib/string.schema-fragment.json` into the host schema and call `stdlib::register_string(&mut registry)`. The 16 byte-oriented functions match the C++ catalog: `str_len`, `str_starts_with`, `str_ends_with`, `str_contains`, `str_find`, `str_substr`, `str_lower`, `str_upper`, `str_trim`, `str_repeat`, `str_to_int`, `str_is_int`, `str_to_float`, `str_is_float`, `str_replace`, and `str_match`. Signature parity with the compiler is checked by
`cargo test -p dslvm --test stdlib_string` whenever `lmscript` (or `$LMSCRIPT_BIN`) is on PATH.

## VM strings and host text

VM strings are arbitrary bytes: invalid UTF-8 and embedded NUL are legal. Constants, concatenation, equality, and host calls preserve every byte. Lengths and offsets refer to bytes; slices can split UTF-8 codepoints. `Value::Str` stores a `ByteString`; `Value::string(bytes_or_text)` accepts byte vectors, byte slices, and Rust text. Existing direct `Value::Str(String)` constructors should use `Value::string(text)` or `Value::Str(text.into())`.

Host callbacks use `value.as_bytes()?` for unchanged bytes, `value.as_str()?` for validated UTF-8 (a conversion failure is `RUN_HOST`), or `value.to_string_lossy()?` for replacement-character text. `ByteString` also offers `as_bytes()`, `as_str()`, `to_string_lossy()`, and byte indexing. Registry identifiers remain UTF-8 host text. Host adapters such as the router choose their own text validation at their API boundary.

The string catalog uses byte offsets and ASCII case/whitespace rules. Substrings clamp negative starts to zero and overlong counts to the end; negative counts return empty. Search returns the first byte offset, or -1 (0 for an empty needle); replacement consumes non-overlapping matches and treats an empty search as a no-op. Repeat and replacement preflight a 64 MiB output cap, including no-op replacement copies; repeat also rejects negative counts. Numeric parsing accepts complete signed decimal tokens without whitespace, rejects overflow and floating-point underflow to zero, and returns zero on failure (`str_is_int`/`str_is_float` distinguish valid zero). NaN and infinity are invalid; representable subnormals and signed zero are preserved. Glob matching supports `*` (including slash), `?`, classes/ranges, `[!class]`, and backslash escapes. Unterminated classes and trailing backslashes are literal; leading `]` in a class is literal and descending ranges match no bytes. A deterministic 2^27 work cap counts matcher iterations, scanned class entries, and skipped trailing stars. Catalog policy failures return `RUN_HOST`.

The conformance runner also supports the optional catalog fixtures from a matching
C++ checkout, including its raw-byte hex adapters:

```sh
cargo run -p dsl-conformance --bin dsl-conformance -- ../lmcode-cpp/conformance/stdlib-vectors
```

An empty vector directory is an error, never a successful zero-case run.
