# Build, run, test

- **Toolchain facts:** cargo/rustc 1.92, edition 2021, no dependencies; debug build of the workspace about 10 s, release about
  20 s on the shared box (use `nice -n 10 cargo ...`). curl and the released lmscript (>= 0.2.328, `npm install -g @lmctl-ai/lmctl`;
  `latest` 0.2.328 / `staging` 0.2.330 on 2026-10-18) for the router test.
- **Commands:** `cargo test` (dslvm: 2 unit + 54 integration tests; dsl-conformance: pin check + 24 vectors + a negative control);
  `cargo run -p dsl-conformance --bin dsl-conformance` (report, `PASS 24/24 vectors`); `cargo run --release -p dsl-conformance --bin bench int-arith 200000`;
  `bash examples/router/test.sh` (11 checks; `LMSCRIPT_BIN=path` overrides the compiler; the script refuses an lmscript older than
  0.2.328 with a message).
- **Numbers (2026-10-18, release, 200k runs, discarding stub):** int-arith 883k runs/s (C++ 893k), handles-and-chains 468k (C++ 626k),
  strings 832k (C++ 1,065k); 17 to 34 million instructions per second. Rust within 1.0 to 1.3x of C++, untuned.
- **What the router test proves:** the schema is byte-identical to poc-web's, the released lmscript compiles the copied chunk to
  209 bytes (v3), the Rust server answers GET with `date +%F` and POST with 405, stops on SIGTERM, the SAME `.dslb` answers the same
  on `../poc-web/build/server` when built, and schema mismatch / truncation / wrong magic are refused before serving.
