# rust-dsl-sdk

The Rust runtime for lmscript `_dsl_` bytecode (format v3): crate `dslvm` (loader, structural verifier, stack VM, host registry),
the conformance runner over the `cpp-dsl-sdk` vectors, and an example host (`examples/router`). Second runtime after the C++
reference `lmctlhq/cpp-dsl-sdk`; same semantics, same error classes, no external crates (SHA-256 is implemented in the crate and
checked against FIPS vectors). Status: EXPERIMENTAL (operator requirement R21). Slice 1: the crate. The full README follows.
