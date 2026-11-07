//! The opt-in `rest` std module (`lmcode_std_rest_v1`, spec/std-rest.md): an embedded REST server over stdio.
//!
//! ```ignore
//! let mut registry = Registry::new();
//! let mut rest = register_rest(&mut registry, RestOptions::default())?;   // 18 extra signatures; Registry::new() has none
//! let program = Program::load(&bytecode, &schema_bytes, &registry)?;     // schema = lmcode-rest.schema.json (+ yours)
//! rest.run(&program, &registry, &mut Context::new())?;                   // body, then serve input -> output
//! ```
//! Everything runs on the calling thread: no sockets, no threads, no timeouts. Handlers run strictly one at a time and
//! never nest inside the body's run.
pub mod http;
pub mod router;

use crate::{Context, Error, ObjRef, Program, Registry, Value};
use http::{encode_response, error_response, take_request, Bytes, Request, Response, Taken};
use router::{percent_decode, Match, RouteTable};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::rc::Rc;

pub const HANDLER_INTERFACE: &str = "interface RestHandler(RestExchange) -> void";
const EXCHANGE: &str = "RestExchange";

/// Host options (not visible to lmcode). `None` streams mean the process's stdin and stdout.
pub struct RestOptions {
    pub input: Option<Box<dyn Read>>,
    pub output: Option<Box<dyn Write>>,
    pub max_body_bytes: usize,
    /// Receives the entry's function name and the VM error message; default: one line on stderr.
    pub on_handler_error: Option<Rc<dyn Fn(&str, &str)>>,
}
impl Default for RestOptions {
    fn default() -> Self { RestOptions { input: None, output: None, max_body_bytes: 1024 * 1024, on_handler_error: None } }
}

#[derive(Default)]
struct ExchangeData {
    request: Request,
    path: Bytes,
    params: Vec<(String, Bytes)>,
    query: HashMap<Bytes, Bytes>,
    status: u16,
    /// (lower-cased name, name, value) in first-insertion order.
    headers: Vec<(Bytes, Bytes, Bytes)>,
    body: Bytes,
}
impl ExchangeData {
    fn request_headers(&self, name: &[u8]) -> Vec<&[u8]> {
        self.request.headers.iter().filter(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_slice()).collect()
    }
    fn set_header(&mut self, name: &[u8], value: &[u8]) {
        let lower = name.to_ascii_lowercase();
        match self.headers.iter_mut().find(|h| h.0 == lower) {
            Some(h) => { h.1 = name.to_vec(); h.2 = value.to_vec(); }
            None => self.headers.push((lower, name.to_vec(), value.to_vec())),
        }
    }
}

struct EntryInfo { name: String, annotation: String, iface: String }

struct Shared {
    state_values: HashMap<Bytes, Bytes>,
    state_counts: HashMap<Bytes, i64>,
    table: RouteTable,
    /// The bound program's entries (None until `run`).
    entries: Option<Vec<EntryInfo>>,
    /// annotation -> (entry index, entry name), built by `serve`.
    entry_index: HashMap<Bytes, (usize, String)>,
    serving: bool,
    stopped: bool,
    on_handler_error: Rc<dyn Fn(&str, &str)>,
}

fn text(bytes: &[u8]) -> String { String::from_utf8_lossy(bytes).into_owned() }

impl Shared {
    fn route(&mut self, method: &[u8], path: &[u8], annotation: &[u8]) -> Result<(), Error> {
        if self.serving { return Err(Error::host("rest: routes are frozen once rest_serve has been called")); }
        self.table.add(method, path, annotation)
    }

    /// Validates (before any byte is read) and records that serving was requested.
    fn serve(&mut self) -> Result<(), Error> {
        if self.serving { return Err(Error::host("rest: rest_serve already called")); }
        let Some(entries) = &self.entries else { return Err(Error::host("rest: no program is bound (run the program through the controller)")) };
        if self.table.is_empty() { return Err(Error::host("rest: no routes registered")); }
        let mut index: HashMap<Bytes, (usize, String)> = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            if entry.iface != HANDLER_INTERFACE { continue; }
            if index.insert(entry.annotation.as_bytes().to_vec(), (i, entry.name.clone())).is_some() {
                return Err(Error::host(format!("rest: duplicate annotation @{}", entry.annotation)));
            }
        }
        for route in &self.table.routes {
            if index.contains_key(&route.annotation) { continue; }
            if let Some(wrong) = entries.iter().find(|e| e.annotation.as_bytes() == route.annotation.as_slice()) {
                return Err(Error::host(format!("rest: entry {} must implement RestHandler(RestExchange) -> void", wrong.name)));
            }
            return Err(Error::host(format!("rest: route {} {} references missing annotation @{}", route.method, text(&route.path), text(&route.annotation))));
        }
        self.entry_index = index;
        self.serving = true;
        Ok(())
    }
}

/// Owns the server state and the streams; created by [`register_rest`].
pub struct RestController {
    shared: Rc<RefCell<Shared>>,
    input: Option<Box<dyn Read>>,
    output: Option<Box<dyn Write>>,
    max_body_bytes: usize,
    slot: Option<ObjRef>,
}

impl RestController {
    /// Binds `program`, runs its body, then (when the body called `rest_serve` and not `rest_stop`) serves the input
    /// stream until EOF, `rest_stop()` or a framing error.
    pub fn run(&mut self, program: &Program, registry: &Registry, ctx: &mut Context) -> Result<(), Error> {
        {
            let mut s = self.shared.borrow_mut();
            s.entries = Some(program.entries().iter().map(|e| EntryInfo { name: e.name.clone(), annotation: e.annotation.clone(), iface: e.iface.clone() }).collect());
            s.stopped = false;
        }
        program.run(ctx, registry)?;
        let serve = { let s = self.shared.borrow(); s.serving && !s.stopped };
        if serve { self.pump(program, registry, ctx)?; }
        Ok(())
    }

    /// Ends serving as `rest_stop()` does.
    pub fn stop(&self) { self.shared.borrow_mut().stopped = true; }
    pub fn serving(&self) -> bool { self.shared.borrow().serving }

    fn pump(&mut self, program: &Program, registry: &Registry, ctx: &mut Context) -> Result<(), Error> {
        let mut input: Box<dyn Read> = self.input.take().unwrap_or_else(|| Box::new(std::io::stdin()));
        let mut output: Box<dyn Write> = self.output.take().unwrap_or_else(|| Box::new(std::io::stdout()));
        let io = |what: &str, e: std::io::Error| Error::host(format!("rest: {} failed: {}", what, e));
        let mut buffer: Vec<u8> = Vec::new();
        let mut chunk = vec![0u8; 16 * 1024];
        loop {
            let n = match input.read(&mut chunk) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(io("read", e)),
            };
            if n == 0 { return Ok(()); } // EOF: a truncated request is dropped without a response
            buffer.extend_from_slice(&chunk[..n]);
            loop {
                match take_request(&buffer, self.max_body_bytes) {
                    Taken::Need => break,
                    Taken::Fatal(status) => {
                        output.write_all(&encode_response(&error_response(status, &[]), false)).map_err(|e| io("write", e))?;
                        return output.flush().map_err(|e| io("write", e));
                    }
                    Taken::Request(request, used) => {
                        buffer.drain(..used);
                        let head = request.method == "HEAD";
                        let response = self.dispatch(program, registry, ctx, &request);
                        output.write_all(&encode_response(&response, head)).map_err(|e| io("write", e))?;
                        output.flush().map_err(|e| io("write", e))?;
                        if self.shared.borrow().stopped { return Ok(()); }
                    }
                }
            }
        }
    }

    /// The transport-independent core: one request in, one response out. Needs a bound, validated program (`rest_serve` called).
    pub fn dispatch(&mut self, program: &Program, registry: &Registry, ctx: &mut Context, request: &Request) -> Response {
        let target = &request.target;
        let mark = target.iter().position(|c| *c == b'?');
        let raw_path = &target[..mark.unwrap_or(target.len())];
        let query_text: &[u8] = mark.map(|m| &target[m + 1..]).unwrap_or(&[]);
        let Some(path) = percent_decode(raw_path, false) else { return error_response(400, &[]) };
        let mut query: HashMap<Bytes, Bytes> = HashMap::new();
        for pair in query_text.split(|c| *c == b'&') {
            if pair.is_empty() { continue; }
            let (k, v) = match pair.iter().position(|c| *c == b'=') { Some(eq) => (&pair[..eq], &pair[eq + 1..]), None => (pair, &pair[..0]) };
            let (Some(key), Some(value)) = (percent_decode(k, true), percent_decode(v, true)) else { return error_response(400, &[]) };
            query.entry(key).or_insert(value);
        }
        let (params, index, name) = {
            let s = self.shared.borrow();
            match s.table.find(&request.method, raw_path) {
                Match::NotFound => return error_response(404, &[]),
                Match::NotAllowed(allow) => return error_response(405, &[("Allow", &allow)]),
                Match::Found { route, params } => match s.entry_index.get(&route.annotation) {
                    Some((i, n)) => (params, *i, n.clone()),
                    None => return error_response(500, &[]),
                },
            }
        };
        let data = ExchangeData { request: request.clone(), path, params, query, status: 200, headers: Vec::new(), body: Vec::new() };
        let obj = match self.slot {
            Some(obj) => { match ctx.get_mut::<ExchangeData>(obj) { Ok(slot) => *slot = data, Err(_) => return error_response(500, &[]) } obj }
            None => { let obj = ctx.insert(EXCHANGE, data); self.slot = Some(obj); obj }
        };
        let outcome = program.call(index, ctx, registry, vec![Value::object(EXCHANGE, obj)]);
        let done = ctx.get_mut::<ExchangeData>(obj).map(std::mem::take);
        if let Err(error) = outcome {
            let report = self.shared.borrow().on_handler_error.clone();
            report(&name, &error.message);
            return error_response(500, &[]);
        }
        match done { Ok(x) => response_of(x), Err(_) => error_response(500, &[]) }
    }
}

fn response_of(x: ExchangeData) -> Response {
    let mut headers: Vec<(Bytes, Bytes)> = x.headers.iter().map(|(_, n, v)| (n.clone(), v.clone())).collect();
    let mut body = x.body;
    if x.status == 204 || x.status == 304 { body.clear(); }
    else if !body.is_empty() && !x.headers.iter().any(|h| h.0 == b"content-type") {
        headers.insert(0, (b"Content-Type".to_vec(), b"text/plain; charset=utf-8".to_vec()));
    }
    Response { status: x.status, headers, body }
}

fn is_reserved(lower: &[u8]) -> bool { matches!(lower, b"content-length" | b"connection" | b"transfer-encoding") }

/// Registers the 18 `rest` signatures in `registry` and returns the controller that owns the server.
/// The default registry never calls this.
pub fn register_rest(registry: &mut Registry, options: RestOptions) -> Result<RestController, Error> {
    let on_handler_error = options.on_handler_error.unwrap_or_else(|| Rc::new(|entry: &str, message: &str| eprintln!("rest: handler {} failed: {}", entry, message)));
    let shared = Rc::new(RefCell::new(Shared {
        state_values: HashMap::new(), state_counts: HashMap::new(), table: RouteTable::default(), entries: None,
        entry_index: HashMap::new(), serving: false, stopped: false, on_handler_error,
    }));
    let s = Rc::clone(&shared);
    registry.function("rest_route", &["string", "string", "string"], "void", move |_, a| {
        s.borrow_mut().route(a[0].as_bytes()?, a[1].as_bytes()?, a[2].as_bytes()?)?; Ok(Value::None)
    })?;
    let s = Rc::clone(&shared);
    registry.function("rest_serve", &[], "void", move |_, _| { s.borrow_mut().serve()?; Ok(Value::None) })?;
    let s = Rc::clone(&shared);
    registry.function("rest_stop", &[], "void", move |_, _| { s.borrow_mut().stopped = true; Ok(Value::None) })?;

    fn x<'a>(ctx: &'a mut Context, this: ObjRef) -> Result<&'a mut ExchangeData, Error> {
        ctx.get_mut::<ExchangeData>(this).map_err(|_| Error::host("invalid exchange"))
    }
    let t = EXCHANGE;
    registry.method(t, "method", &[], "string", |c, o, _| Ok(Value::string(x(c, o)?.request.method.clone())))?;
    registry.method(t, "path", &[], "string", |c, o, _| Ok(Value::string(x(c, o)?.path.as_slice())))?;
    registry.method(t, "query", &["string"], "string", |c, o, a| Ok(Value::string(x(c, o)?.query.get(a[0].as_bytes()?).cloned().unwrap_or_default())))?;
    registry.method(t, "has_query", &["string"], "bool", |c, o, a| Ok(Value::Bool(x(c, o)?.query.contains_key(a[0].as_bytes()?))))?;
    registry.method(t, "param", &["string"], "string", |c, o, a| {
        let want = a[0].as_bytes()?;
        Ok(Value::string(x(c, o)?.params.iter().find(|(n, _)| n.as_bytes() == want).map(|(_, v)| v.clone()).unwrap_or_default()))
    })?;
    registry.method(t, "header", &["string"], "string", |c, o, a| Ok(Value::string(x(c, o)?.request_headers(a[0].as_bytes()?).join(&b", "[..]))))?;
    registry.method(t, "has_header", &["string"], "bool", |c, o, a| Ok(Value::Bool(!x(c, o)?.request_headers(a[0].as_bytes()?).is_empty())))?;
    registry.method(t, "body", &[], "string", |c, o, _| Ok(Value::string(x(c, o)?.request.body.as_slice())))?;
    registry.method(t, "status", &["int"], "void", |c, o, a| {
        let code = a[0].as_int()?;
        if !(200..=599).contains(&code) { return Err(Error::host(format!("rest: status {} out of range 200..599", code))); }
        x(c, o)?.status = code as u16; Ok(Value::None)
    })?;
    registry.method(t, "set_header", &["string", "string"], "void", |c, o, a| {
        let (name, value) = (a[0].as_bytes()?, a[1].as_bytes()?);
        if name.is_empty() || name.iter().any(|b| *b <= 0x20 || *b == 0x7f || *b == b':') || value.iter().any(|b| *b == b'\r' || *b == b'\n') {
            return Err(Error::host("rest: invalid response header"));
        }
        if is_reserved(&name.to_ascii_lowercase()) { return Err(Error::host(format!("rest: response header {} is managed by the server", text(name)))); }
        x(c, o)?.set_header(name, value); Ok(Value::None)
    })?;
    registry.method(t, "send", &["string"], "void", |c, o, a| { x(c, o)?.body = a[0].as_bytes()?.to_vec(); Ok(Value::None) })?;
    registry.method(t, "json", &["string"], "void", |c, o, a| {
        let e = x(c, o)?; e.body = a[0].as_bytes()?.to_vec(); e.set_header(b"Content-Type", b"application/json"); Ok(Value::None)
    })?;
    let s = Rc::clone(&shared);
    registry.method(t, "get", &["string"], "string", move |_, _, a| Ok(Value::string(s.borrow().state_values.get(a[0].as_bytes()?).cloned().unwrap_or_default())))?;
    let s = Rc::clone(&shared);
    registry.method(t, "set", &["string", "string"], "void", move |_, _, a| {
        s.borrow_mut().state_values.insert(a[0].as_bytes()?.to_vec(), a[1].as_bytes()?.to_vec()); Ok(Value::None)
    })?;
    let s = Rc::clone(&shared);
    registry.method(t, "increment", &["string"], "int", move |_, _, a| {
        let mut s = s.borrow_mut();
        let n = s.state_counts.entry(a[0].as_bytes()?.to_vec()).or_insert(0);
        *n = n.wrapping_add(1); Ok(Value::Int(*n))
    })?;
    Ok(RestController { shared, input: options.input, output: options.output, max_body_bytes: options.max_body_bytes, slot: None })
}

#[cfg(test)]
mod tests;
