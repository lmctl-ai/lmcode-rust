# Invocation result sink

The Rust SDK's standard catalog declares the 24 `lmcode_return_*` imports:
`lmcode_return_set(T)` and `lmcode_return_put(string,T)` for `bool`, `float`,
`FloatList`, `FloatMap`, `int`, `IntList`, `IntMap`, `Json`, `StrList`,
`StrMap`, and `string`, plus `lmcode_return_null()` and
`lmcode_return_put_null(string)`. These are host callbacks, not VM opcodes.

One sink belongs to one logical invocation. The Rust `Context` carries it as
`result_sink: Option<ResultSink>`; the host sets it before the body and keeps
it through all selected entry calls, then clears it before the next
invocation. `set_result` and `put_result` are the conversion helpers, while
`frame_if_success` is the final emission gate. Concurrent invocations use
separate `Context`/sink pairs and the normal registry execution discipline;
the Rust `Registry` is not a shared concurrent invocation object. Values are copied
and validated at assignment time, so collection mutation and arena teardown do
not alter captured results. A failed write leaves the previous value intact;
an invocation that fails emits no result frame.

The wire frame is exactly `LMCODE_RETURN_BEGIN`, one compact JSON value line,
and `LMCODE_RETURN_END`, each terminated by LF. A successful invocation emits
the mandatory `null` frame when unset. Do not frame an application or dispatch
failure. Float serialization uses Rust Debug (with an integral .0 suffix removed);
the `0.1 + 0.2` fixture must contain `0.30000000000000004`, never a `%.15g`
format.

Conversion rejects invalid UTF-8, nonfinite values, unsafe mathematical
integers, nonzero underflow, duplicate keys, and host handles. Json nodes are private,
immutable trees built by the parser; safe Rust callers cannot construct cycles. Field writes require an
object or null root. JSON pointer keys escape `~` and `/`. These errors are
reported as host failures and never silently stringify or round numeric data.

The canonical schema and generated metadata are `std/lmcode-std.schema.json`
and `std/lmcode-std.docs.json`; `std.lock` and `std-docs.lock` pin each file to
the matching lmcode-cpp revision and digest. Run `scripts/sync-std.sh --verify`
to check both. The compiler's bundled catalog is updated in a later compiler
slice; do not publish a compiler during the intentional canonical-SDK skew
window.

## Verification

The normal cargo workspace tests execute the nine canonical return vectors
through the Rust VM, including all overloads, body/entry ownership and failed
entry framing. Their source/bytecode/expected bytes remain the C++ canonical
text, pinned by return-vectors.lock and the inner vectors.lock.
Run python3 scripts/mutate-results.py for production-copy negative controls,
including reuse of the actual VM format_g15 formatter. Compiler 0.2.444 can
compile the explicit-schema fixtures; implicit return catalog injection is the
subsequent compiler slice, not a Rust runtime requirement.

ResultSink.frame() returns a String and performs no I/O. frame_if_success(false)
returns None. The host must combine dispatch and application validation before
calling the framing helper, then write its bytes without changing LF. The SDK
does not infer application success or add frames to existing CLI hosts.
