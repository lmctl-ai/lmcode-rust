# Rule entries, DSLB v6 (2026-10-28)

R49 is authoritative: mono lmscript durable-memory requirements/dsl-bytecode.md
and design/dsl-language.md at 0e621804. Version 6 only; all other versions fail load.
Entries store name, annotation, canonical interface, nparams and an independent
locals/max-stack/code/line-table region, sharing constants/imports/contexts.

Program.entries() exposes immutable source-order metadata; call(index, context,
registry, args) shares execute with run(). Body execution is explicit; calling an
entry first is supported. Parameters start at local zero; value HALT returns the
single untyped value and void returns None. Host owns argument kinds. Wrong arity
is RUN_HOST `call entry 'NAME' expects N argument(s); found M`.

Entry load refusals include mismatched parameter count/type, hidden parameters,
wrong HALT depth, malformed interface text, invalid local/pool indexes and jumps
outside the region. They use LOAD_BAD_BYTECODE and name the entry. New tests were
run against v5 first: ten refusal tests failed; after implementation they pass.
Context/registry cleanup and location behavior use the same path as body execution.

Core vectors: 70 from the mono generator at 0e621804, format dsl-conformance/3.
Standard vectors: 267 v6 files copied byte-for-byte from lmcode-cpp 451bc67.
The mono generator has a known runtime-empty-lines-fallback mutation defect: it
truncates the final entry count while removing the line table. Only that generated
fixture is corrected by retaining the u16 zero-entry tail (two zero bytes). The
compiler itself is untouched; vectors.lock records this exception. Regeneration
must preserve that tail until the upstream generator is fixed. The old extra
runtime-type-error vector is absent from the authoritative 70; VM type-error unit
coverage remains in place. Do not silently claim a larger shared corpus.

Gate: LMSCRIPT_BIN=<v6 compiler> LMSCRIPT_STD_BIN=<v6 compiler> cargo test --workspace;
cargo run -p dsl-conformance --bin dsl-conformance -- conformance/vectors;
same runner on conformance/stdlib-vectors; cargo clippy --workspace --all-targets.
Router: LMSCRIPT_MIN=0 with the supplied mono compiler (source version 0.2.38),
then bash examples/router/test.sh. Optional native lmweb parity is separate.
No matrix script lives in this repo. The C++ cross-SDK matrix rebuilds sibling
checkouts; coordinate its run with their owners instead of modifying their trees.
