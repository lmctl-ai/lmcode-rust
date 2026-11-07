//! A small HTTP/1.1 subset (spec/std-rest.md, "HTTP subset"): request framing and response encoding.
//! Whitespace is ASCII-only; header values are trimmed of spaces and tabs only. Bytes are never decoded.

pub const MAX_HEADER_BYTES: usize = 16 * 1024;
pub type Bytes = Vec<u8>;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Request { pub method: String, pub target: Bytes, pub headers: Vec<(Bytes, Bytes)>, pub body: Bytes }
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Response { pub status: u16, pub headers: Vec<(Bytes, Bytes)>, pub body: Bytes }

pub fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK", 201 => "Created", 202 => "Accepted", 204 => "No Content", 301 => "Moved Permanently", 302 => "Found", 304 => "Not Modified",
        400 => "Bad Request", 401 => "Unauthorized", 403 => "Forbidden", 404 => "Not Found", 405 => "Method Not Allowed", 408 => "Request Timeout",
        409 => "Conflict", 413 => "Payload Too Large", 415 => "Unsupported Media Type", 422 => "Unprocessable Entity", 429 => "Too Many Requests",
        431 => "Request Header Fields Too Large", 500 => "Internal Server Error", 501 => "Not Implemented", 502 => "Bad Gateway", 503 => "Service Unavailable",
        s if s < 300 => "OK", s if s < 400 => "Redirect", s if s < 500 => "Client Error", _ => "Server Error",
    }
}

fn error_body(status: u16) -> &'static str {
    match status {
        400 => r#"{"error":"bad request"}"#, 404 => r#"{"error":"not found"}"#, 405 => r#"{"error":"method not allowed"}"#,
        413 => r#"{"error":"payload too large"}"#, 431 => r#"{"error":"request header too large"}"#,
        501 => r#"{"error":"not implemented"}"#, _ => r#"{"error":"internal error"}"#,
    }
}
pub fn error_response(status: u16, extra: &[(&str, &str)]) -> Response {
    let mut headers = vec![(b"Content-Type".to_vec(), b"application/json".to_vec())];
    headers.extend(extra.iter().map(|(n, v)| (n.as_bytes().to_vec(), v.as_bytes().to_vec())));
    Response { status, headers, body: error_body(status).as_bytes().to_vec() }
}

fn blank(b: u8) -> bool { b <= 0x20 || b == 0x7f }
fn trim_ht(mut v: &[u8]) -> &[u8] {
    while let [b' ' | b'\t', rest @ ..] = v { v = rest; }
    while let [rest @ .., b' ' | b'\t'] = v { v = rest; }
    v
}
fn find(hay: &[u8], needle: &[u8]) -> Option<usize> { hay.windows(needle.len()).position(|w| w == needle) }

/// Parses the header section (without the terminating blank line). `Err(status)` for a malformed head.
pub fn parse_head(text: &[u8]) -> Result<(String, Bytes, Vec<(Bytes, Bytes)>), u16> {
    let mut lines = Vec::new();
    let mut rest = text;
    loop {
        match find(rest, b"\r\n") { Some(i) => { lines.push(&rest[..i]); rest = &rest[i + 2..]; } None => { lines.push(rest); break; } }
    }
    // request line: [A-Z]+ SP target(no ASCII whitespace) SP HTTP/1.0|1.1
    let first = lines[0];
    let sp = first.iter().position(|c| *c == b' ').ok_or(400u16)?;
    let (method, tail) = (&first[..sp], &first[sp + 1..]);
    if method.is_empty() || !method.iter().all(u8::is_ascii_uppercase) { return Err(400); }
    let sp2 = tail.iter().position(|c| *c == b' ').ok_or(400u16)?;
    let (target, version) = (&tail[..sp2], &tail[sp2 + 1..]);
    if version != b"HTTP/1.1" && version != b"HTTP/1.0" { return Err(400); }
    if target.is_empty() || target.iter().any(|c| blank(*c)) || target[0] != b'/' || target.contains(&b'#') { return Err(400); }
    let mut headers = Vec::new();
    for line in &lines[1..] {
        let colon = line.iter().position(|c| *c == b':').unwrap_or(0);
        if colon == 0 || line[..colon].iter().any(|c| blank(*c)) || line.iter().any(|c| matches!(c, 0..=8 | 0x0a..=0x1f | 0x7f)) { return Err(400); }
        headers.push((line[..colon].to_vec(), trim_ht(&line[colon + 1..]).to_vec()));
    }
    Ok((String::from_utf8(method.to_vec()).map_err(|_| 400u16)?, target.to_vec(), headers))
}

/// Content-Length (0 when absent). `Err(status)`: 501 for Transfer-Encoding, 400 for a bad or conflicting length.
pub fn body_length(headers: &[(Bytes, Bytes)]) -> Result<u64, u16> {
    if headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(b"transfer-encoding")) { return Err(501); }
    let values: Vec<&Bytes> = headers.iter().filter(|(n, _)| n.eq_ignore_ascii_case(b"content-length")).map(|(_, v)| v).collect();
    let Some(first) = values.first() else { return Ok(0) };
    if values.iter().any(|v| v != first) { return Err(400); }
    if first.is_empty() || first.len() > 15 || !first.iter().all(u8::is_ascii_digit) { return Err(400); }
    Ok(std::str::from_utf8(first).unwrap().parse().unwrap())
}

pub fn encode_response(response: &Response, head: bool) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {} {}\r\n", response.status, reason_phrase(response.status)).into_bytes();
    for (name, value) in &response.headers { out.extend_from_slice(name); out.extend_from_slice(b": "); out.extend_from_slice(value); out.extend_from_slice(b"\r\n"); }
    out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", response.body.len()).as_bytes());
    if !head { out.extend_from_slice(&response.body); }
    out
}

#[derive(Debug, PartialEq)]
pub enum Taken { Need, Fatal(u16), Request(Request, usize) }

/// Takes one request off the front of `buffer`: `Need` for more input, `Fatal(status)` for an unrecoverable framing error,
/// or the request plus the number of bytes consumed.
pub fn take_request(buffer: &[u8], max_body_bytes: usize) -> Taken {
    let Some(end) = find(buffer, b"\r\n\r\n") else { return if buffer.len() > MAX_HEADER_BYTES { Taken::Fatal(431) } else { Taken::Need } };
    if end > MAX_HEADER_BYTES { return Taken::Fatal(431); }
    let (method, target, headers) = match parse_head(&buffer[..end]) { Ok(h) => h, Err(s) => return Taken::Fatal(s) };
    let length = match body_length(&headers) { Ok(l) => l, Err(s) => return Taken::Fatal(s) };
    if length > max_body_bytes as u64 { return Taken::Fatal(413); }
    let start = end + 4;
    let length = length as usize;
    if buffer.len() < start + length { return Taken::Need; }
    Taken::Request(Request { method, target, headers, body: buffer[start..start + length].to_vec() }, start + length)
}
