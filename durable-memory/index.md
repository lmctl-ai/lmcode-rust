# rust-dsl-sdk durable memory

What this repo is: the Rust runtime for lmscript `_dsl_` bytecode (crate `dslvm`), the conformance runner over the pinned
`cpp-dsl-sdk` vectors (24/24) and a std::net router example that runs poc-web's unchanged bytecode. Status (2026-10-18):
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
