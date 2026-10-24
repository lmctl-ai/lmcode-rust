//! Optional host-side catalogs for lmscript, the Rust counterpart of lmcode-cpp `stdlib/`. These are ordinary host functions:
//! copy the signatures from `stdlib/string.schema-fragment.json` into the host schema, then call `register_string`.
//! String operations count BYTES (UTF-8 is not decoded), as in every lmcode SDK.
use crate::{Error, Registry, Value};

/// Byte length of `s`, including NUL bytes.
pub fn str_len(s: impl AsRef<[u8]>) -> i64 { s.as_ref().len() as i64 }

/// Registers the string catalog implemented so far: `str_len(string) -> int`.
pub fn register_string(reg: &mut Registry) -> Result<&mut Registry, Error> {
    reg.function("str_len", &["string"], "int", |_, a| Ok(Value::Int(str_len(a[0].as_bytes()?))))
}
