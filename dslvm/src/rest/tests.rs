//! Unit tests of the `rest` module: route templates, matching, HTTP framing, registration, serve validation and exchange rules.
//! The shared conformance suite is replayed in `tests/rest_suite.rs`.
use super::http::{body_length, encode_response, parse_head, take_request, Response, Taken};
use super::router::{parse_route, percent_decode, Match, RouteTable};
use super::*;

fn err_text<T>(r: Result<T, Error>) -> String { match r { Err(e) => { assert_eq!(e.code, "RUN_HOST"); e.message } Ok(_) => panic!("expected an error") } }
fn found(table: &RouteTable, method: &str, path: &str) -> Option<String> {
    match table.find(method, path.as_bytes()) { Match::Found { route, .. } => Some(String::from_utf8(route.annotation.clone()).unwrap()), _ => None }
}
fn bytes(s: &str) -> Vec<u8> { s.as_bytes().to_vec() }

const IFACE: &str = HANDLER_INTERFACE;
/// A controller whose "program" has the given entries (annotation, interface, function name); no VM is needed to validate.
fn controller_with(entries: &[(&str, &str, &str)]) -> RestController {
    let mut registry = Registry::new();
    let controller = register_rest(&mut registry, RestOptions::default()).unwrap();
    controller.shared.borrow_mut().entries = Some(entries.iter().map(|(a, i, n)| EntryInfo { annotation: a.to_string(), iface: i.to_string(), name: n.to_string() }).collect());
    controller
}
fn route(c: &RestController, method: &str, path: &str, annotation: &str) -> Result<(), Error> { c.shared.borrow_mut().route(method.as_bytes(), path.as_bytes(), annotation.as_bytes()) }
fn serve(c: &RestController) -> Result<(), Error> { c.shared.borrow_mut().serve() }

// ---- routes -------------------------------------------------------------------------------------------------
#[test]
fn route_templates_valid_shapes_parse_into_literal_param_and_rest_segments() {
    use router::Segment::*;
    let r = parse_route(b"GET", b"/users/{id}/files/{path...}").unwrap();
    assert_eq!(r.segments, vec![Literal(bytes("users")), Param("id".into()), Literal(bytes("files")), Rest("path".into())]);
    assert_eq!(parse_route(b"GET", b"/").unwrap().segments.len(), 1);
    assert_eq!(parse_route(b"GET", b"/files/").unwrap().segments.last(), Some(&Literal(vec![])));
}

#[test]
fn route_templates_invalid_shapes_are_run_host_errors_with_pinned_text() {
    let bad = |method: &str, path: &str, want: &str| {
        let m = err_text(parse_route(method.as_bytes(), path.as_bytes()));
        assert!(m.starts_with(&format!("rest: invalid route {} {}: ", method, path)) && m.contains(want), "{} {} -> {}", method, path, m);
    };
    bad("get", "/x", "method must be one of GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS");
    bad("GET", "x", "path must start with '/'");
    bad("GET", "/a b", "path must not contain whitespace, control characters, ? or #");
    bad("GET", "/a?b", "whitespace, control characters, ? or #");
    bad("GET", "/a#b", "whitespace, control characters, ? or #");
    bad("GET", "/a//b", "empty segment ('//')");
    bad("GET", "/{1x}", "invalid parameter name '1x'");
    bad("GET", "/{}", "invalid parameter name ''");
    bad("GET", "/{a}/{a}", "duplicate parameter 'a'");
    bad("GET", "/{a...}/x", "'{a...}' must be the last segment");
    bad("GET", "/a{b}", "malformed placeholder in segment 'a{b}'");
    bad("GET", "/{b", "malformed placeholder");
    parse_route(b"GET", "/caf\u{e0}".as_bytes()).unwrap(); // UTF-8 bytes such as C3 A0 are not whitespace
    parse_route(b"GET", b"/caf\xa0").unwrap(); // neither is a lone 0xA0 byte
}

#[test]
fn route_table_duplicates_are_detected_by_shape_parameter_names_do_not_matter() {
    let mut t = RouteTable::default();
    t.add(b"GET", b"/a/{x}", b"one").unwrap();
    t.add(b"POST", b"/a/{x}", b"two").unwrap();
    assert_eq!(err_text(t.add(b"GET", b"/a/{y}", b"three")), "rest: duplicate route GET /a/{y}");
    t.add(b"GET", b"/a/b", b"literal").unwrap();
}

#[test]
fn route_table_specificity_is_literal_then_param_then_rest_first_difference_wins() {
    let mut t = RouteTable::default();
    for (path, name) in [("/x/{rest...}", "rest"), ("/x/{p}/z", "param-z"), ("/x/{p}/{q}", "param-param"), ("/x/y/{q}", "literal-param"), ("/x/y/z", "literal-literal")] {
        t.add(b"GET", path.as_bytes(), name.as_bytes()).unwrap();
    }
    let pick = |p: &str| found(&t, "GET", p).unwrap();
    assert_eq!(pick("/x/y/z"), "literal-literal");
    assert_eq!(pick("/x/y/w"), "literal-param");
    assert_eq!(pick("/x/a/z"), "param-z");
    assert_eq!(pick("/x/a/b"), "param-param");
    assert_eq!(pick("/x/a/b/c"), "rest");
    assert_eq!(pick("/x/"), "rest");
    assert!(matches!(t.find("GET", b"/x"), Match::NotFound));
}

#[test]
fn route_table_404_vs_405_allow_header_and_head_fallback() {
    let mut t = RouteTable::default();
    t.add(b"GET", b"/r", b"g").unwrap(); t.add(b"DELETE", b"/r", b"d").unwrap(); t.add(b"PUT", b"/only-put", b"p").unwrap();
    assert!(matches!(t.find("POST", b"/r"), Match::NotAllowed(a) if a == "DELETE, GET, HEAD"));
    assert!(matches!(t.find("GET", b"/only-put"), Match::NotAllowed(a) if a == "PUT"));
    assert!(matches!(t.find("GET", b"/missing"), Match::NotFound));
    assert_eq!(found(&t, "HEAD", "/r").as_deref(), Some("g"));
    assert!(matches!(t.find("HEAD", b"/only-put"), Match::NotAllowed(_)));
    assert!(matches!(t.find("OPTIONS", b"/r"), Match::NotAllowed(_)), "OPTIONS is never implicit");
}

#[test]
fn route_table_encoded_slashes_stay_in_one_segment_parameters_are_decoded() {
    let mut t = RouteTable::default();
    t.add(b"GET", b"/f/{name}", b"f").unwrap();
    match t.find("GET", b"/f/a%2Fb") { Match::Found { params, .. } => assert_eq!(params, vec![("name".to_string(), bytes("a/b"))]), _ => panic!() }
    assert!(matches!(t.find("GET", b"/f/a%2Fb/c"), Match::NotFound));
    assert!(matches!(t.find("GET", b"/f/%zz"), Match::NotFound));
    assert!(matches!(t.find("GET", b"/f/"), Match::NotFound), "an empty segment is not a {{param}}");
    assert!(matches!(t.find("GET", b"/f"), Match::NotFound));
}

#[test]
fn percent_decoding_yields_bytes_plus_only_in_queries_and_rejects_bad_escapes() {
    assert_eq!(percent_decode(b"%C3%B6", false).unwrap(), vec![0xc3, 0xb6]);
    assert_eq!(percent_decode(b"%ff%00", false).unwrap(), vec![0xff, 0]);
    assert_eq!(percent_decode(b"a+b", true).unwrap(), bytes("a b"));
    assert_eq!(percent_decode(b"a+b", false).unwrap(), bytes("a+b"));
    for bad in [&b"%4"[..], b"%", b"%zz", b"%4g"] { assert_eq!(percent_decode(bad, false), None); }
}

// ---- HTTP subset ----------------------------------------------------------------------------------------------
#[test]
fn http_request_heads() {
    let (m, t, h) = parse_head(b"GET /a?b=1 HTTP/1.1\r\nHost: x\r\nX-A: \t v \t ").unwrap();
    assert_eq!((m.as_str(), t), ("GET", bytes("/a?b=1")));
    assert_eq!(h, vec![(bytes("Host"), bytes("x")), (bytes("X-A"), bytes("v"))]);
    for bad in ["garbage", "GET /a HTTP/2.0", "GET a HTTP/1.1", "GET /a#f HTTP/1.1", "get /a HTTP/1.1", "GET /a HTTP/1.1\r\nNoColon", "GET /a HTTP/1.1\r\n: v", "GET /a HTTP/1.1\r\nBad Name: v", "GET /a b HTTP/1.1", "GET /a HTTP/1.1\r\nX: a\x01b", ""] {
        assert_eq!(parse_head(bad.as_bytes()).unwrap_err(), 400, "{:?}", bad);
    }
    // 0xA0 is an ordinary byte: allowed in targets, header names and values; only ASCII blanks are not
    assert!(parse_head(b"GET /caf\xa0 HTTP/1.1\r\nX\xa0Y: v\xa0").is_ok());
    assert_eq!(parse_head(b"GET /a HTTP/1.1\r\nX: v\xa0").unwrap().2[0].1, b"v\xa0".to_vec(), "values are trimmed of spaces and tabs only");
}

#[test]
fn http_content_length_rules() {
    let h = |pairs: &[(&str, &str)]| -> Vec<(Vec<u8>, Vec<u8>)> { pairs.iter().map(|(n, v)| (bytes(n), bytes(v))).collect() };
    assert_eq!(body_length(&[]), Ok(0));
    assert_eq!(body_length(&h(&[("content-length", "12")])), Ok(12));
    assert_eq!(body_length(&h(&[("Content-Length", "1"), ("content-length", "1")])), Ok(1));
    assert_eq!(body_length(&h(&[("Content-Length", "1"), ("content-length", "2")])), Err(400));
    for v in ["-1", "1.5", "abc", "", &"1".repeat(16)] { assert_eq!(body_length(&h(&[("Content-Length", v)])), Err(400), "{:?}", v); }
    assert_eq!(body_length(&h(&[("Transfer-Encoding", "chunked")])), Err(501));
}

#[test]
fn http_response_encoding_has_no_connection_header_and_head_keeps_content_length() {
    let r = |status, body: &str| Response { status, headers: vec![(bytes("A"), bytes("b"))], body: bytes(body) };
    assert_eq!(encode_response(&r(200, "hi"), false), b"HTTP/1.1 200 OK\r\nA: b\r\nContent-Length: 2\r\n\r\nhi".to_vec());
    assert_eq!(encode_response(&r(299, "hi"), true), b"HTTP/1.1 299 OK\r\nA: b\r\nContent-Length: 2\r\n\r\n".to_vec());
}

#[test]
fn take_request_incremental_framing_fatal_errors_and_pipelined_remainders() {
    let full = b"POST /a HTTP/1.1\r\nContent-Length: 3\r\n\r\nabcGET /b HTTP/1.1\r\n\r\n";
    let Taken::Request(one, used) = take_request(full, 1024) else { panic!() };
    assert_eq!(one.body, bytes("abc"));
    let Taken::Request(two, _) = take_request(&full[used..], 1024) else { panic!() };
    assert_eq!(two.target, bytes("/b"));
    assert_eq!(take_request(b"GET /a HTT", 1024), Taken::Need);
    assert_eq!(take_request(b"POST /a HTTP/1.1\r\nContent-Length: 5\r\n\r\nab", 1024), Taken::Need);
    assert_eq!(take_request(b"POST /a HTTP/1.1\r\nContent-Length: 2000\r\n\r\n", 1024), Taken::Fatal(413));
    assert_eq!(take_request(&vec![b'a'; 20000], 1024), Taken::Fatal(431));
    assert_eq!(take_request(b"nonsense\r\n\r\n", 1024), Taken::Fatal(400));
    assert_eq!(take_request(b"POST /a HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n", 1024), Taken::Fatal(501));
}

// ---- registration, validation, lifecycle ------------------------------------------------------------------------
#[test]
fn registration_exactly_the_18_declared_signatures_absent_from_the_default_registry() {
    let mut declared = vec![
        "fn rest_route(string, string, string) -> void", "fn rest_serve() -> void", "fn rest_stop() -> void",
        "method RestExchange.method() -> string", "method RestExchange.path() -> string", "method RestExchange.query(string) -> string",
        "method RestExchange.has_query(string) -> bool", "method RestExchange.param(string) -> string", "method RestExchange.header(string) -> string",
        "method RestExchange.has_header(string) -> bool", "method RestExchange.body() -> string", "method RestExchange.status(int) -> void",
        "method RestExchange.set_header(string, string) -> void", "method RestExchange.send(string) -> void", "method RestExchange.json(string) -> void",
        "method RestExchange.get(string) -> string", "method RestExchange.set(string, string) -> void", "method RestExchange.increment(string) -> int",
    ];
    declared.sort();
    assert_eq!(declared.len(), 18);
    let mut registry = Registry::new();
    assert_eq!(registry.signatures().len(), 206, "default catalog is unchanged");
    assert!(!registry.signatures().iter().any(|s| s.contains("rest_") || s.contains("RestExchange")));
    let before = registry.signatures();
    register_rest(&mut registry, RestOptions::default()).unwrap();
    let mut added: Vec<String> = registry.signatures().into_iter().filter(|s| !before.contains(s)).collect();
    added.sort();
    assert_eq!(added, declared);
    assert_eq!(registry.signatures().len(), 224);
    assert!(Registry::new().signatures() == before, "a second default registry is still unchanged");
}

#[test]
fn serve_validation_happens_before_any_byte_is_read() {
    let none = controller_with(&[]);
    assert_eq!(err_text(serve(&none)), "rest: no routes registered");
    let missing = controller_with(&[]);
    route(&missing, "GET", "/a", "nope").unwrap();
    assert_eq!(err_text(serve(&missing)), "rest: route GET /a references missing annotation @nope");
    let wrong = controller_with(&[("a", "interface Rule(Node) -> void", "check")]);
    route(&wrong, "GET", "/a", "a").unwrap();
    assert_eq!(err_text(serve(&wrong)), "rest: entry check must implement RestHandler(RestExchange) -> void");
    let dup = controller_with(&[("a", IFACE, "one"), ("a", IFACE, "two")]);
    route(&dup, "GET", "/a", "a").unwrap();
    assert_eq!(err_text(serve(&dup)), "rest: duplicate annotation @a");
    let mut registry = Registry::new();
    let unbound = register_rest(&mut registry, RestOptions::default()).unwrap();
    route(&unbound, "GET", "/a", "a").unwrap();
    assert_eq!(err_text(serve(&unbound)), "rest: no program is bound (run the program through the controller)");
    for c in [&none, &missing, &wrong, &dup, &unbound] { assert!(!c.serving()); }
}

#[test]
fn serve_validation_order_is_already_called_then_unbound_then_routes() {
    let c = controller_with(&[("a", IFACE, "a")]);
    route(&c, "GET", "/a", "a").unwrap();
    serve(&c).unwrap();
    assert_eq!(err_text(serve(&c)), "rest: rest_serve already called");
    let mut registry = Registry::new();
    let unbound = register_rest(&mut registry, RestOptions::default()).unwrap(); // no routes AND no program: unbound wins
    assert!(err_text(serve(&unbound)).starts_with("rest: no program is bound"));
}

#[test]
fn serve_validation_ignores_entries_that_implement_other_interfaces() {
    let c = controller_with(&[("a", IFACE, "a"), ("other", "interface Rule(Node) -> void", "other"), ("a", "interface Rule(Node) -> void", "dup")]);
    route(&c, "GET", "/a", "a").unwrap();
    serve(&c).unwrap();
    assert!(c.serving());
}

#[test]
fn lifecycle_routes_freeze_after_rest_serve_a_second_rest_serve_fails() {
    let c = controller_with(&[("a", IFACE, "a")]);
    route(&c, "GET", "/a", "a").unwrap();
    serve(&c).unwrap();
    assert_eq!(err_text(route(&c, "GET", "/b", "a")), "rest: routes are frozen once rest_serve has been called");
    assert_eq!(err_text(serve(&c)), "rest: rest_serve already called");
}

// ---- exchange rules ---------------------------------------------------------------------------------------------
struct Harness { registry: Registry, ctx: Context, obj: ObjRef }
impl Harness {
    fn new() -> Harness {
        let mut registry = Registry::new();
        register_rest(&mut registry, RestOptions::default()).unwrap();
        let mut ctx = Context::new();
        let obj = ctx.insert(EXCHANGE, ExchangeData { status: 200, ..Default::default() });
        Harness { registry, ctx, obj }
    }
    fn call(&mut self, signature: &str, args: &[Value]) -> Result<Value, Error> {
        let (_, _, _, f, _) = self.registry.methods.get(&format!("method RestExchange.{}", signature)).expect("registered");
        f(&mut self.ctx, self.obj, args)
    }
    fn data(&self) -> &ExchangeData { self.ctx.get::<ExchangeData>(self.obj).unwrap() }
}
fn s(v: &str) -> Value { Value::string(v) }

#[test]
fn exchange_rules_status_range_header_validation_reserved_headers_counters_vs_strings() {
    let mut h = Harness::new();
    assert_eq!(err_text(h.call("status(int) -> void", &[Value::Int(199)])), "rest: status 199 out of range 200..599");
    assert!(err_text(h.call("status(int) -> void", &[Value::Int(600)])).contains("out of range"));
    h.call("status(int) -> void", &[Value::Int(599)]).unwrap();
    assert_eq!(h.data().status, 599);
    let set = "set_header(string, string) -> void";
    for name in ["", "a b", "a:b", "a\tb", "a\x7fb"] {
        assert_eq!(err_text(h.call(set, &[s(name), s("v")])), "rest: invalid response header", "{:?}", name);
    }
    assert_eq!(err_text(h.call(set, &[s("X"), s("a\r\nInjected: 1")])), "rest: invalid response header");
    for name in ["Content-Length", "connection", "TRANSFER-ENCODING"] {
        assert_eq!(err_text(h.call(set, &[s(name), s("1")])), format!("rest: response header {} is managed by the server", name));
    }
    h.call(set, &[s("X-A"), s("1")]).unwrap();
    h.call(set, &[s("x-a"), s("2")]).unwrap();
    h.call(set, &[s("caf\u{e0}"), s("0xA0 is an ordinary name byte")]).unwrap();
    assert_eq!(h.data().headers.len(), 2, "same name replaces, case-insensitively");
    assert_eq!((h.data().headers[0].1.clone(), h.data().headers[0].2.clone()), (bytes("x-a"), bytes("2")));
    let inc = "increment(string) -> int";
    assert_eq!(h.call(inc, &[s("k")]).unwrap(), Value::Int(1));
    assert_eq!(h.call(inc, &[s("k")]).unwrap(), Value::Int(2));
    assert_eq!(h.call("get(string) -> string", &[s("k")]).unwrap(), s(""), "counters and strings are separate maps");
    h.call("set(string, string) -> void", &[s("k"), s("v")]).unwrap();
    assert_eq!(h.call("get(string) -> string", &[s("k")]).unwrap(), s("v"));
    assert_eq!(h.call(inc, &[s("k")]).unwrap(), Value::Int(3));
}

#[test]
fn exchange_rules_increment_wraps_as_int64() {
    let mut registry = Registry::new();
    let controller = register_rest(&mut registry, RestOptions::default()).unwrap();
    controller.shared.borrow_mut().state_counts.insert(bytes("big"), i64::MAX);
    let mut ctx = Context::new();
    let obj = ctx.insert(EXCHANGE, ExchangeData::default());
    let (_, _, _, f, _) = registry.methods.get("method RestExchange.increment(string) -> int").unwrap();
    assert_eq!(f(&mut ctx, obj, &[s("big")]).unwrap(), Value::Int(i64::MIN));
}

#[test]
fn exchange_responses_default_content_type_status_204_and_header_order() {
    let mut h = Harness::new();
    h.call("send(string) -> void", &[s("hi")]).unwrap();
    h.call("set_header(string, string) -> void", &[s("X-First"), s("1")]).unwrap();
    let r = response_of(std::mem::take(h.ctx.get_mut::<ExchangeData>(h.obj).unwrap()));
    assert_eq!(r.headers[0], (bytes("Content-Type"), bytes("text/plain; charset=utf-8")));
    assert_eq!(r.headers[1].0, bytes("X-First"));
    let mut h = Harness::new();
    h.call("status(int) -> void", &[Value::Int(204)]).unwrap();
    h.call("json(string) -> void", &[s("{}")]).unwrap();
    let r = response_of(std::mem::take(h.ctx.get_mut::<ExchangeData>(h.obj).unwrap()));
    assert!(r.body.is_empty());
    assert_eq!(r.headers, vec![(bytes("Content-Type"), bytes("application/json"))]);
}
