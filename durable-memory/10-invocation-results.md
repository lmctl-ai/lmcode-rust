# Invocation results (R3)

One collector owns one process invocation. Rust selects into Context, whose
host closures already carry the active run; it does not install mutable state
on a shared Registry. ResultSink clones bind the same slot and seal, while
values written into it are deep owned copies independent of collection arenas.

```rust
use dslvm::{Context, Registry};
use dslvm::stdlib::{InvocationResults, ResultContract};
# fn example(files: &[String], schema: &[u8]) -> Result<(), dslvm::Error> {
let mut results = InvocationResults::from_command(files)?;
let contract = ResultContract::from_schema(schema)?;
let mut context = Context::new();
let registry = Registry::new();
for part in results.registrations().to_vec() {
    results.select(part.index, &mut context, contract.clone())?;
    // Load the authoritative role schema, run its program and selected entries.
    // On failure: results.fail(Some(part.index), "run", error)?; stop new work.
    results.complete(part.index)?;
}
// Drain accepted work and finish application validation BEFORE writing.
let code = results.write_frame(&mut std::io::stdout().lock());
# let _ = (code, registry); Ok(()) }
```

The host must use its role's exact schema bytes; manifest input never supplies a
contract. Re-selecting active slots preserves their value and requires the same
contract. Completion validates, snapshots and seals. Retained Context bindings
cannot write/reset a sealed slot. Fresh collectors create fresh slots; no implicit
reset on body/entry execution. Context/collector use Rc and are not Send/Sync:
interleave on one thread or provide host serialization, not concurrent mutation.

Private LMCODE_INVOCATION_FILE selects strict version-1 metadata. Explicit
from_command_with_manifest(files, None) is the standalone/test seam and never
reads ambient environment. Standalone duplicate file occurrences are distinct
slots; metadata paths are absolute, unique and matched exactly to registrations.
from_metadata accepts bytes for isolated testing/embedding. No public manifest
flag, timer, retry, size cap or shell is added.

One single success envelope is {"ok":true,"result":value}. Builder results are
in add order, even if execution is reversed, and have a required parts count.
A one-slot builder remains an array of one payload. fail latches the first error
and optional zero-based builder part; later failures cannot replace it. Existing
active work can drain but no new slot may start. Missing completion is failure,
not implicit null. Failure envelopes have no result/parts, and use phase cli,
load, run or app with exit 2, 3, 3 or 4 respectively.

write_frame writes the terminal markers and compact envelope, then flushes.
It is single-shot even after partial-write or flush failure (exit 4). A host
must not append another frame; failure_frame is only for pre-collector errors.
Old sink-only frame/report_ok/report_error/report_app_failure APIs are removed.
error_json remains a diagnostic serializer, not the invocation writer.

ResultContract validates optional schema-2 result metadata: null/bool/int/float/
string/Json, typed string/int/float lists/maps, and root nullable. No declaration
permits JSON including null. Known native types preserve distinctions; Json
values defer to their actual shape. Writes validate before changing the sink;
unset nonnullable values fail completion. Integers widen to floats without value
conversion. Rust shortest-round-trip digits use the canonical general layout;
0.1+0.2 remains 0.30000000000000004. No bytecode or standard signature changes.

Gate: cargo test --workspace --locked, canonical invocation vector script,
production-copy mutate-results.py (includes seal/sink/part/flush controls), and
scripts/sync-vectors.sh --verify. New fixtures include invocation-vectors.lock.
R7 updates sample adapters before changing their SDK pin; do not restore reporters
just to compile an old sample. Linux was locally qualified; other platforms need
native CI runs. Actions is manual; no release is dispatched by this work.
