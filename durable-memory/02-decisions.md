# Public decisions

- Schema 2 uses arrays of signatures so one name can have exact overloads.
- Conversions are declared and single-step; the compiler resolves them before bytecode is written.
- Object arithmetic operators are schema mappings to ordinary methods.
- Host-owned collections use `len()` and `at()`; no collection opcode is part of the bytecode.
- Compiler-inlined functions do not add a runtime function table.
- Runtime errors retain the source line and column when the bytecode line table provides them.

- VM strings are bytes, including invalid UTF-8 and NUL. `Value::Str(ByteString)` and string constants retain the original bytes through concatenation, equality, and callbacks; lengths and offsets count bytes.
- Hosts access strings with `Value::as_bytes()`. `Value::as_str()` validates UTF-8 and reports conversion failures as `RUN_HOST`; `Value::to_string_lossy()` converts invalid sequences to replacement characters. Registry identifiers are UTF-8 host text.

- Default registries install the canonical 69-signature std surface; opt-out and manual subset binders remain explicit compatibility paths. Standard functions are not host omissions in normalized schema checks, and built-in tuples cannot be overwritten by arbitrary host callbacks.
- Default std run guards reject re-entry; borrowing RAII cleanup clears std collections on entry and exit, including unwind. Freed collection slots are reused without moving unrelated context objects. Manual opt-out registries preserve their previous lifetime behavior.
