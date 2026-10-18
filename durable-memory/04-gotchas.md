# Gotchas and limits

- **What the C++ header did implicitly and the spec did not say** (found while porting): constants are not validated as UTF-8 in
  C++ (Rust validates and refuses); `%.15g` float text is C-library behaviour, now specified by `format_g15` and the vector
  `float-formatting`; the C++ `Context` silently ignores an object set under an undeclared name (Rust too, but `LOAD_CTX` of an
  undeclared name is a load error in both); int arithmetic is computed in 64 bits then range-checked (both); jump targets must be
  instruction starts within the code, `code.len()` itself is not a valid target (both, because HALT must be last).
- **The vectors are pinned, not live.** A new cpp-dsl-sdk commit does not change this repo until `scripts/sync-vectors.sh` is run
  and `vectors.lock` committed; `cargo test` fails if the copy drifts from the lock.
- **`Value::Obj` carries the type name as a `String`;** cloning it on every `LOAD_CTX` is part of the 1.3x gap versus C++.
  Interning would remove it; not done (no need yet).
- **Dates:** `examples/router` has no local-time API without crates; it uses UTC plus `ROUTER_TZ_OFFSET_SECONDS` (set by test.sh
  from `date +%z`). A host that needs local time should pass it in through a host function, as the C++ server does with `localtime_r`.
- **Signals in the router example** use the C runtime's `signal(2)` through an `extern "C"` declaration (no libc crate); Linux
  signal numbers 15 and 2 are hard-coded.
- **Shared box:** load 5-25; one heavy build at a time, `nice` cargo; the mono unit suite is unrelated to this repo.
