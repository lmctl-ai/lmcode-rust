# Decisions (dated, with reasons)

- **2026-10-18, host objects live in a per-run `Context` arena** (`Vec<Option<Box<dyn Any>>>` + type names + name bindings);
  the DSL sees `ObjRef` handles; methods get `&mut Context` and borrow via `ctx.get_mut::<T>(this)`; functions create objects with
  `ctx.insert`. Reason: one mutable path and no `Rc<RefCell<_>>`, no lifetimes in the public API, and the handle lifetime (one
  run) is exactly what the language promises. Alternatives considered: `&mut dyn Any` parameters (two simultaneous borrows when a
  method creates a sibling object) and `Rc<RefCell<dyn Any>>` (runtime borrow panics, reference cycles).
- **2026-10-18, no external crates.** SHA-256 implemented in `dslvm/src/sha256.rs` with FIPS vectors (empty, "abc", the 56-byte
  string, one million 'a'); `%.15g` implemented in `format_g15` with a comparison table. Reason: the team rule, and a runtime
  that a host team can vendor by path or git with nothing else to audit.
- **2026-10-18, error messages mirror the C++ texts** so the vector substrings and host test pins hold across runtimes; one
  `Error(String)` type, phases distinguished by which call failed (load versus run). Hosts map load errors to exit 1 and run-time
  errors to exit 3, as the C++ hosts do.
- **2026-10-18, pinned copy of the vectors** (`conformance/vectors` + `vectors.lock` with the SDK commit and a digest over every
  file; `scripts/sync-vectors.sh`, `--verify` inside `cargo test`). Reason: `cargo test` must work offline and alone; the same
  mechanism the C++ hosts use for the header.
- **2026-10-18 (S0', spec v4): registration declares the schema's types** (`function(name, &[params], returns, f)`, `method(..)`, all returning `Result` so a duplicate is an error), `signatures()` lists the canonical strings `lmscript --dsl-signatures` prints (hosts diff them), `Error { code, message }` classifies the existing errors and `report_ok/report_error/report_app_failure` print the one-line JSON result with exit codes 3/4. Deliberately NOT added (R33/R34): call-time checks, forged-file defenses, caps.
- **2026-10-18, crate major version = bytecode version** (3.0.0); the loader refuses other versions; no backward compatibility.
- **2026-10-18, verifier accepts only what the compiler emits:** forward jumps, HALT last, 4096 locals; UTF-8 constants are
  additionally validated (the C++ header stores raw bytes and does not check this; recorded as a spec gap).
- **2026-10-18, the router example recomputes the civil date from the Unix epoch** (Hinnant's algorithm) with an optional
  `ROUTER_TZ_OFFSET_SECONDS`, because std has no local-time API and no crates are allowed; `test.sh` derives the offset from
  `date +%z`.
