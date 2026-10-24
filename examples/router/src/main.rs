//! Example host on the Rust runtime: the SAME schema and the SAME bytecode file as lmweb's C++ server (lmctl-ai/lmweb,
//! cpp/server.cpp). A generic single-threaded HTTP/1.1 server (std::net, Connection: close) that knows nothing about
//! routing: per request it builds the context (req, res), runs the DSL chunk once, then writes whatever the chunk set
//! (default 500 when the chunk set no status). The schema is embedded at build time (DSL host flag standard, lmscript R42):
//! `router --dsl FILE [--port N]` serves (0 = free port, prints `listening on PORT`); `router --dsl-schema` prints the
//! human summary, `router --dsl-schema --json` the embedded schema bytes exactly, `router --dsl-signatures` one canonical
//! signature per line. Any other combination (including the removed `--schema FILE`) exits 2 with empty stdout.
//! Exit: 1 load refused, 2 usage. Stop with SIGTERM/SIGINT (the accept loop polls a shutdown flag).
use dslvm::{Context, Error, Program, Registry, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// The schema this host implements, embedded at build time; bytecode is verified against exactly these bytes.
const SCHEMA: &[u8] = include_bytes!("../schema/router.dsl.json");
const USAGE: &str = "usage: router --dsl-schema [--json] | --dsl-signatures | --dsl FILE [--port N]  (0 picks a free port)";

#[derive(Default)]
struct Request { method: String, path: String }
#[derive(Default)]
struct Response { status: i64, body: String, headers: Vec<(String, String)> }

fn today() -> String {
    // civil date from the Unix epoch (UTC, like lmweb's localtime on a UTC box is close enough for the proof; the test compares with `date +%F`)
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let tz = tz_offset_seconds();
    let days = ((secs as i64 + tz) / 86_400) as i64;
    let (y, m, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}
/// Local offset from `date +%z` is not available without libc; honour TZ=UTC or an explicit ROUTER_TZ_OFFSET_SECONDS, else UTC.
fn tz_offset_seconds() -> i64 { std::env::var("ROUTER_TZ_OFFSET_SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(0) }
fn civil_from_days(z: i64) -> (i64, u32, u32) {  // Howard Hinnant's algorithm
    let z = z + 719_468; let era = z.div_euclid(146_097); let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
fn reason(status: i64) -> &'static str {
    match status { 200 => "OK", 201 => "Created", 204 => "No Content", 400 => "Bad Request", 404 => "Not Found", 405 => "Method Not Allowed", 500 => "Internal Server Error", _ => "Status" }
}

fn registry() -> Registry {
    let mut reg = Registry::new();
    reg.context("req", "Request").unwrap().context("res", "Response").unwrap();
    reg.method("Request", "method", &[], "string", |ctx, this, _| Ok(Value::string(ctx.get::<Request>(this)?.method.clone()))).unwrap();
    reg.method("Request", "path", &[], "string", |ctx, this, _| Ok(Value::string(ctx.get::<Request>(this)?.path.clone()))).unwrap();
    reg.method("Response", "status", &["int"], "void", |ctx, this, a| { ctx.get_mut::<Response>(this)?.status = a[0].as_int()?; Ok(Value::None) }).unwrap();
    reg.method("Response", "body", &["string"], "void", |ctx, this, a| { ctx.get_mut::<Response>(this)?.body = a[0].as_str()?.to_string(); Ok(Value::None) }).unwrap();
    reg.method("Response", "header", &["string", "string"], "void", |ctx, this, a| { let (k, v) = (a[0].as_str()?.to_string(), a[1].as_str()?.to_string()); ctx.get_mut::<Response>(this)?.headers.push((k, v)); Ok(Value::None) }).unwrap();
    reg.function("now_date", &[], "string", |_, _| Ok(Value::string(today()))).unwrap();
    reg
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut data = Vec::new(); let mut buf = [0u8; 4096];
    let head_end = loop {
        if let Some(p) = data.windows(4).position(|w| w == b"\r\n\r\n") { break p; }
        let n = stream.read(&mut buf).ok()?; if n == 0 || data.len() > 65_536 { return None; }
        data.extend_from_slice(&buf[..n]);
    };
    let head = String::from_utf8_lossy(&data[..head_end]).into_owned();
    let mut lines = head.split("\r\n"); let first = lines.next()?; let mut parts = first.split_whitespace();
    let req = Request { method: parts.next()?.to_string(), path: parts.next()?.to_string() };
    let mut content_length = 0usize;
    for line in lines { if let Some((k, v)) = line.split_once(':') { if k.eq_ignore_ascii_case("content-length") { content_length = v.trim().parse().unwrap_or(0); } } }
    let mut have = data.len() - head_end - 4;  // the body is read and discarded; version 1 routes on method only
    while have < content_length { match stream.read(&mut buf) { Ok(0) | Err(_) => break, Ok(n) => have += n } }
    Some(req)
}

fn usage() -> ! { eprintln!("{}", USAGE); std::process::exit(2) }

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    // Introspection modes take exactly these argument lists; every other use of the flags is a usage error.
    match a.as_slice() {
        ["--dsl-schema"] => { print!("{}", registry().schema_summary()); std::process::exit(0) }
        ["--dsl-schema", "--json"] => { let mut o = std::io::stdout().lock(); std::process::exit(if o.write_all(SCHEMA).and_then(|_| o.flush()).is_ok() { 0 } else { 4 }) }
        ["--dsl-signatures"] => { for s in registry().signatures() { println!("{}", s); } std::process::exit(0) }  // diffed against `lmscript --dsl-signatures` by test.sh
        _ => {}
    }
    let (mut port, mut dsl) = (8080u16, None::<String>);
    let mut i = 0;
    while i < a.len() {
        let value = |i: &mut usize| -> String { *i += 1; match a.get(*i) { Some(v) if !v.is_empty() && !v.starts_with("--") => v.to_string(), _ => usage() } };
        match a[i] {
            "--port" => port = value(&mut i).parse().unwrap_or_else(|_| usage()),
            "--dsl" if dsl.is_none() => dsl = Some(value(&mut i)),
            _ => usage(),  // --schema (removed), --json, --dsl-schema/--dsl-signatures mixed with run flags, repeats, strays
        }
        i += 1;
    }
    let Some(dsl) = dsl else { usage() };
    // (1) the host starts and initializes: the registry is the implementation of the embedded schema
    let reg = registry();
    // (2) load and verify the DSL against the embedded schema; refused here, before the socket opens
    let program = match std::fs::read(&dsl) {
        Ok(b) => match Program::load(&b, SCHEMA, &reg) { Ok(p) => p, Err(Error { message: m, .. }) => { eprintln!("error: {}: {}", dsl, m); std::process::exit(1) } },
        Err(e) => { eprintln!("error: cannot read {}: {}", dsl, e); std::process::exit(1) }
    };
    eprintln!("loaded {} ({} constants, {} code bytes)", dsl, program.constant_count(), program.code_size());
    // (3) the host continues: serve, letting the DSL decide each response
    let listener = TcpListener::bind(("127.0.0.1", port)).unwrap_or_else(|e| { eprintln!("bind: {}", e); std::process::exit(1) });
    listener.set_nonblocking(true).ok();
    println!("listening on {}", listener.local_addr().map(|a| a.port()).unwrap_or(0));
    let stopping = Arc::new(AtomicBool::new(false));
    install_signal_flag(stopping.clone());
    while !stopping.load(Ordering::Relaxed) {
        let mut stream = match listener.accept() { Ok((s, _)) => s, Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => { std::thread::sleep(Duration::from_millis(50)); continue; } Err(_) => continue };
        stream.set_nonblocking(false).ok(); stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        let mut res = Response::default();
        match read_request(&mut stream) {
            None => { res.status = 400; res.body = "bad request".into(); }
            Some(req) => {
                let mut ctx = Context::new();
                ctx.set_object("req", "Request", req);
                let r = ctx.set_object("res", "Response", Response::default());
                match program.run(&mut ctx, &reg) {
                    Ok(()) => { res = ctx.take::<Response>(r).unwrap_or_default(); if res.status == 0 { res.status = 500; res.body = "the DSL set no status".into(); } }
                    Err(Error { message: m, .. }) => { eprintln!("dsl runtime error: {}", m); res.status = 500; res.body = "dsl runtime error".into(); }
                }
            }
        }
        let mut out = format!("HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n", res.status, reason(res.status), res.body.len());
        for (k, v) in &res.headers { out.push_str(&format!("{}: {}\r\n", k, v)); }
        out.push_str("\r\n"); out.push_str(&res.body);
        let _ = stream.write_all(out.as_bytes());
    }
    eprintln!("stopped");
}

/// Minimal SIGTERM/SIGINT handling without libc: a `signal` extern from the C runtime that std already links.
fn install_signal_flag(flag: Arc<AtomicBool>) {
    static FLAG: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();
    let _ = FLAG.set(flag);
    extern "C" fn on_signal(_: i32) { if let Some(f) = FLAG.get() { f.store(true, Ordering::Relaxed); } }
    extern "C" { fn signal(signum: i32, handler: extern "C" fn(i32)) -> usize; }
    unsafe { signal(15, on_signal); signal(2, on_signal); }  // SIGTERM, SIGINT (Linux numbers)
}
