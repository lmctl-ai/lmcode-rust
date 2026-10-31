# Canonical invocation-result fixtures

These are compiler-produced DSLB v6 programs, independently executed by
`conformance/run_returns.cpp` through `Program::load`, `run` and `call`.
`make conformance` always runs them; no optional harness skip is accepted.

The fixture host binds a ResultSink before the body, keeps it across selected
entries, checks each integer HALT result is 7, and emits only after dispatch and
application validation succeed. `all-overloads` asserts the payload after every
one of the 24 standard return imports. Error fixtures emit zero stdout bytes.
The frame expectations end in exactly one LF, including null when no write ran.

Regenerate with the prepared/pinned compiler:

```
LMSCRIPT_BIN=/path/to/lmscript python3 conformance/return-vectors/generate.py
```

The explicit schema declares the new imports so the slice-1 fixtures compile
before the compiler's bundled std/lowering slice 3. This does not eliminate the
std-skew release ban. `vectors.lock` binds schema, sources, binary v6 programs,
manifest and expected bytes; execution never derives expectations from outputs.

Fault-host flags demonstrate the wrong `%.15g` formatter, resetting the sink
before an entry, and framing before an entry can fail. Application validation
failure (exit 4) is also frame-silent. Native hosts must preserve LF bytes on
stdout; this harness selects binary stdout on Windows.
