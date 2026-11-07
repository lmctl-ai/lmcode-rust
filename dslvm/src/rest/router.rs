//! Route templates and matching for the `rest` std module (spec/std-rest.md, "Route templates").
//! Every string is an lmcode byte string. Whitespace means ASCII only (0x00-0x20, 0x7f).
use crate::Error;

pub const METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

fn fail<T>(message: String) -> Result<T, Error> { Err(Error::host(message)) }
fn text(bytes: &[u8]) -> String { String::from_utf8_lossy(bytes).into_owned() }
/// ASCII whitespace and control bytes: 0x00-0x20 and 0x7f.
pub fn is_blank(b: u8) -> bool { b <= 0x20 || b == 0x7f }

/// Percent-decodes a byte string; `None` on an invalid escape. `plus` turns `+` into a space (queries).
pub fn percent_decode(input: &[u8], plus: bool) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        match input[i] {
            b'%' => {
                let hi = input.get(i + 1).and_then(|c| (*c as char).to_digit(16))?;
                let lo = input.get(i + 2).and_then(|c| (*c as char).to_digit(16))?;
                out.push((hi * 16 + lo) as u8);
                i += 3;
            }
            b'+' if plus => { out.push(b' '); i += 1; }
            c => { out.push(c); i += 1; }
        }
    }
    Some(out)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Segment { Literal(Vec<u8>), Param(String), Rest(String) }
impl Segment {
    fn rank(&self) -> u8 { match self { Segment::Literal(_) => 0, Segment::Param(_) => 1, Segment::Rest(_) => 2 } }
}

#[derive(Debug, Clone)]
pub struct Route { pub method: String, pub path: Vec<u8>, pub segments: Vec<Segment>, pub shape: Vec<u8>, pub annotation: Vec<u8> }

fn valid_name(name: &[u8]) -> bool {
    matches!(name.first(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'_')) && name.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

pub fn parse_route(method: &[u8], path: &[u8]) -> Result<Route, Error> {
    let label = format!("{} {}", text(method), text(path));
    let invalid = |reason: String| -> Result<Route, Error> { fail(format!("rest: invalid route {}: {}", label, reason)) };
    let Some(method) = METHODS.iter().find(|m| m.as_bytes() == method) else { return invalid(format!("method must be one of {}", METHODS.join(", "))) };
    if path.first() != Some(&b'/') { return invalid("path must start with '/'".into()); }
    if path.iter().any(|c| is_blank(*c) || *c == b'?' || *c == b'#') { return invalid("path must not contain whitespace, control characters, ? or #".into()); }
    let raw: Vec<&[u8]> = path[1..].split(|c| *c == b'/').collect();
    let mut names: Vec<String> = Vec::new();
    let mut segments = Vec::new();
    for (index, part) in raw.iter().enumerate() {
        let last = index == raw.len() - 1;
        if part.is_empty() && !last { return invalid("empty segment ('//')".into()); }
        if part.len() >= 2 && part[0] == b'{' && part[part.len() - 1] == b'}' {
            let inner = &part[1..part.len() - 1];
            let (name, rest) = match inner.strip_suffix(b"...") { Some(n) => (n, true), None => (inner, false) };
            if !valid_name(name) { return invalid(format!("invalid parameter name '{}'", text(name))); }
            let name = text(name);
            if names.contains(&name) { return invalid(format!("duplicate parameter '{}'", name)); }
            names.push(name.clone());
            if rest && !last { return invalid(format!("'{{{}...}}' must be the last segment", name)); }
            segments.push(if rest { Segment::Rest(name) } else { Segment::Param(name) });
        } else {
            if part.iter().any(|c| *c == b'{' || *c == b'}') { return invalid(format!("malformed placeholder in segment '{}'", text(part))); }
            segments.push(Segment::Literal(part.to_vec()));
        }
    }
    let mut shape = Vec::new();
    for (i, s) in segments.iter().enumerate() {
        if i > 0 { shape.push(b'/'); }
        match s { Segment::Literal(v) => { shape.push(b'L'); shape.extend_from_slice(v); } Segment::Param(_) => shape.push(b'P'), Segment::Rest(_) => shape.push(b'R') }
    }
    Ok(Route { method: method.to_string(), path: path.to_vec(), segments, shape, annotation: Vec::new() })
}

fn specificity(a: &Route, b: &Route) -> std::cmp::Ordering {
    for (x, y) in a.segments.iter().zip(&b.segments) {
        let d = x.rank().cmp(&y.rank());
        if d != std::cmp::Ordering::Equal { return d; }
    }
    a.segments.len().cmp(&b.segments.len())
}

fn match_segments(route: &Route, raw: &[&[u8]]) -> Option<Vec<(String, Vec<u8>)>> {
    let mut params = Vec::new();
    for (i, seg) in route.segments.iter().enumerate() {
        if let Segment::Rest(name) = seg {
            if raw.len() <= i { return None; }
            let mut parts = Vec::new();
            for r in &raw[i..] { parts.push(percent_decode(r, false)?); }
            params.push((name.clone(), parts.join(&b'/')));
            return Some(params);
        }
        let decoded = percent_decode(raw.get(i)?, false)?;
        match seg {
            Segment::Literal(v) => { if decoded != *v { return None; } }
            Segment::Param(name) => { if decoded.is_empty() { return None; } params.push((name.clone(), decoded)); }
            Segment::Rest(_) => unreachable!(),
        }
    }
    (raw.len() == route.segments.len()).then_some(params)
}

#[derive(Debug)]
pub enum Match<'a> { Found { route: &'a Route, params: Vec<(String, Vec<u8>)> }, NotFound, NotAllowed(String) }

#[derive(Default)]
pub struct RouteTable { pub routes: Vec<Route> }
impl RouteTable {
    pub fn len(&self) -> usize { self.routes.len() }
    pub fn is_empty(&self) -> bool { self.routes.is_empty() }
    pub fn add(&mut self, method: &[u8], path: &[u8], annotation: &[u8]) -> Result<(), Error> {
        let mut route = parse_route(method, path)?;
        if self.routes.iter().any(|r| r.method == route.method && r.shape == route.shape) {
            return fail(format!("rest: duplicate route {} {}", text(method), text(path)));
        }
        route.annotation = annotation.to_vec();
        self.routes.push(route);
        Ok(())
    }
    /// `raw_path` is the undecoded path without the query.
    pub fn find(&self, method: &str, raw_path: &[u8]) -> Match<'_> {
        let raw: Vec<&[u8]> = raw_path.get(1..).unwrap_or(&[]).split(|c| *c == b'/').collect();
        let hits: Vec<(&Route, Vec<(String, Vec<u8>)>)> = self.routes.iter().filter_map(|r| match_segments(r, &raw).map(|p| (r, p))).collect();
        if hits.is_empty() { return Match::NotFound; }
        let best = |verb: &str| hits.iter().filter(|(r, _)| r.method == verb).min_by(|a, b| specificity(a.0, b.0));
        let direct = best(method).or_else(|| if method == "HEAD" { best("GET") } else { None });
        if let Some((route, params)) = direct { return Match::Found { route, params: params.clone() }; }
        let mut allow: Vec<String> = hits.iter().map(|(r, _)| r.method.clone()).collect();
        if allow.iter().any(|m| m == "GET") { allow.push("HEAD".into()); }
        allow.sort(); allow.dedup();
        Match::NotAllowed(allow.join(", "))
    }
}
