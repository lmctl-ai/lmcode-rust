# Standard library catalog

The numeric catalog is exposed by `dslvm::stdlib::register_math` and the
`stdlib/math.schema-fragment.json` fragment. Each of `math_abs`, `math_floor`,
`math_ceil`, `math_round`, and `math_sqrt` has one int and one float overload;
`math_min`, `math_max`, and `math_pow` have matching int,int and float,float
overloads.

Integer arithmetic wraps in signed 64-bit two's-complement space. Integer
absolute value wraps for `i64::MIN`; integer floor, ceil, and round are
identity; integer power uses exponentiation by squaring, with `0^0 == 1`, and
rejects negative exponents with `math_pow: integer exponent must be
nonnegative`. Integer square root is the exact floor root and rejects negative
arguments with `math_sqrt: integer argument must be nonnegative`.

Float floor, ceil, and round use IEEE behavior with ties away from zero.
Minimum and maximum propagate NaN and return the first argument on ties,
including signed zero. Float power handles exponent zero and base one before
NaN propagation, and otherwise uses native binary64 power. Float square root
preserves negative zero and returns NaN for negative finite inputs.

## Catalog v2

The canonical v2 schema is pinned byte-for-byte in `std/lmcode-std.schema.json`
and `std.lock`; it contains 171 signatures. `Registry::new()` installs all of
them by default, while `Registry::without_std()` provides an explicit legacy
surface. The Rust registry is checked against the canonical 171-signature
inventory. Published `lmscript` 0.2.426 still injects the legacy 72-signature
catalog by default; merged-schema parity uses `LMSCRIPT_V2_BIN`, pointing to a
scratch lmctl-mono-main build, and is skipped explicitly when that variable is
absent.

V2 adds typed `IntMap`/`FloatMap`, `IntStack`/`FloatStack`/`StrStack`,
`IntQueue`/`FloatQueue`/`StrQueue`, stable min-first `IntPQ`/`FloatPQ`/`StrPQ`,
and immutable `Json`. Maps preserve first insertion order on overwrite and
append on reinsertion. Stacks and queues expose non-consuming pop-order `at`;
priority queues preserve insertion order for equal (including signed-zero)
priorities and reject nonfinite priorities without mutation.

Json accepts strict UTF-8 JSON, escaped NUL, signed int64 integers and finite
binary64 numbers. It rejects malformed UTF-8, duplicate decoded keys, BOMs,
invalid numbers and nesting deeper than 256. `to_json()` emits compact output
while preserving source number/string spellings and object order. Json
navigation borrows immutable nodes; roots and `keys()` snapshots use the
existing 65,536-object run arena. Exact bounds and parse diagnostics are pinned
by `conformance/stdlib-vectors`.

Local verification also used a scratch archive of lmctl-mono `origin/main`
`0ce49614a3236b88495652c7c892bba82bb3551d`: build native-git, then lmscript
with its current Node typings. Both `LMSCRIPT_BIN` and `LMSCRIPT_V2_BIN` pointed
to that scratch `lmscript/bin/lmscript` for the full workspace tests. The second
variable enables a separate empty-host-schema check of all 171 injected defaults;
it never silently substitutes the explicit catalog check. Published 0.2.426
still injects 72 defaults, so public CI uses the complete explicit canonical
schema for v2 parity and consumes the committed compiler-emitted vectors.
Native adapters allocate through `Context::insert_collection`; immutable JSON
navigation shares `Rc` nodes and cleanup-tracked handles without quota charges.

## Catalog v3

The current canonical fragment is v3: 181 signatures, synced byte-identically
from C++ with `std.lock`. `encoding.rs` implements four codecs without dependencies
or text conversion; output arithmetic is checked before allocation. Invalid
Base64/hex traps with `str_base64_decode: invalid encoding` or
`str_hex_decode: invalid encoding`; Base64 rejects whitespace, URL-safe symbols,
misplaced padding and nonzero unused pad bits. Hex accepts only complete ASCII
hex pairs and emits lowercase. There is no catalog-specific allocation cap.

`StrMap`, `IntMap` and `FloatMap` now provide `keys()`/`values()`: fresh mutable
lists in live insertion order, independently copied and quota-charged as the
returned list type. Float NaN payloads and signed zero remain unchanged.
The 267 shared stdlib vectors and 51 core vectors run in normal workspace tests;
unit checks also pin overflow boundaries, float bits and all six quota errors.
Published npm 0.2.426 still injects 72 defaults, but checks the explicit canonical
181-signature schema on CI. Local default-injection parity uses a scratch build
of lmctl-mono main `fa04890cbf6379abfd70803c6283a974efea5554`, with both
`LMSCRIPT_BIN` and `LMSCRIPT_STD_BIN` set to its `lmscript/bin/lmscript`.
`LMSCRIPT_STD_BIN` supersedes the v2-specific optional parity variable above.
