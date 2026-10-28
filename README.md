# lmcode-rust

Status: experimental; bytecode compatibility is not promised. Requires a Rust toolchain; the router checks also use curl and lmscript.

`lmcode-rust` is the Rust runtime for lmscript bytecode v5. It provides the `dslvm`
crate, a loader, structural verifier, stack VM, typed host registry, and a conformance
runner over the shared SDK vectors. The workspace has no external Rust crate
dependencies.

Install the released compiler for the router example:

```sh
npm install -g @lmctl-ai/lmctl@0.2.426
```

Build and run the checks:

```sh
cargo test --workspace
bash examples/router/test.sh
```

## Gallery

| **Bytecode Conformance Suite** | **Unit Tests & Invariants** |
| --- | --- |
| <img src="docs/images/conformance.png" width="440" alt="Bytecode Conformance Suite"> | <img src="docs/images/unit-tests.png" width="440" alt="Unit Tests & Invariants"> |
| All 51 core bytecode vectors pass across opcodes, types, error paths, and control flow.<br>`cargo run --bin dsl-conformance -- conformance/vectors` | 74 VM tests, string catalog tests, and type invariants passing.<br>`cargo test` |

| **Router Example Host** | **Same Bytecode, Four Runtimes** |
| --- | --- |
| <img src="docs/images/router-example.png" width="440" alt="Router Example Host"> | <img src="docs/images/same-bytecode-four-runtimes.png" width="440" alt="Same Bytecode, Four Runtimes"> |
| Real HTTP router running compiled DSL routing logic in pure Rust.<br>`bash examples/router/test.sh` | Cross-language parity: C++, Rust, Go, and C# run the exact same bytecode.<br>`PASS 51/51 vectors across 4 runtimes` |

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

`Registry::new()` enables the pinned built-in lmcode standard catalog (`std/lmcode-std.schema.json`, 181 signatures). Use `Registry::without_std()` for a legacy host surface, or call `stdlib::register_std(&mut registry)` explicitly; registration is idempotent. Verify the canonical schema copy with `scripts/sync-std.sh --verify` and `std.lock`.

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

`Registry::new()` installs lmcode std automatically, without group-registration calls or handwritten std declarations. Until compiler injection lands, use `scripts/merge-std.py` to combine the pinned `std/lmcode-std.schema.json` with the host schema at build time, and embed those exact merged bytes. The `stdlib::register_*` functions remain compatibility APIs for explicit opt-out registries. The v3 catalog has 181 signatures: the original string, math, collection and file surface plus typed Int/Float maps, Int/Float/Str stacks, queues and stable min-first priority queues, immutable Json, strict byte Base64/hex codecs, and map key/value snapshots. Published `lmscript` 0.2.426 still defaults to v1; set `LMSCRIPT_STD_BIN` to a scratch lmctl-mono-main compiler for default-injection v3 parity.
Run `cargo test -p dslvm --test stdlib_catalogs` for exact parity between the explicit canonical schema and the registry; it uses `lmscript` (or `$LMSCRIPT_BIN`) on PATH.

The built-in numeric catalog is also described by `stdlib/math.schema-fragment.json`; it provides int/float overloads of `math_abs`, `math_min`, `math_max`, `math_floor`, `math_ceil`, `math_round`, `math_pow`, and `math_sqrt`. Integer operations wrap at 64 bits, integer roots are exact floor roots, and float operations preserve IEEE NaN, infinity, and signed-zero behavior described by the shared catalog.

Version 2 containers preserve insertion order for typed maps, use non-consuming pop-order `at` for stacks and queues, and use stable min-first priority queues with insertion-order ties. Json parses strict UTF-8 JSON (including escaped NUL), rejects duplicate keys, malformed numbers, BOMs, and nesting beyond 256, and preserves source number/string spellings and object order in compact `to_json()` output. Runtime diagnostics and bounds messages follow the shared catalog vectors.

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

Built-in std includes strings, math, collections, JSON, and files. `file_read`,
`file_write` (create/truncate), and `file_append` preserve every content byte.
`file_exists` is true only for regular files; missing paths and directories are
false. `file_lines` splits LF bytes into an arena-owned StrList, preserving CR
and empty fields, including the final field (`"a\n"` becomes `["a", ""]`).
Relative paths use the process cwd without sandbox restrictions; Unix passes
path bytes unchanged, while Windows requires UTF-8 paths and supports long names.
Failures trap with `RUN_HOST` and `FUNCTION: PATH: OS_REASON`.
Testing is outside std: hosts declare its separate fragment and explicitly call
`stdlib::register_testing(&mut registry, sink)` for `print`/`assert_close`.

Version 3 adds `str_base64_encode/decode` and `str_hex_encode/decode` over raw
bytes, with no artificial size cap. Base64 uses the RFC 4648 standard alphabet
and canonical padding; decoders reject whitespace, invalid symbols, misplaced
padding and nonzero pad bits. Hex output is lowercase and input accepts either
case. `keys()`/`values()` on StrMap, IntMap and FloatMap allocate independent
mutable list snapshots in insertion order, preserving bytes and float bits.
All snapshots share the 65,536-object run arena. The shared suite now contains
267 catalog vectors plus 51 core vectors.

## Compiler-only rule methods (R47/R48)

Schema 2 may declare `interfaces`, each containing one named-parameter signature.
A top-level `@NAME fn ... implements Interface` is checked and inlined by the
compiler; annotations do not install runtime callbacks or add registry signatures.
The loader hashes the schema bytes without parsing JSON, so the interface-bearing
schema needs no VM change. `scripts/merge-std.py` preserves the metadata when
merging or stripping std. DSLB stays v5, but changing schema bytes requires
recompilation. The shared 51 core and 267 std vectors are unchanged by this feature.

`cargo test -p dslvm --test rule_methods` runs a pinned annotated rule through a
host callback (42), checks schema mismatch, and tests schema-tool preservation.
The fixture source and exact schema are in `dslvm/tests/fixtures/rule-methods`.
Reproduce its bytecode with an R47/R48-capable compiler (mono `c1eb89de` used):

```sh
"$LMSCRIPT_BIN" dslvm/tests/fixtures/rule-methods/program.lms /tmp/rule-methods.dslb
cmp /tmp/rule-methods.dslb dslvm/tests/fixtures/rule-methods/program.dslb
```

Pinned runtime tests need no new compiler; regenerating this source does. The
older published compiler in CI must not be used to regenerate rule fixtures.
