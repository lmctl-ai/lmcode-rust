# Build, run, and test

Install the released CLI with `npm install -g @lmctl-ai/lmctl@0.2.347`, then run `bash test.sh`. The script builds the native host, compiles a DSL chunk, runs it against the schema, and checks success and error paths.

For embedding, copy the SDK header or add the Rust crate from the public SDK repository, embed the schema bytes in the host binary (`include_bytes!`), and use the matching SDK major version.

Hosts follow the DSL host flag standard (lmscript R42): `HOST --dsl FILE [options]` runs, `HOST --dsl-schema` prints `Registry::schema_summary()`, `HOST --dsl-schema --json` prints the embedded schema bytes exactly, and `HOST --dsl-signatures` prints `Registry::signatures()` one per line. `--schema FILE` is removed and refused; any other combination exits 2 with empty stdout. `examples/router/test.sh` checks all of this, plus `_dsl_ cli="router"` once the compiler supports it (lmctl 0.2.347 does not, so it is skipped).
