# Build, run, and test

Install the released CLI with `npm install -g @lmctl-ai/lmctl@0.2.419`, then run `cargo test --workspace` for library, string catalog, and conformance checks. Run `bash examples/router/test.sh` for the router end-to-end checks: building the host, compiling DSL, and exercising success and error paths.

For embedding, add the `dslvm` crate from the public SDK repository, embed the schema bytes in the host binary (`include_bytes!`), and use the matching SDK major version.

Hosts follow the DSL host flag standard (lmscript R42): `HOST --dsl FILE [options]` runs, `HOST --dsl-schema` prints `Registry::schema_summary()`, `HOST --dsl-schema --json` prints the embedded schema bytes exactly, and `HOST --dsl-signatures` prints `Registry::signatures()` one per line. `--schema FILE` is removed and refused; any other combination exits 2 with empty stdout. `examples/router/test.sh` checks all of this, plus `_dsl_ cli="router"` once the compiler supports it (lmctl 0.2.347 does not, so it is skipped).

The synchronized catalog suite contains 118 vectors, including 16 file cases;
the core suite remains 51. File fixtures own a fresh temporary directory per
case and remove it afterward. `test_long_path` creates a Unicode filename past
260 characters. `runtime_error_file` checks the operation/path prefix and a
nonempty OS reason without depending on localized wording. Compiler parity
also checks the five-function file fragment and the 72-function standard.

The catalog suite includes 41 seeded property vectors (seeds 7, 42, 20261024),
with independently calculated byte/numeric/container expectations. CI runs these
through the ordinary catalog conformance step. `LMCODE_TRACE=1` emits actual
trace/error records for byte-exact cross-SDK comparisons; the source generator
and larger 16-seed/64-round mode live in lmcode-cpp `scripts/gen-std-corpus.py`
and `scripts/diff-stdlib.py`. The synchronized suite has 118 catalog cases and
51 core cases.
