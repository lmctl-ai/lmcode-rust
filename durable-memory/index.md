# rust-dsl-sdk durable memory

What this repo is: the Rust runtime for lmscript `_dsl_` bytecode (crate `dslvm`), the conformance runner over the pinned
`cpp-dsl-sdk` vectors (21/21) and a std::net router example that runs poc-web's unchanged bytecode. Status (2026-10-18):
EXPERIMENTAL, works; crate major version = bytecode version = 3; private repo `lmctlhq/rust-dsl-sdk`. Operator requirement R21.

Team guide: lmctl-mono `lmscript/durable-memory/dsl-framework.md`. Requirements and findings ("Findings from the Rust runtime"):
lmctl-mono `lmscript/durable-memory/requirements/dsl-bytecode.md`. README.md: embedding example, ownership model, tests, numbers.

| File | Fact |
| --- | --- |
| [01-purpose-and-place.md](01-purpose-and-place.md) | why a second runtime, what the conformance contract is |
| [02-decisions.md](02-decisions.md) | dated decisions: arena ownership, no crates, message parity, pinned vectors, version rule |
| [03-build-run-test.md](03-build-run-test.md) | commands, toolchain, numbers, what each test proves |
| [04-gotchas.md](04-gotchas.md) | what the C++ header did implicitly, vector pin, shared box, dates in the router |
| [05-open-items.md](05-open-items.md) | next steps |
