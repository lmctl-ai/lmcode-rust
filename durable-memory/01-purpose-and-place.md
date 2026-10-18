# Purpose and place

- **Fact (2026-10-18):** R21 asked whether the DSL can support a Rust host as well as C++. This repo is the answer: a Rust
  runtime with the same semantics as `cpp-dsl-sdk/include/dslvm.hpp`, proven by the 21 language-neutral conformance vectors
  that the lmscript compiler generates (lmctl-mono `scripts/generate-dsl-conformance.ts`) and by poc-web's unchanged bytecode
  file answering identically on both runtimes.
- **Fact:** the conformance vectors, not the C++ header, are the contract. Everything the Rust runtime does was derived from the
  vectors plus the spec (`design/dsl-language.md` section 7) and checked against the header's messages where the vectors pin
  substrings (`bad magic`, `unsupported bytecode version 2`, `schema mismatch`, `bytecode is truncated`, `trailing byte`,
  `host function 'x' is not bound`, `goes backwards`, `int division by zero`, `int overflow`, `negating the smallest int`).
- **Fact:** a Rust host of OR-Tools is not possible (no Rust binding in lmor); R21's purpose is hosting Rust libraries and the
  lmapi Rust tools (`lmrust` family) with the same DSL and the same compiler.
- **Place:** the fifth DSL repo; read `cpp-dsl-sdk/durable-memory` for the format decisions, this repo for the Rust API.
