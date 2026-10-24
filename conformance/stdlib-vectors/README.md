# String catalog vectors

The 18 string vectors cover all 16 functions in
`stdlib/string.schema-fragment.json`, plus negative/oversized repeat failures.
Run `make conformance` to run both the core and catalog suites.

Each `NAME.dsl` is a DSL body; its `.dslb` is compiled, not hand-assembled.
`NAME.expect` and `manifest.json` record independently specified expectations.
The host contract is in the manifest: `bytes(hex)` supplies arbitrary bytes and
`t.hex(value)` records lowercase hexadecimal, avoiding source-encoding and
newline ambiguities. Invalid UTF-8, embedded NUL, and partial UTF-8 slices must
remain bytes in every SDK. The helper does not decode or normalize text.

Recompile using a matching version-5 compiler from the repository root:

```sh
python3 conformance/compile-vectors.py conformance/stdlib-vectors
make conformance
```

Set `LMSCRIPT_BIN` to choose the compiler and `TMPDIR` to choose scratch storage.
The wrapper places one line before the source body; source-located errors thus
refer to body line plus one. Expectations are never regenerated from VM output.
The optional catalog is separate from the 51 core runtime vectors so SDKs can
adopt functions independently without silently skipping the core contract.

The additional 11 `math-*` vectors use `math.schema.json` to cover all 16 integer/float overloads in `stdlib/math.schema-fragment.json`. The complete optional suite has 29 vectors. Expectations check wrapping integer powers, exact floor roots near int64 limits, signed zero, NaN propagation and halfway-away rounding. Domain failures assert `RUN_HOST` with source positions. Non-exact finite libm results use the ordinary approximate trace format. The compiler only accepts integer literals through 2^53, so boundary inputs are constructed using exact wrapping integer expressions rather than oversized literals.
