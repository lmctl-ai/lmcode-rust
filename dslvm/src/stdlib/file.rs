//! Raw-byte file helpers. Paths and file contents never undergo UTF-8 decoding on Unix.
use crate::{ByteString, Context, Error, Registry, Value};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

fn failure(function: &str, path: &ByteString, error: impl std::fmt::Display) -> Error {
    Error::host(format!("{}: {}: {}", function, String::from_utf8_lossy(path.as_bytes()), error))
}
fn reject_nul(function: &str, path: &ByteString) -> Result<(), Error> {
    if path.as_bytes().contains(&0) { Err(failure(function, path, "invalid argument")) } else { Ok(()) }
}
#[cfg(unix)]
fn native_path(function: &str, path: &ByteString) -> Result<PathBuf, Error> {
    use std::os::unix::ffi::OsStringExt;
    reject_nul(function, path)?;
    Ok(PathBuf::from(std::ffi::OsString::from_vec(path.as_bytes().to_vec())))
}
#[cfg(windows)]
fn native_path(function: &str, path: &ByteString) -> Result<PathBuf, Error> {
    reject_nul(function, path)?;
    let text = std::str::from_utf8(path.as_bytes()).map_err(|e| failure(function, path, e))?;
    Ok(PathBuf::from(text))
}

fn regular(function: &str, path: &ByteString, allow_missing: bool) -> Result<Option<fs::Metadata>, Error> {
    let native = native_path(function, path)?;
    match fs::metadata(native) {
        Ok(metadata) if metadata.is_file() => Ok(Some(metadata)),
        Ok(metadata) => Err(failure(function, path, if metadata.is_dir() { "is a directory" } else { "invalid argument" })),
        Err(error) if allow_missing && matches!(error.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory) => Ok(None),
        Err(error) => Err(failure(function, path, error)),
    }
}
fn exists(function: &str, path: &ByteString) -> Result<bool, Error> {
    let native = native_path(function, path)?;
    match fs::metadata(native) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if matches!(error.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory) => Ok(false),
        Err(error) => Err(failure(function, path, error)),
    }
}
fn read_bytes(function: &str, path: &ByteString) -> Result<Vec<u8>, Error> {
    regular(function, path, false)?;
    let native = native_path(function, path)?;
    let mut file = File::open(native).map_err(|e| failure(function, path, e))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| failure(function, path, e))?;
    Ok(bytes)
}
fn write_bytes(function: &str, path: &ByteString, bytes: &[u8], append: bool) -> Result<(), Error> {
    regular(function, path, true)?;
    let native = native_path(function, path)?;
    let mut options = OpenOptions::new(); options.create(true).write(true);
    if append { options.append(true); } else { options.truncate(true); }
    let mut file = options.open(native).map_err(|e| failure(function, path, e))?;
    file.write_all(bytes).map_err(|e| failure(function, path, e))?;
    file.flush().map_err(|e| failure(function, path, e))?;
    file.sync_all().map_err(|e| failure(function, path, e))?;
    Ok(())
}
fn lines(bytes: &[u8]) -> Vec<ByteString> {
    let mut output = Vec::new(); let mut start = 0;
    for (i, byte) in bytes.iter().enumerate() { if *byte == b'\n' { output.push(ByteString::from(&bytes[start..i])); start = i + 1; } }
    output.push(ByteString::from(&bytes[start..])); output
}

/// Register raw-byte regular-file helpers and `file_lines` using the context-owned StrList arena.
pub fn register_file(registry: &mut Registry) -> Result<&mut Registry, Error> {
    registry.begin_compatibility_registration();
    registry.function("file_read", &["string"], "string", |_, args| Ok(Value::string(read_bytes("file_read", args[0].as_string()?)?)))?;
    registry.function("file_write", &["string", "string"], "void", |_, args| { let path = args[0].as_string()?; write_bytes("file_write", path, args[1].as_bytes()?, false)?; Ok(Value::None) })?;
    registry.function("file_append", &["string", "string"], "void", |_, args| { let path = args[0].as_string()?; write_bytes("file_append", path, args[1].as_bytes()?, true)?; Ok(Value::None) })?;
    registry.function("file_exists", &["string"], "bool", |_, args| Ok(Value::Bool(exists("file_exists", args[0].as_string()?)?)))?;
    registry.function("file_lines", &["string"], "StrList", |ctx: &mut Context, args| {
        let bytes = read_bytes("file_lines", args[0].as_string()?)?;
        Ok(Value::object("StrList", ctx.insert_collection("StrList", crate::stdlib::collections::CollectionList { items: lines(&bytes) })?))
    })?;
    registry.end_compatibility_registration(); Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_file_round_trip_and_lf_lines() {
        let path = std::env::temp_dir().join(format!("lmcode-rust-file-{}", std::process::id()));
        let path = ByteString::from(path.to_string_lossy().as_bytes());
        write_bytes("file_write", &path, b"a\n\xff\n", false).unwrap();
        assert_eq!(read_bytes("file_read", &path).unwrap(), b"a\n\xff\n");
        assert_eq!(lines(&read_bytes("file_read", &path).unwrap()), vec![ByteString::from(b"a"), ByteString::from(b"\xff"), ByteString::from(b"")]);
        write_bytes("file_append", &path, b"z", true).unwrap();
        assert_eq!(read_bytes("file_read", &path).unwrap(), b"a\n\xff\nz");
        let _ = std::fs::remove_file(String::from_utf8(path.into_bytes()).unwrap());
    }

    #[test]
    fn exists_distinguishes_missing_and_invalid_paths() {
        let missing = ByteString::from(b"/tmp/lmcode-rust-file-missing".as_slice());
        assert!(regular("file_exists", &missing, true).unwrap().is_none());
        let nul = ByteString::from(b"bad\0path".as_slice());
        assert_eq!(regular("file_exists", &nul, true).unwrap_err().code, "RUN_HOST");
    }
}
