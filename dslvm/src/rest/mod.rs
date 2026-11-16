//! The `rest` std module (`lmcode_std_rest_v1`, spec/std-rest.md): an embedded REST server and stateless CLI over stdio.
pub mod http;
pub mod router;

use crate::stdlib::collections::StrList;
use crate::{Context, Error, ObjRef, Program, Registry, Value};
use http::{encode_response, error_response, take_request, Bytes, Request, Response, Taken};
use router::{percent_decode, Match, RouteTable};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::rc::Rc;

pub const HANDLER_INTERFACE: &str = "interface RestHandler(RestExchange) -> void";
pub const REST_GET_INTERFACE: &str = "interface RestGet(RestExchange) -> void";
pub const REST_POST_INTERFACE: &str = "interface RestPost(RestExchange) -> void";
pub const REST_PUT_INTERFACE: &str = "interface RestPut(RestExchange) -> void";
pub const REST_DELETE_INTERFACE: &str = "interface RestDelete(RestExchange) -> void";
pub const REST_PATCH_INTERFACE: &str = "interface RestPatch(RestExchange) -> void";
pub const REST_HEAD_INTERFACE: &str = "interface RestHead(RestExchange) -> void";
pub const REST_OPTIONS_INTERFACE: &str = "interface RestOptions(RestExchange) -> void";
pub const CLI_COMMAND_INTERFACE: &str = "interface CliCommand(RestExchange) -> void";
pub const CLI_OPTION_INTERFACE: &str = "interface CliOption(RestExchange) -> void";
const EXCHANGE: &str = "RestExchange";

pub fn method_for_rest_interface(iface: &str) -> Option<&'static [u8]> {
    match iface {
        REST_GET_INTERFACE => Some(b"GET"),
        REST_POST_INTERFACE => Some(b"POST"),
        REST_PUT_INTERFACE => Some(b"PUT"),
        REST_DELETE_INTERFACE => Some(b"DELETE"),
        REST_PATCH_INTERFACE => Some(b"PATCH"),
        REST_HEAD_INTERFACE => Some(b"HEAD"),
        REST_OPTIONS_INTERFACE => Some(b"OPTIONS"),
        _ => None,
    }
}

pub fn is_rest_or_cli_interface(iface: &str) -> bool {
    iface == HANDLER_INTERFACE
        || method_for_rest_interface(iface).is_some()
        || iface == CLI_COMMAND_INTERFACE
        || iface == CLI_OPTION_INTERFACE
}

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
    auto_routes_materialized: bool,
    auto_entry_index: HashMap<Bytes, (usize, String)>,
    auto_entry_indices: HashSet<usize>,
    /// annotation -> (entry index, entry name), built by `serve`.
    entry_index: HashMap<Bytes, (usize, String)>,
    cli_options: HashMap<Bytes, (usize, String)>,
    config_prefix: Bytes,
    serving: bool,
    stopped: bool,
    in_controller_run: bool,
    max_body_bytes: usize,
    slot: Option<ObjRef>,
    active_program: Option<*const Program>,
    active_registry: Option<*const Registry>,
    on_handler_error: Rc<dyn Fn(&str, &str)>,
}

fn text(bytes: &[u8]) -> String { String::from_utf8_lossy(bytes).into_owned() }

impl Shared {
    fn bind_entries(&mut self, entries: Vec<EntryInfo>) {
        self.entries = Some(entries);
        self.auto_routes_materialized = false;
        self.auto_entry_index.clear();
        self.auto_entry_indices.clear();
        self.entry_index.clear();
        self.cli_options.clear();
        self.config_prefix.clear();
    }

    fn normalized_prefix(&self) -> Bytes {
        let raw = &self.config_prefix;
        if raw.is_empty() { return Vec::new(); }
        let mut p = if raw[0] == b'/' { raw.clone() } else { let mut v = vec![b'/']; v.extend_from_slice(raw); v };
        while p.len() > 1 && p.last() == Some(&b'/') { p.pop(); }
        if p == b"/" { Vec::new() } else { p }
    }

    fn materialize_auto_routes(&mut self) -> Result<(), Error> {
        if self.auto_routes_materialized { return Ok(()); }
        self.auto_routes_materialized = true;
        let prefix = self.normalized_prefix();
        let Some(entries) = &self.entries else { return Ok(()) };
        let mut to_add: Vec<(Bytes, Bytes, Bytes, usize, String)> = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let ann = entry.annotation.as_bytes();
            if entry.iface == CLI_OPTION_INTERFACE {
                self.cli_options.insert(ann.to_vec(), (i, entry.name.clone()));
                let stripped: Bytes = ann.iter().copied().skip_while(|b| *b == b'-').collect();
                if !stripped.is_empty() {
                    self.cli_options.insert(stripped, (i, entry.name.clone()));
                }
                self.auto_entry_indices.insert(i);
                continue;
            }
            let (method, mut path): (Bytes, Bytes) = if let Some(m) = method_for_rest_interface(&entry.iface) {
                if let Some(sp) = ann.iter().position(|b| *b == b' ') {
                    (ann[..sp].to_vec(), ann[sp + 1..].to_vec())
                } else if ann.first() == Some(&b'/') {
                    (m.to_vec(), ann.to_vec())
                } else {
                    continue;
                }
            } else if entry.iface == HANDLER_INTERFACE {
                if let Some(sp) = ann.iter().position(|b| *b == b' ') {
                    (ann[..sp].to_vec(), ann[sp + 1..].to_vec())
                } else if ann.first() == Some(&b'/') {
                    (b"GET".to_vec(), ann.to_vec())
                } else {
                    continue;
                }
            } else if entry.iface == CLI_COMMAND_INTERFACE {
                let (m, mut p) = if let Some(sp) = ann.iter().position(|b| *b == b' ') {
                    (ann[..sp].to_vec(), ann[sp + 1..].to_vec())
                } else {
                    (b"GET".to_vec(), ann.to_vec())
                };
                if p.first() != Some(&b'/') {
                    let mut slash = vec![b'/'];
                    slash.extend_from_slice(&p);
                    p = slash;
                }
                (m, p)
            } else {
                continue;
            };
            if !prefix.is_empty() && !path.starts_with(&prefix) {
                path = if path == b"/" { prefix.clone() } else { let mut v = prefix.clone(); v.extend_from_slice(&path); v };
            }
            let tag = format!("__auto_entry_{}", i).into_bytes();
            to_add.push((method, path, tag, i, entry.name.clone()));
        }
        for (method, path, tag, i, name) in to_add {
            self.table.add(&method, &path, &tag)?;
            self.auto_entry_index.insert(tag, (i, name));
            self.auto_entry_indices.insert(i);
        }
        Ok(())
    }

    fn ensure_dispatch_ready(&mut self) -> Result<(), Error> {
        if self.entries.is_none() { return Ok(()); }
        self.materialize_auto_routes()?;
        if self.entry_index.is_empty() && !self.auto_entry_index.is_empty() {
            let mut index = self.auto_entry_index.clone();
            if let Some(entries) = &self.entries {
                for (i, entry) in entries.iter().enumerate() {
                    if !is_rest_or_cli_interface(&entry.iface) || entry.iface == CLI_OPTION_INTERFACE { continue; }
                    if self.auto_entry_indices.contains(&i) { continue; }
                    index.entry(entry.annotation.as_bytes().to_vec()).or_insert((i, entry.name.clone()));
                }
            }
            self.entry_index = index;
        }
        Ok(())
    }

    fn config(&mut self, key: &[u8], value: &[u8]) -> Result<(), Error> {
        if self.serving { return Err(Error::host("rest: routes are frozen once rest_serve has been called")); }
        self.state_values.insert(key.to_vec(), value.to_vec());
        if key == b"prefix" || key == b"base_path" {
            self.config_prefix = value.to_vec();
        } else if key == b"max_body_bytes" && !value.is_empty() && value.iter().all(|b| b.is_ascii_digit()) {
            if let Ok(n) = std::str::from_utf8(value).unwrap_or("").parse::<usize>() {
                if n > 0 { self.max_body_bytes = n; }
            }
        }
        Ok(())
    }

    fn route(&mut self, method: &[u8], path: &[u8], annotation: &[u8]) -> Result<(), Error> {
        if self.serving { return Err(Error::host("rest: routes are frozen once rest_serve has been called")); }
        self.table.add(method, path, annotation)
    }

    /// Validates (before any byte is read) and records that serving was requested.
    fn serve(&mut self) -> Result<(), Error> {
        if self.serving { return Err(Error::host("rest: rest_serve already called")); }
        if self.entries.is_none() { return Err(Error::host("rest: no program is bound (run the program through the controller)")); }
        self.materialize_auto_routes()?;
        let entries = self.entries.as_ref().unwrap();
        if self.table.is_empty() { return Err(Error::host("rest: no routes registered")); }
        let mut index: HashMap<Bytes, (usize, String)> = self.auto_entry_index.clone();
        for (i, entry) in entries.iter().enumerate() {
            if !is_rest_or_cli_interface(&entry.iface) || entry.iface == CLI_OPTION_INTERFACE { continue; }
            if self.auto_entry_indices.contains(&i) { continue; }
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

fn call_entry_isolated(program: &Program, registry: &Registry, ctx: &mut Context, index: usize, obj: ObjRef) -> Result<Value, Error> {
    let was_running = registry.suspend_std_run();
    let saved_collections = ctx.take_collection_tracking();
    let result = program.call(index, ctx, registry, vec![Value::object(EXCHANGE, obj)]);
    ctx.restore_collection_tracking(saved_collections);
    registry.resume_std_run(was_running);
    result
}

/// Owns the server state and the streams; created by [`register_rest`].
pub struct RestController {
    shared: Rc<RefCell<Shared>>,
    input: Rc<RefCell<Option<Box<dyn Read>>>>,
    output: Rc<RefCell<Option<Box<dyn Write>>>>,
}

impl RestController {
    /// Binds `program`, runs its body, then (when the body called `rest_serve` and not `rest_stop`) serves the input
    /// stream until EOF, `rest_stop()` or a framing error.
    pub fn run(&mut self, program: &Program, registry: &Registry, ctx: &mut Context) -> Result<(), Error> {
        {
            let mut s = self.shared.borrow_mut();
            s.bind_entries(program.entries().iter().map(|e| EntryInfo { name: e.name.clone(), annotation: e.annotation.clone(), iface: e.iface.clone() }).collect());
            s.stopped = false;
            s.in_controller_run = true;
            s.active_program = Some(program as *const Program);
            s.active_registry = Some(registry as *const Registry);
        }
        let res = program.run(ctx, registry);
        {
            let mut s = self.shared.borrow_mut();
            s.in_controller_run = false;
        }
        res?;
        let serve = { let s = self.shared.borrow(); s.serving && !s.stopped };
        if serve { self.pump(program, registry, ctx)?; }
        Ok(())
    }

    /// Ends serving as `rest_stop()` does.
    pub fn stop(&self) { self.shared.borrow_mut().stopped = true; }
    pub fn serving(&self) -> bool { self.shared.borrow().serving }

    fn pump(&mut self, program: &Program, registry: &Registry, ctx: &mut Context) -> Result<(), Error> {
        let mut input: Box<dyn Read> = self.input.borrow_mut().take().unwrap_or_else(|| Box::new(std::io::stdin()));
        let mut output: Box<dyn Write> = self.output.borrow_mut().take().unwrap_or_else(|| Box::new(std::io::stdout()));
        let max_body_bytes = self.shared.borrow().max_body_bytes;
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
                match take_request(&buffer, max_body_bytes) {
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

    /// The transport-independent core: one request in, one response out.
    pub fn dispatch(&mut self, program: &Program, registry: &Registry, ctx: &mut Context, request: &Request) -> Response {
        let _ = self.shared.borrow_mut().ensure_dispatch_ready();
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
        let existing_slot = self.shared.borrow().slot;
        let obj = match existing_slot {
            Some(obj) => { match ctx.get_mut::<ExchangeData>(obj) { Ok(slot) => *slot = data, Err(_) => return error_response(500, &[]) } obj }
            None => { let obj = ctx.insert(EXCHANGE, data); self.shared.borrow_mut().slot = Some(obj); obj }
        };
        let outcome = call_entry_isolated(program, registry, ctx, index, obj);
        let done = ctx.get_mut::<ExchangeData>(obj).map(std::mem::take);
        if let Err(error) = outcome {
            let report = self.shared.borrow().on_handler_error.clone();
            report(&name, &error.message);
            return error_response(500, &[]);
        }
        match done { Ok(x) => response_of(x), Err(_) => error_response(500, &[]) }
    }
}

fn dispatch_cli(shared: &Rc<RefCell<Shared>>, ctx: &mut Context, argv: Vec<Bytes>) -> Result<Bytes, Error> {
    let (program_ptr, registry_ptr) = {
        let mut s = shared.borrow_mut();
        if s.entries.is_none() || s.active_program.is_none() || s.active_registry.is_none() {
            return Err(Error::host("rest: no program is bound (run the program through the controller)"));
        }
        s.ensure_dispatch_ready()?;
        (s.active_program.unwrap(), s.active_registry.unwrap())
    };
    // SAFETY: `active_program` and `active_registry` are set for the duration of `Program::run` / `RestController::run`.
    let (program, registry) = unsafe { (&*program_ptr, &*registry_ptr) };

    let mut method = String::from("GET");
    let mut body: Bytes = Vec::new();
    let mut method_explicit = false;
    let mut positionals: Vec<Bytes> = Vec::new();
    let mut parsed_opts: Vec<(Bytes, Bytes, Bytes)> = Vec::new();

    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        if arg == b"--" {
            let rest_body = argv[i + 1..].join(&b" "[..]);
            if body.is_empty() { body = rest_body; }
            break;
        }
        if arg.len() > 1 && arg[0] == b'-' {
            let (flag, key, val): (Bytes, Bytes, Bytes) = if let Some(eq) = arg.iter().position(|b| *b == b'=') {
                let f = arg[..eq].to_vec();
                let v = arg[eq + 1..].to_vec();
                let k = f.strip_prefix(b"--").or_else(|| f.strip_prefix(b"-")).unwrap_or(&f).to_vec();
                (f, k, v)
            } else {
                let f = arg.clone();
                let k = f.strip_prefix(b"--").or_else(|| f.strip_prefix(b"-")).unwrap_or(&f).to_vec();
                let mut v = b"true".to_vec();
                if i + 1 < argv.len() && argv[i + 1].first() != Some(&b'-') {
                    let next = &argv[i + 1];
                    let is_special = matches!(k.as_slice(), b"method" | b"X" | b"body" | b"d");
                    let probe_path = if next.first() == Some(&b'/') { next.clone() } else { let mut p = vec![b'/']; p.extend_from_slice(next); p };
                    let next_is_root_cmd = positionals.is_empty() && !is_special && !matches!(shared.borrow().table.find("GET", &probe_path), Match::NotFound);
                    if !next_is_root_cmd {
                        v = argv[i + 1].clone();
                        i += 1;
                    }
                }
                (f, k, v)
            };
            if key == b"method" || key == b"X" {
                method = String::from_utf8_lossy(&val).to_ascii_uppercase();
                method_explicit = true;
            } else if key == b"body" || key == b"d" {
                body = val.clone();
            }
            parsed_opts.push((flag, key, val));
        } else {
            positionals.push(arg.clone());
        }
        i += 1;
    }

    let mut path: Bytes = if positionals.is_empty() {
        b"/".to_vec()
    } else {
        let mut joined = Vec::new();
        for (idx, seg) in positionals.iter().enumerate() {
            if idx == 0 && seg.first() == Some(&b'/') {
                joined.extend_from_slice(seg);
            } else {
                joined.push(b'/');
                joined.extend_from_slice(seg);
            }
        }
        if joined.is_empty() { b"/".to_vec() } else { joined }
    };
    let prefix = shared.borrow().normalized_prefix();
    if !prefix.is_empty() && !path.starts_with(&prefix) {
        path = if path == b"/" { prefix } else { let mut v = prefix; v.extend_from_slice(&path); v };
    }

    let mut query: HashMap<Bytes, Bytes> = HashMap::new();
    {
        let mut s = shared.borrow_mut();
        for (flag, key, val) in &parsed_opts {
            query.insert(key.clone(), val.clone());
            query.insert(flag.clone(), val.clone());
            s.state_values.insert(key.clone(), val.clone());
        }
    }

    let data = ExchangeData {
        request: Request { method: method.clone(), target: path.clone(), headers: Vec::new(), body: body.clone() },
        path: path.clone(),
        params: Vec::new(),
        query,
        status: 200,
        headers: Vec::new(),
        body: Vec::new(),
    };
    let existing_slot = shared.borrow().slot;
    let (obj, mut saved_outer) = match existing_slot {
        Some(obj) => {
            let prev = std::mem::replace(ctx.get_mut::<ExchangeData>(obj)?, data);
            (obj, Some(prev))
        }
        None => {
            let obj = ctx.insert(EXCHANGE, data);
            shared.borrow_mut().slot = Some(obj);
            (obj, None)
        }
    };
    let mut finish_slot = |ctx: &mut Context| -> Result<ExchangeData, Error> {
        let slot = ctx.get_mut::<ExchangeData>(obj)?;
        Ok(match saved_outer.take() {
            Some(prev) => std::mem::replace(slot, prev),
            None => std::mem::take(slot),
        })
    };

    for (flag, key, _) in &parsed_opts {
        let opt_entry = {
            let s = shared.borrow();
            s.cli_options.get(flag).or_else(|| s.cli_options.get(key)).cloned()
        };
        if let Some((idx, _)) = opt_entry {
            if let Err(err) = call_entry_isolated(program, registry, ctx, idx, obj) {
                let _ = finish_slot(ctx);
                return Err(err);
            }
        }
    }

    if shared.borrow().stopped {
        let done = finish_slot(ctx)?;
        return Ok(response_of(done).body);
    }

    let route_match = {
        let s = shared.borrow();
        let mut m = match s.table.find(&method, &path) {
            Match::Found { route, params } => Ok((route.annotation.clone(), params, method.clone())),
            Match::NotFound => Err(404u16),
            Match::NotAllowed(_) => Err(405u16),
        };
        if m == Err(405) && !method_explicit {
            let candidates: &[&str] = if !body.is_empty() {
                &["POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD"]
            } else {
                &["POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD"]
            };
            for cand in candidates {
                if let Match::Found { route, params } = s.table.find(cand, &path) {
                    m = Ok((route.annotation.clone(), params, (*cand).to_string()));
                    break;
                }
            }
        }
        match m {
            Ok((ann, params, final_method)) => match s.entry_index.get(&ann) {
                Some((idx, name)) => Ok((*idx, name.clone(), params, final_method)),
                None => Err(500),
            },
            Err(code) => Err(code),
        }
    };

    match route_match {
        Err(404) => {
            let done = finish_slot(ctx)?;
            if positionals.is_empty() && !done.body.is_empty() {
                return Ok(response_of(done).body);
            }
            Ok(error_response(404, &[]).body)
        }
        Err(status) => {
            let _ = finish_slot(ctx);
            Ok(error_response(status, &[]).body)
        }
        Ok((index, name, params, final_method)) => {
            {
                let slot = ctx.get_mut::<ExchangeData>(obj)?;
                slot.request.method = final_method;
                slot.params = params;
            }
            let outcome = call_entry_isolated(program, registry, ctx, index, obj);
            let done = finish_slot(ctx)?;
            if let Err(error) = outcome {
                let report = shared.borrow().on_handler_error.clone();
                report(&name, &error.message);
                return Ok(error_response(500, &[]).body);
            }
            Ok(response_of(done).body)
        }
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

/// Registers the 20 `rest` signatures in `registry` and returns the controller that owns the server.
pub fn register_rest(registry: &mut Registry, options: RestOptions) -> Result<RestController, Error> {
    let on_handler_error = options.on_handler_error.unwrap_or_else(|| Rc::new(|entry: &str, message: &str| eprintln!("rest: handler {} failed: {}", entry, message)));
    let shared = Rc::new(RefCell::new(Shared {
        state_values: HashMap::new(), state_counts: HashMap::new(), table: RouteTable::default(), entries: None,
        auto_routes_materialized: false, auto_entry_index: HashMap::new(), auto_entry_indices: HashSet::new(),
        entry_index: HashMap::new(), cli_options: HashMap::new(), config_prefix: Vec::new(), serving: false, stopped: false, in_controller_run: false,
        max_body_bytes: options.max_body_bytes, slot: None, active_program: None, active_registry: None, on_handler_error,
    }));
    let input_cell = Rc::new(RefCell::new(options.input));
    let output_cell = Rc::new(RefCell::new(options.output));

    let s_before = Rc::clone(&shared);
    let s_after = Rc::clone(&shared);
    let in_after = Rc::clone(&input_cell);
    let out_after = Rc::clone(&output_cell);
    registry.set_rest_hooks(
        Rc::new(move |program, _ctx, reg| {
            let mut s = s_before.borrow_mut();
            if s.in_controller_run { return Ok(false); }
            s.table = RouteTable::default();
            s.serving = false;
            s.stopped = false;
            s.bind_entries(program.entries().iter().map(|e| EntryInfo { name: e.name.clone(), annotation: e.annotation.clone(), iface: e.iface.clone() }).collect());
            s.active_program = Some(program as *const Program);
            s.active_registry = Some(reg as *const Registry);
            Ok(true)
        }),
        Rc::new(move |program, reg, ctx| {
            let serve = { let s = s_after.borrow(); s.serving && !s.stopped };
            if serve {
                let mut temp = RestController {
                    shared: Rc::clone(&s_after),
                    input: Rc::clone(&in_after),
                    output: Rc::clone(&out_after),
                };
                temp.pump(program, reg, ctx)?;
            }
            Ok(())
        }),
    );

    let s = Rc::clone(&shared);
    registry.override_std_function("rest_config", &["string", "string"], "void", move |_, a| {
        s.borrow_mut().config(a[0].as_bytes()?, a[1].as_bytes()?)?; Ok(Value::None)
    }, crate::dry(|_: &mut Context, _: &[Value]| Ok(Value::None)))?;
    let s = Rc::clone(&shared);
    registry.override_std_function("rest_route", &["string", "string", "string"], "void", move |_, a| {
        s.borrow_mut().route(a[0].as_bytes()?, a[1].as_bytes()?, a[2].as_bytes()?)?; Ok(Value::None)
    }, crate::dry(|_: &mut Context, _: &[Value]| Ok(Value::None)))?;
    let s = Rc::clone(&shared);
    registry.override_std_function("rest_serve", &[], "void", move |_, _| {
        s.borrow_mut().serve()?; Ok(Value::None)
    }, crate::dry(|_: &mut Context, _: &[Value]| Ok(Value::None)))?;
    let s = Rc::clone(&shared);
    registry.override_std_function("rest_stop", &[], "void", move |_, _| {
        s.borrow_mut().stopped = true; Ok(Value::None)
    }, crate::dry(|_: &mut Context, _: &[Value]| Ok(Value::None)))?;
    let s = Rc::clone(&shared);
    registry.override_std_function("rest_cli", &["StrList"], "string", move |ctx, a| {
        let list_ref = a[0].as_obj()?;
        let items: Vec<Bytes> = ctx.get::<StrList>(list_ref)?.items.iter().map(|b| b.as_bytes().to_vec()).collect();
        let out = dispatch_cli(&s, ctx, items)?;
        Ok(Value::string(out))
    }, crate::dry(|_: &mut Context, _: &[Value]| Ok(Value::string(""))))?;

    fn x<'a>(ctx: &'a mut Context, this: ObjRef) -> Result<&'a mut ExchangeData, Error> {
        ctx.get_mut::<ExchangeData>(this).map_err(|_| Error::host("invalid exchange"))
    }
    let t = EXCHANGE;
    registry.override_std_method(t, "method", &[], "string", |c, o, _| Ok(Value::string(x(c, o)?.request.method.clone())), crate::dry(crate::real))?;
    registry.override_std_method(t, "path", &[], "string", |c, o, _| Ok(Value::string(x(c, o)?.path.as_slice())), crate::dry(crate::real))?;
    registry.override_std_method(t, "query", &["string"], "string", |c, o, a| Ok(Value::string(x(c, o)?.query.get(a[0].as_bytes()?).cloned().unwrap_or_default())), crate::dry(crate::real))?;
    registry.override_std_method(t, "has_query", &["string"], "bool", |c, o, a| Ok(Value::Bool(x(c, o)?.query.contains_key(a[0].as_bytes()?))), crate::dry(crate::real))?;
    registry.override_std_method(t, "param", &["string"], "string", |c, o, a| {
        let want = a[0].as_bytes()?;
        Ok(Value::string(x(c, o)?.params.iter().find(|(n, _)| n.as_bytes() == want).map(|(_, v)| v.clone()).unwrap_or_default()))
    }, crate::dry(crate::real))?;
    registry.override_std_method(t, "header", &["string"], "string", |c, o, a| Ok(Value::string(x(c, o)?.request_headers(a[0].as_bytes()?).join(&b", "[..]))), crate::dry(crate::real))?;
    registry.override_std_method(t, "has_header", &["string"], "bool", |c, o, a| Ok(Value::Bool(!x(c, o)?.request_headers(a[0].as_bytes()?).is_empty())), crate::dry(crate::real))?;
    registry.override_std_method(t, "body", &[], "string", |c, o, _| Ok(Value::string(x(c, o)?.request.body.as_slice())), crate::dry(crate::real))?;
    registry.override_std_method(t, "status", &["int"], "void", |c, o, a| {
        let code = a[0].as_int()?;
        if !(200..=599).contains(&code) { return Err(Error::host(format!("rest: status {} out of range 200..599", code))); }
        x(c, o)?.status = code as u16; Ok(Value::None)
    }, crate::dry(crate::real))?;
    registry.override_std_method(t, "set_header", &["string", "string"], "void", |c, o, a| {
        let (name, value) = (a[0].as_bytes()?, a[1].as_bytes()?);
        if name.is_empty() || name.iter().any(|b| *b <= 0x20 || *b == 0x7f || *b == b':') || value.iter().any(|b| *b == b'\r' || *b == b'\n') {
            return Err(Error::host("rest: invalid response header"));
        }
        if is_reserved(&name.to_ascii_lowercase()) { return Err(Error::host(format!("rest: response header {} is managed by the server", text(name)))); }
        x(c, o)?.set_header(name, value); Ok(Value::None)
    }, crate::dry(crate::real))?;
    registry.override_std_method(t, "send", &["string"], "void", |c, o, a| { x(c, o)?.body = a[0].as_bytes()?.to_vec(); Ok(Value::None) }, crate::dry(crate::real))?;
    registry.override_std_method(t, "json", &["string"], "void", |c, o, a| {
        let e = x(c, o)?; e.body = a[0].as_bytes()?.to_vec(); e.set_header(b"Content-Type", b"application/json"); Ok(Value::None)
    }, crate::dry(crate::real))?;
    let s = Rc::clone(&shared);
    registry.override_std_method(t, "get", &["string"], "string", move |_, _, a| Ok(Value::string(s.borrow().state_values.get(a[0].as_bytes()?).cloned().unwrap_or_default())), crate::dry(crate::real))?;
    let s = Rc::clone(&shared);
    registry.override_std_method(t, "set", &["string", "string"], "void", move |_, _, a| {
        s.borrow_mut().state_values.insert(a[0].as_bytes()?.to_vec(), a[1].as_bytes()?.to_vec()); Ok(Value::None)
    }, crate::dry(crate::real))?;
    let s = Rc::clone(&shared);
    registry.override_std_method(t, "increment", &["string"], "int", move |_, _, a| {
        let mut s = s.borrow_mut();
        let n = s.state_counts.entry(a[0].as_bytes()?.to_vec()).or_insert(0);
        *n = n.wrapping_add(1); Ok(Value::Int(*n))
    }, crate::dry(crate::real))?;
    Ok(RestController { shared, input: input_cell, output: output_cell })
}

#[cfg(test)]
mod tests;
