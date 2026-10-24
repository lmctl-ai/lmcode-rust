# Build, run, and test

Install the released CLI with `npm install -g @lmctl-ai/lmctl@0.2.347`, then run `cargo test --workspace` for library, string catalog, and conformance checks. Run `bash examples/router/test.sh` for the router end-to-end checks: building the host, compiling DSL, and exercising success and error paths.

For embedding, add the `dslvm` crate from the public SDK repository, embed the schema bytes in the host binary (`include_bytes!`), and use the matching SDK major version.

Hosts follow the DSL host flag standard (lmscript R42): `HOST --dsl FILE [options]` runs, `HOST --dsl-schema` prints `Registry::schema_summary()`, `HOST --dsl-schema --json` prints the embedded schema bytes exactly, and `HOST --dsl-signatures` prints `Registry::signatures()` one per line. `--schema FILE` is removed and refused; any other combination exits 2 with empty stdout. `examples/router/test.sh` checks all of this, plus `_dsl_ cli="router"` once the compiler supports it (lmctl 0.2.347 does not, so it is skipped).
