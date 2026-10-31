# Optional dry host callbacks (H1)

Dry mode is invocation-wide and begins in `InvocationResults`, not in an ambient
process variable. A version-1 invocation manifest must contain `dry: true` or
`dry: false`. `InvocationResults::from_command` reads that field when a manifest is
present; no-manifest standalone hosts use real mode. Embedded callers can opt in
with `InvocationResults::from_command_with_dry(files, true)`. Selecting a slot sets
`Context::dry()` and binds that slot's independent result sink.

## Registering an optional slot

Each registration has one signature and a real callback. A dry slot can reuse that
callback or provide a preview. No slot means the real callback is skipped and the
SDK returns a fresh dummy for the declared return type.

```rust
use dslvm::{dry, real, Args, Context, Error, Registry, Value};

fn register_example(registry: &mut Registry) -> Result<(), Error> {
    registry.function_with_dry("read_value", &[], "int",
        |_, _| Ok(Value::Int(99)), dry(real))?;
    registry.function_with_dry("balance_preview", &[], "int",
        |_, _| Ok(Value::Int(99)),
        dry(|_: &mut Context, _: &Args| -> Result<Value, Error> { Ok(Value::Int(7)) }))?;
    Ok(())
}
```

Methods use `method_with_dry` with the same signature, receiver and argument list.
The `dry` wrapper owns a closure and does not clone it. The `real` marker selects
the registered real callback as the dry implementation.

## Defaults and ownership

The SDK creates these values when a callback has no dry slot:

- `void`: `Value::None` and no stack value.
- `bool`, numeric aliases, and string: `true`, zero in the declared numeric kind,
  and an empty string.
- Built-in string/int/float lists and maps: a new empty collection in the run arena
  for each call.
- `null`, object, opaque, resource and unsupported declared types: an internal typed
  dry null. It is not a live handle and cannot be passed to a real or preview callback.
  Using it as a receiver for an override is a located `RUN_HOST` error.

Preview callbacks return ordinary values and must satisfy the declared type and
ownership rules. The result contract still applies to `$return` and every result sink
write; dry null is JSON null, so a nonnullable result fails normally. Result-sink
operations and run-owned collection operations execute in dry mode. File reads use
`dry(real)`; file writes have no dry callback and are skipped with a void dummy.
Other built-in functions and methods are explicitly registered as run-owned/pure
slots. Replacing a host callback does not inherit a standard callback's policy by
name. Callback authors control their own effects; the SDK does not sandbox host code.

A dry success envelope has the ordinary result plus the trailing marker, for example:

```json
{"ok":true,"result":0,"dry":true}
```

Builder results keep add order and `parts`, followed by `dry:true`. Failure envelopes
also carry the trailing marker, with the existing part association and no partial
result. Real envelopes omit the marker. Dry callbacks are invoked once per reached
call; no automatic retries or real fallback occurs.
