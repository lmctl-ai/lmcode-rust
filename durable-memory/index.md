# rust-dsl-sdk durable memory

What this repo is: the Rust runtime for lmscript `_dsl_` bytecode (crate `dslvm`), the conformance runner over the pinned
`cpp-dsl-sdk` vectors (35/35) and a std::net router example that runs poc-web's unchanged bytecode. Status (2026-10-18):
EXPERIMENTAL, works; crate major version = bytecode version = 5; private repo `lmctlhq/rust-dsl-sdk`. Operator requirement R21.

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
For that historical v4 checkpoint, use the matching built v4 compiler via LMSCRIPT_MIN=0/LMSCRIPT_BIN; release 0.2.328 remains v3.

## S3 loop runtime (2026-10-18)

The runtime accepts the structured range pair `FOR_RANGE (0x2f)` and `FOR_NEXT (0x31)`. `FOR_RANGE` consumes start/end
integers once, stores the index and end in distinct hidden integer locals, and takes a forward exit relative to the end of
its operands. `FOR_NEXT` carries the end-local marker and a backward relative target to the instruction after its opener;
the verified pair metadata supplies the counter slot. The verifier enforces nesting, matching slots, instruction targets,
read-only loop locals, and lexical-region boundaries for ordinary jumps. It imposes no loop budget, time limit, memory cap,
or new run-time type/termination checks. The compiler owns the hidden locals and proves the loop shape before loading.

S3 verification: 58 integration + 2 library unit + 3 conformance-harness tests pass;
the prior 31 shared vectors pinned to cpp-dsl-sdk 126b592 pass, including one million iterations and 17 nested loops.

## S4 math runtime (2026-10-18)

The v4 `MATH` instruction is opcode `0x32` followed by a one-byte function id. IDs 0–13 are floating-point
operations (including truncating `int(float)`), and 14–17 are wrapping integer abs/min/max/clamp. Float min/max
propagate NaN and preserve the first operand on equal values, while clamp is the direct `min(max(x, lo), hi)` formula
even when bounds are reversed. Transcendentals use the platform Rust/libm operations; conformance may mark numeric
trace lines with `~ ` and checks finite values with relative tolerance `1e-12`.

S4 verification: 62 integration + 2 library + 4 conformance-harness tests, and 35/35 shared vectors
pinned to cpp-dsl-sdk 90e100090534eff869c512156742727cf1723ccb. Clippy completed with style warnings.

## S2 canonical overload binding

Multiple signatures may share one function/method name. Registry keys are full canonical signatures;
duplicate parameter tuples are refused regardless of return type. Enums retain their schema type
names in signatures and use strings as runtime values. Self expands to the receiver type before
registration/import binding; no identity check or dynamic overload resolution is added.
Verified: 64 integration + 2 library + 4 harness tests and 37 shared vectors.

## Compiler flow-lowering integration

Shared conformance now has 39 vectors, including inline foreach and int/string/enum switch.
Thing.len()/at() are conformance-host methods, not runtime instructions. Compiler-private
collection/discriminator temporaries use ordinary writable slots; only range controls carry
the wire hidden flag. No interpreter or verifier change was needed. Both runtimes pass all 39.

## v5 structured while (2026-10-19)

The v5 runtime accepts `WHILE_BEGIN` (0x34), `WHILE_TEST` (0x35) and `LOOP_BACK` (0x36). The verifier pairs nested while
loops, permits only a forward break to the innermost loop exit, and checks operand-stack depth at joins and loop heads. Continue
targets the loop latch. `WHILE_TEST` trusts the compiler-proven bool operand like range bounds; it adds no run-time type check,
budget or termination limit. Version 4 files are refused.

Verified after regeneration: 66 integration + 2 library + 4 harness tests; all 42 shared v5 vectors pass.
The earlier 37/39-vector results above are historical checkpoints.

Latest v5 verification: 68 integration + 2 library + 4 harness tests and all 43 shared vectors pass after porting upstream while-test structure/depth rules. The router parity test passes 11 checks with byte-identical C++/Rust schema bytes; an existing sibling schema mismatch now fails instead of being reported as a missing checkout.

## S5: located existing runtime errors

Errors retain their codes and carry optional line/column from the faulting opcode. Error::locate prefixes once, preserving nested locations; report_error includes numeric JSON fields only when known. Empty tables remain unlocated and column 0 is line-only, so long source lines impose no new cap. Shared runtime_error_at vectors match exact code/line/column and message substring. Verified: 70 integration + 2 library + 4 harness tests, all 50 shared vectors. No new dynamic checks or limits.
