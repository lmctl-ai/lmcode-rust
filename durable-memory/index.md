# rust-dsl-sdk durable memory

What this repo is: the Rust runtime for lmscript `_dsl_` bytecode (crate `dslvm`), the conformance runner over the pinned
`cpp-dsl-sdk` vectors (31/31) and a std::net router example that runs poc-web's unchanged bytecode. Status (2026-10-18):
EXPERIMENTAL, works; crate major version = bytecode version = 4; private repo `lmctlhq/rust-dsl-sdk`. Operator requirement R21.

Team guide: lmctl-mono `lmscript/durable-memory/dsl-framework.md`. Requirements and findings ("Findings from the Rust runtime"):
lmctl-mono `lmscript/durable-memory/requirements/dsl-bytecode.md`. README.md: embedding example, ownership model, tests, numbers.

| File | Fact |
| --- | --- |
| [01-purpose-and-place.md](01-purpose-and-place.md) | why a second runtime, what the conformance contract is |
| [02-decisions.md](02-decisions.md) | dated decisions: arena ownership, no crates, message parity, pinned vectors, version rule |
| [03-build-run-test.md](03-build-run-test.md) | commands, toolchain, numbers, what each test proves |
| [04-gotchas.md](04-gotchas.md) | what the C++ header did implicitly, vector pin, shared box, dates in the router |
| [05-open-items.md](05-open-items.md) | next steps |

## S1′ verification (2026-10-18)

Version 4 crate and wire format: 54 integration + 2 library unit tests, 3 conformance-harness tests,
24/24 shared vectors pinned to cpp-dsl-sdk cd7396b, and 11 router checks (the same 388-byte program
runs on Rust and C++). No new runtime caps or dynamic checks; integer arithmetic wraps.
Clippy completed with style suggestions; this slice does not perform unrelated style refactoring.
Use the matching built v4 compiler via LMSCRIPT_MIN=0/LMSCRIPT_BIN; release 0.2.328 remains v3.

## S3 loop runtime (2026-10-18)

The runtime accepts the structured range pair `FOR_RANGE (0x2f)` and `FOR_NEXT (0x31)`. `FOR_RANGE` consumes start/end
integers once, stores the index and end in distinct hidden integer locals, and takes a forward exit relative to the end of
its operands. `FOR_NEXT` carries the end-local marker and a backward relative target to the instruction after its opener;
the verified pair metadata supplies the counter slot. The verifier enforces nesting, matching slots, instruction targets,
read-only loop locals, and lexical-region boundaries for ordinary jumps. It imposes no loop budget, time limit, memory cap,
or new run-time type/termination checks. The compiler owns the hidden locals and proves the loop shape before loading.

S3 verification: 58 integration + 2 library unit + 3 conformance-harness tests pass;
31 shared vectors pinned to cpp-dsl-sdk 126b592 pass, including one million iterations and 17 nested loops.
