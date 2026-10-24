# Public decisions

- Schema 2 uses arrays of signatures so one name can have exact overloads.
- Conversions are declared and single-step; the compiler resolves them before bytecode is written.
- Object arithmetic operators are schema mappings to ordinary methods.
- Host-owned collections use `len()` and `at()`; no collection opcode is part of the bytecode.
- Compiler-inlined functions do not add a runtime function table.
- Runtime errors retain the source line and column when the bytecode line table provides them.

- VM strings are bytes, including invalid UTF-8 and NUL. `Value::Str(ByteString)` and string constants retain the original bytes through concatenation, equality, and callbacks; lengths and offsets count bytes.
- Hosts access strings with `Value::as_bytes()`. `Value::as_str()` validates UTF-8 and reports conversion failures as `RUN_HOST`; `Value::to_string_lossy()` converts invalid sequences to replacement characters. Registry identifiers are UTF-8 host text.

- Default registries install the canonical 72-signature std surface; opt-out and manual subset binders remain explicit compatibility paths. Standard functions are not host omissions in normalized schema checks, and built-in tuples cannot be overwritten by arbitrary host callbacks.
- Default std run guards reject re-entry; borrowing RAII cleanup clears std collections on entry and exit, including unwind. Freed collection slots are reused without moving unrelated context objects. Manual opt-out registries preserve their previous lifetime behavior.

- Built-in std contains only string, math, collections and file functions (72 signatures). Testing is optional and explicitly registered against host declarations; its fragment is not part of the standard pin.
- File content remains bytes. `file_lines` splits only LF, preserves CR and trailing empty fields, and returns one empty field for empty input. Read/write/append/lines reject nonregular targets; `file_exists` uses metadata, so unreadable regular files still exist. Missing/empty paths and ENOTDIR yield false; other metadata errors trap. Read/write close errors are checked.
- File paths have no sandbox or root restriction. Unix paths remain native bytes; Windows validates UTF-8 and supports long Unicode paths. Error messages retain the function and path plus a nonempty OS reason.
- Text error messages render invalid-UTF-8 paths reversibly as `bytes:<lowercase hex>`; this display conversion does not alter the path supplied to the OS.
