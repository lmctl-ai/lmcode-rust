# Gotchas

- Bytecode is bound to exact schema bytes; changing whitespace in a schema changes its hash.
- Handles are opaque and host-owned for one run.
- The language and VM impose no time or memory limits; hosts may enforce domain policy around their own APIs.
- The released CLI and the runtime must agree on bytecode version.
- The Rust workspace has no external crate dependencies; see `THIRD_PARTY_NOTICES.md` for attribution.
- Hosts embed their schema; to test a schema-hash mismatch, compile the chunk against a modified schema rather than passing a different schema file to the host.
