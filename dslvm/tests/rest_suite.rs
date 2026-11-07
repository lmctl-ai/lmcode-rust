//! The opt-in `rest` std module over stdio: replays conformance/rest-suite (stream, fatal, silent groups) through in-memory
//! streams and through a real child process (stdin/stdout of the `rest-serve` binary). No network is involved.
use dslvm::rest::{register_rest, RestOptions};
use dslvm::{Context, Program, Registry};
use std::cell::RefCell;
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;

// ---- a minimal JSON reader for scenarios.json (the crate has no dependencies) -----------------------------------
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum Json { Null, Bool(bool), Num(f64), Str(String), Arr(Vec<Json>), Obj(Vec<(String, Json)>) }
impl Json {
    fn get(&self, key: &str) -> Option<&Json> { if let Json::Obj(o) = self { o.iter().find(|(k, _)| k == key).map(|(_, v)| v) } else { None } }
    fn str(&self) -> &str { if let Json::Str(s) = self { s } else { panic!("not a string: {:?}", self) } }
    fn arr(&self) -> &[Json] { if let Json::Arr(a) = self { a } else { panic!("not an array") } }
}
struct P<'a> { b: &'a [u8], i: usize }
impl P<'_> {
    fn ws(&mut self) { while self.b[self.i].is_ascii_whitespace() { self.i += 1; } }
    fn value(&mut self) -> Json {
        self.ws();
        match self.b[self.i] {
            b'{' => { self.i += 1; let mut o = vec![]; loop { self.ws(); if self.b[self.i] == b'}' { self.i += 1; break Json::Obj(o); } if self.b[self.i] == b',' { self.i += 1; continue; } let k = self.string(); self.ws(); self.i += 1; o.push((k, self.value())); } }
            b'[' => { self.i += 1; let mut a = vec![]; loop { self.ws(); if self.b[self.i] == b']' { self.i += 1; break Json::Arr(a); } if self.b[self.i] == b',' { self.i += 1; continue; } a.push(self.value()); } }
            b'"' => Json::Str(self.string()),
            b't' => { self.i += 4; Json::Bool(true) }
            b'f' => { self.i += 5; Json::Bool(false) }
            b'n' => { self.i += 4; Json::Null }
            _ => { let s = self.i; while matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') { self.i += 1; } Json::Num(std::str::from_utf8(&self.b[s..self.i]).unwrap().parse().unwrap()) }
        }
    }
    fn string(&mut self) -> String {
        self.i += 1; let mut out = String::new();
        loop {
            let c = self.b[self.i];
            match c {
                b'"' => { self.i += 1; return out; }
                b'\\' => {
                    self.i += 1;
                    match self.b[self.i] {
                        b'n' => out.push('\n'), b'r' => out.push('\r'), b't' => out.push('\t'), b'b' => out.push('\u{8}'), b'f' => out.push('\u{c}'),
                        b'u' => { let code = u32::from_str_radix(std::str::from_utf8(&self.b[self.i + 1..self.i + 5]).unwrap(), 16).unwrap(); out.push(char::from_u32(code).unwrap()); self.i += 4; }
                        other => out.push(other as char),
                    }
                    self.i += 1;
                }
                _ => { let rest = std::str::from_utf8(&self.b[self.i..]).unwrap_or_else(|e| std::str::from_utf8(&self.b[self.i..self.i + e.valid_up_to()]).unwrap()); let ch = rest.chars().next().unwrap(); out.push(ch); self.i += ch.len_utf8(); }
            }
        }
    }
}

fn dir() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("conformance").join("rest-suite") }
fn suite() -> Json { let text = std::fs::read(dir().join("scenarios.json")).unwrap(); P { b: &text, i: 0 }.value() }
fn max_body() -> usize { if let Some(Json::Num(n)) = suite().get("options").and_then(|o| o.get("max_body_bytes")) { *n as usize } else { panic!("options.max_body_bytes") } }
const GOOD: &[u8] = b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n";
fn latin1(s: &str) -> Vec<u8> { s.chars().map(|c| c as u32 as u8).collect() }

/// Composes a scenario into request bytes (rules: tests/rest-suite/README.md).
fn request_bytes(s: &Json) -> Vec<u8> {
    if let Some(raw) = s.get("raw") { return latin1(raw.str()); }
    let body: String = match (s.get("body_repeat"), s.get("body")) {
        (Some(r), _) => { let Some(Json::Num(n)) = r.get("count") else { panic!() }; r.get("text").unwrap().str().repeat(*n as usize) }
        (None, Some(b)) => b.str().to_string(),
        _ => String::new(),
    };
    let method = s.get("method").unwrap().str();
    let mut head = format!("{} {} HTTP/1.1\r\nHost: localhost\r\n", method, s.get("target").unwrap().str());
    if let Some(Json::Obj(h)) = s.get("headers") { for (n, v) in h { head.push_str(&format!("{}: {}\r\n", n, v.str())); } }
    if !body.is_empty() || ["POST", "PUT", "PATCH"].contains(&method) { head.push_str(&format!("Content-Length: {}\r\n", body.len())); }
    head.push_str("\r\n");
    let mut out = latin1(&head); out.extend_from_slice(body.as_bytes()); out
}

struct Parsed { status: u16, headers: Vec<(String, String)>, body: Vec<u8>, length: usize }
/// Every response in `out`; `methods[i]` is the request method of response i (HEAD responses carry no body bytes).
fn parse_responses(out: &[u8], methods: &[String]) -> Vec<Parsed> {
    let (mut v, mut pos) = (vec![], 0);
    while pos < out.len() {
        let end = pos + out[pos..].windows(4).position(|w| w == b"\r\n\r\n").unwrap_or_else(|| panic!("incomplete response head: {:?}", String::from_utf8_lossy(&out[pos..])));
        let head = String::from_utf8_lossy(&out[pos..end]).into_owned();
        let mut lines = head.split("\r\n");
        let status: u16 = lines.next().unwrap().strip_prefix("HTTP/1.1 ").and_then(|r| r[..3].parse().ok()).expect("status line");
        let headers: Vec<(String, String)> = lines.map(|l| { let (n, v) = l.split_once(':').unwrap(); (n.to_lowercase(), v.trim().to_string()) }).collect();
        let length: usize = headers.iter().find(|h| h.0 == "content-length").expect("Content-Length present").1.parse().unwrap();
        let start = end + 4;
        let head_only = methods.get(v.len()).map(|m| m == "HEAD").unwrap_or(false);
        let stop = if head_only { start } else { start + length };
        v.push(Parsed { status, headers, body: out[start..stop].to_vec(), length });
        pos = stop;
    }
    v
}

fn check(label: &str, r: &Parsed, expect: &Json) {
    let Some(Json::Num(status)) = expect.get("status") else { panic!() };
    assert_eq!(r.status, *status as u16, "{}: status", label);
    let header = |n: &str| r.headers.iter().find(|h| h.0 == n).map(|h| h.1.as_str());
    if let Some(Json::Obj(hs)) = expect.get("headers") {
        for (name, want) in hs { match want { Json::Null => assert_eq!(header(name), None, "{}: header {} must be absent", label, name), w => assert_eq!(header(name), Some(w.str()), "{}: header {}", label, name) } }
    }
    assert_eq!(header("connection"), None, "{}: no Connection header", label);
    if let Some(b) = expect.get("body") { assert_eq!(String::from_utf8_lossy(&r.body), b.str(), "{}: body", label); }
}

/// A writer the test can read back after the controller (which owns its box) is done.
#[derive(Clone, Default)]
struct Sink(Rc<RefCell<Vec<u8>>>);
impl Write for Sink { fn write(&mut self, b: &[u8]) -> std::io::Result<usize> { self.0.borrow_mut().extend_from_slice(b); Ok(b.len()) } fn flush(&mut self) -> std::io::Result<()> { Ok(()) } }

/// Yields the input in the given pieces, one per `read`, to exercise reassembly across reads.
struct Pieces(std::collections::VecDeque<Vec<u8>>);
impl Read for Pieces {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let Some(mut piece) = self.0.pop_front() else { return Ok(0) };
        let n = piece.len().min(buf.len());
        buf[..n].copy_from_slice(&piece[..n]);
        if n < piece.len() { self.0.push_front(piece.split_off(n)); }
        Ok(n)
    }
}

fn serve_reader(input: Box<dyn Read>, on_error: Option<Rc<dyn Fn(&str, &str)>>, max: Option<usize>) -> Vec<u8> {
    let sink = Sink::default();
    let mut registry = Registry::new();
    let mut rest = register_rest(&mut registry, RestOptions {
        input: Some(input), output: Some(Box::new(sink.clone())), max_body_bytes: max.unwrap_or_else(max_body),
        on_handler_error: Some(on_error.unwrap_or_else(|| Rc::new(|_, _| {}))),
    }).unwrap();
    let program = Program::load(&std::fs::read(dir().join("app.dslb")).unwrap(), &std::fs::read(dir().join("host.schema.json")).unwrap(), &registry).unwrap();
    rest.run(&program, &registry, &mut Context::new()).unwrap();
    let out = sink.0.borrow().clone();
    out
}
fn serve_memory(input: &[u8]) -> Vec<u8> { serve_reader(Box::new(Cursor::new(input.to_vec())), None, None) }

/// Process-level serve: the `rest-serve` binary over real pipes.
fn serve_process(input: &[u8]) -> Vec<u8> {
    let d = dir();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rest-serve"))
        .arg(d.join("app.dslb")).arg(d.join("host.schema.json")).args(["--max-body-bytes", &max_body().to_string()])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let data = input.to_vec();
    // the server may stop reading (rest_stop, framing errors) before all input is written
    let writer = std::thread::spawn(move || { let _ = stdin.write_all(&data); });
    let out = child.wait_with_output().unwrap();
    writer.join().unwrap();
    assert_eq!(out.status.code(), Some(0), "rest-serve exit code; stderr: {}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

/// Replays all three groups, each on a fresh server; returns the number of checks.
fn replay(serve: &dyn Fn(&[u8]) -> Vec<u8>) -> usize {
    let suite = suite();
    let mut count = 0;
    let stream = suite.get("stream").unwrap().arr();
    let answered: Vec<&Json> = stream.iter().filter(|s| s.get("unanswered").is_none()).collect();
    let input: Vec<u8> = stream.iter().flat_map(request_bytes).collect();
    let methods: Vec<String> = answered.iter().map(|s| s.get("method").unwrap().str().to_string()).collect();
    let responses = parse_responses(&serve(&input), &methods);
    assert_eq!(responses.len(), answered.len(), "stream: exactly one response per answered request, none after rest_stop()");
    for (s, r) in answered.iter().zip(&responses) {
        check(&format!("{} ({})", s.get("name").unwrap().str(), s.get("target").unwrap().str()), r, s.get("expect").unwrap());
        assert!(r.length > 0 || r.body.is_empty());
        count += 1;
    }
    for s in suite.get("fatal").unwrap().arr() {
        let input = [request_bytes(s), GOOD.to_vec()].concat();
        let r = parse_responses(&serve(&input), &[s.get("method").unwrap().str().to_string()]);
        assert_eq!(r.len(), 1, "{}: one error response, then the end", s.get("name").unwrap().str());
        check(s.get("name").unwrap().str(), &r[0], s.get("expect").unwrap());
        count += 1;
    }
    for s in suite.get("silent").unwrap().arr() {
        assert!(serve(&request_bytes(s)).is_empty(), "{}: no output", s.get("name").unwrap().str());
        count += 1;
    }
    count
}

#[test]
fn conformance_suite_through_in_memory_streams_stream_fatal_silent_groups() {
    assert_eq!(replay(&serve_memory), 39);
}

#[test]
fn conformance_suite_through_a_real_child_process_stdin_to_stdout_exit_code_0() {
    assert_eq!(replay(&serve_process), 39);
}

fn get(path: &str) -> Vec<u8> { format!("GET {} HTTP/1.1\r\nHost: x\r\n\r\n", path).into_bytes() }
fn statuses(out: &[u8]) -> Vec<u16> { parse_responses(out, &[]).iter().map(|r| r.status).collect() }

#[test]
fn stdio_a_request_split_across_reads_even_mid_header_and_mid_body_is_reassembled() {
    let body = r#"{"values":[10,20,12]}"#;
    let parts = ["POST /su".to_string(), "m HTTP/1.1\r\nHost: x\r\nContent-Len".to_string(), format!("gth: {}\r\n\r\n{{\"values\"", body.len()), ":[10,20,12]}".to_string()];
    let out = serve_reader(Box::new(Pieces(parts.iter().map(|p| p.as_bytes().to_vec()).collect())), None, None);
    let r = parse_responses(&out, &["POST".to_string()]);
    assert_eq!((r[0].status, String::from_utf8_lossy(&r[0].body).into_owned()), (201, r#"{"sum":42}"#.to_string()));
    // and byte by byte
    let all = [get("/health"), get("/count")].concat();
    let out = serve_reader(Box::new(Pieces(all.iter().map(|b| vec![*b]).collect())), None, None);
    assert_eq!(statuses(&out), vec![200, 200]);
}

#[test]
fn stdio_pipelined_requests_in_one_read_are_answered_in_order() {
    let out = serve_memory(&[get("/hello/a"), get("/hello/b"), get("/nope"), get("/hello/c")].concat());
    assert_eq!(statuses(&out), vec![200, 200, 404, 200]);
    let bodies: Vec<String> = parse_responses(&out, &[]).iter().map(|r| String::from_utf8_lossy(&r.body).into_owned()).collect();
    assert_eq!(bodies[0], "hello, a");
    assert_eq!(bodies[3], "hello, c");
}

#[test]
fn stdio_handler_errors_reach_on_handler_error_with_the_entry_name_and_never_the_client_serving_continues() {
    let seen: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
    let sink = Rc::clone(&seen);
    let out = serve_reader(Box::new(Cursor::new([get("/boom"), get("/health")].concat())), Some(Rc::new(move |entry, text| sink.borrow_mut().push((entry.to_string(), text.to_string())))), None);
    let r = parse_responses(&out, &[]);
    assert_eq!((r[0].status, r[0].body.as_slice()), (500, &br#"{"error":"internal error"}"#[..]));
    assert_eq!(r[1].status, 200);
    let seen = seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, "boom");
    assert!(seen[0].1.to_lowercase().contains("zero"), "{}", seen[0].1);
}

#[test]
fn stdio_invalid_percent_escapes_in_the_path_or_query_are_400_and_do_not_end_serving() {
    let out = serve_memory(&[get("/nope?x=%zz"), get("/hello/%zz"), get("/health")].concat());
    assert_eq!(statuses(&out), vec![400, 400, 200]);
}

#[test]
fn stdio_state_and_head_semantics_survive_across_requests() {
    let put = b"PUT /note HTTP/1.1\r\nContent-Length: 3\r\n\r\nabc".to_vec();
    let head = b"HEAD /health HTTP/1.1\r\n\r\n".to_vec();
    let out = serve_memory(&[put, get("/note"), head, get("/count"), get("/count")].concat());
    let r = parse_responses(&out, &["PUT", "GET", "HEAD", "GET", "GET"].map(String::from));
    assert_eq!(r.iter().map(|x| x.status).collect::<Vec<_>>(), vec![202, 200, 200, 200, 200]);
    assert_eq!(r[1].body, b"abc");
    assert!(r[2].body.is_empty() && r[2].length > 0, "HEAD: Content-Length of the GET body, no body bytes");
    assert_eq!((r[3].body.as_slice(), r[4].body.as_slice()), (&b"1"[..], &b"2"[..]));
}

#[test]
fn stdio_max_body_bytes_option_decides_413_from_the_header() {
    let post = b"POST /sum HTTP/1.1\r\nContent-Length: 13\r\n\r\n{\"values\":[]}".to_vec();
    assert_eq!(statuses(&serve_reader(Box::new(Cursor::new(post.clone())), None, Some(10))), vec![413]);
    assert_eq!(statuses(&serve_reader(Box::new(Cursor::new([post, get("/health")].concat())), None, Some(1024))), vec![201, 200]);
}
