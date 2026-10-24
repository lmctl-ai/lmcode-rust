#!/usr/bin/env bash
# End-to-end test of the Rust router host. Needs cargo (1.92 used), curl and the released lmscript CLI
# (`npm install -g @lmctl-ai/lmctl`, >= 0.2.346; LMSCRIPT_BIN overrides). Proves that the UNCHANGED bytecode compiled from
# lmweb's router script runs on this Rust runtime and, when ../../../lmweb/build/server exists, on the C++ runtime too.
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; ROOT="$(cd "$HERE/../.." && pwd)"
LMWEB_DIR="${LMWEB_DIR:-}"
if [ -z "$LMWEB_DIR" ]; then
  for candidate in "$ROOT/../lmweb" "$ROOT/../poc-web"; do
    if [ -d "$candidate" ]; then LMWEB_DIR="$candidate"; break; fi
  done
fi
LMSCRIPT="${LMSCRIPT_BIN:-}"
if [ -z "$LMSCRIPT" ]; then LMSCRIPT=lmscript; command -v lmscript >/dev/null 2>&1 || LMSCRIPT=lmctl; fi; MIN_LMSCRIPT="${LMSCRIPT_MIN:-0.2.346}"
if ! command -v "$LMSCRIPT" >/dev/null 2>&1 && [ ! -f "$LMSCRIPT" ]; then echo "FAIL: lmscript not found ($LMSCRIPT). Install the released CLI: npm install -g @lmctl-ai/lmctl (>= $MIN_LMSCRIPT), or set LMSCRIPT_BIN." >&2; exit 1; fi
HAVE_LMSCRIPT="$("$LMSCRIPT" --version 2>/dev/null | sed -n 's/^lm\(script\|ctl\) //p')"
if [ -z "$HAVE_LMSCRIPT" ] || [ "$(printf '%s\n%s\n' "$MIN_LMSCRIPT" "$HAVE_LMSCRIPT" | sort -V | head -1)" != "$MIN_LMSCRIPT" ]; then echo "FAIL: lmscript ${HAVE_LMSCRIPT:-unknown} at $LMSCRIPT is too old: _dsl_ needs >= $MIN_LMSCRIPT (npm install -g @lmctl-ai/lmctl@latest) or set LMSCRIPT_BIN." >&2; exit 1; fi
WORK="$(mktemp -d "${TMPDIR:-/tmp}/rust-router-test-XXXXXX")"; mkdir -p "$WORK/home"
PIDS=""
fail() { echo "FAIL: $*" >&2; exit 1; }
pass=0; ok() { pass=$((pass + 1)); echo "ok $pass - $*"; }
cleanup() { for p in $PIDS; do kill -TERM "$p" 2>/dev/null; wait "$p" 2>/dev/null; done; rm -rf "$WORK"; }
trap cleanup EXIT
SCHEMA="$HERE/schema/router.dsl.json"
export ROUTER_TZ_OFFSET_SECONDS="$(date +%z | awk '{s=substr($1,1,1)=="-"?-1:1; h=substr($1,2,2); m=substr($1,4,2); print s*(h*3600+m*60)}')"
TODAY="$(date +%F)"

# 1. Build the host (debug is fine; release for numbers) and the conformance runner's pin check.
nice -n 10 cargo build -q -p router --manifest-path "$ROOT/Cargo.toml" 2>"$WORK/build.err" || { cat "$WORK/build.err"; fail "cargo build"; }
SERVER="$ROOT/target/debug/router"
ok "Rust router builds (dslvm crate, std::net, no external crates)"
bash "$ROOT/scripts/sync-vectors.sh" --verify >/dev/null || fail "conformance vectors differ from vectors.lock"
ok "conformance vectors match vectors.lock"

# 1b. DSL host flag standard (lmscript R42): the schema is embedded; introspection flags are exact, --schema is gone.
"$SERVER" --dsl-schema --json >"$WORK/embedded.json" || fail "--dsl-schema --json exit"
cmp -s "$WORK/embedded.json" "$SCHEMA" || fail "--dsl-schema --json differs from schema/router.dsl.json"
ok "--dsl-schema --json prints the embedded schema bytes exactly"
"$SERVER" --dsl-signatures >"$WORK/sigs.txt" || fail "--dsl-signatures exit"
{ grep -E '^(context|fn) ' "$WORK/sigs.txt"
  sed -n 's/^method //p' "$WORK/sigs.txt" | awk -F. '$1 != owner { owner = $1; print owner ":" } { print "  " $0 }'
} >"$WORK/summary.expect"
"$SERVER" --dsl-schema >"$WORK/summary.txt" || fail "--dsl-schema exit"
cmp -s "$WORK/summary.txt" "$WORK/summary.expect" || { diff "$WORK/summary.expect" "$WORK/summary.txt"; fail "--dsl-schema summary format"; }
grep -q '^Response:$' "$WORK/summary.txt" && grep -q '^  Response.status(int) -> void$' "$WORK/summary.txt" || fail "summary groups methods by owner"
ok "--dsl-schema prints the shared summary (context/fn lines, then Owner: and indented Owner.method lines)"
if "$LMSCRIPT" --dsl-signatures "$SCHEMA" >"$WORK/lmscript-sigs.txt" 2>/dev/null; then
  python3 "$ROOT/scripts/merge-std.py" "$SCHEMA" "$WORK/router-std.schema.json" --std "$ROOT/std/lmcode-std.schema.json" || fail "merge canonical std schema"
  "$LMSCRIPT" --dsl-signatures "$WORK/router-std.schema.json" >"$WORK/lmscript-std-sigs.txt" 2>/dev/null || fail "lmscript canonical std signatures"
  cmp -s "$WORK/sigs.txt" "$WORK/lmscript-std-sigs.txt" || { diff "$WORK/lmscript-std-sigs.txt" "$WORK/sigs.txt"; fail "--dsl-signatures differs from merged lmscript schema"; }
  ok "--dsl-signatures equals lmscript --dsl-signatures merged with canonical std"
fi
for flags in "--schema $SCHEMA" "--dsl $WORK/x.dslb --schema $SCHEMA" "--schema" "--json" "--dsl-schema --json extra" "--json --dsl-schema" \
             "--dsl-schema $SCHEMA" "--dsl-schema --dsl $WORK/x.dslb" "--dsl $WORK/x.dslb --dsl-schema" "--dsl $WORK/x.dslb --json" \
             "--dsl-signatures --json" "--port 0" ""; do
  out="$("$SERVER" $flags 2>/dev/null)"; code=$?
  [ "$code" = 2 ] && [ -z "$out" ] || fail "router $flags: exit $code, stdout '$out' (want 2, empty)"
done
ok "--schema FILE and every other flag combination are refused (exit 2, empty stdout)"

# 2. Compile lmweb's router chunk with the released lmscript (same schema bytes as lmweb).
if [ -n "$LMWEB_DIR" ] && [ -f "$LMWEB_DIR/schema/router.dsl.json" ]; then
  cmp -s "$SCHEMA" "$LMWEB_DIR/schema/router.dsl.json" || fail "router schema differs from the sibling lmweb schema"
  ok "schema is byte-identical to lmweb's schema/router.dsl.json"
else
  echo "skip - lmweb checkout not found next to this repo; schema identity not checked"
fi
(cd "$HERE" && HOME="$WORK/home" "$LMSCRIPT" dsl/build_router.lms "$WORK/router.dslb" >"$WORK/compile.out" 2>&1) || { cat "$WORK/compile.out"; fail "lmscript compiles dsl/build_router.lms"; }
[ "$(head -c 4 "$WORK/router.dslb")" = "DSLB" ] || fail "magic"
[ "$(od -An -tu1 -j4 -N2 "$WORK/router.dslb" | xargs)" = "0 5" ] || fail "bytecode version 5 (use a v5 compiler)"
ok "lmscript $HAVE_LMSCRIPT compiles the router to $(wc -c <"$WORK/router.dslb") bytes (bytecode v5)"

# 3. Serve it on the Rust runtime.
start_server() { # $1 binary; sets PORT and SERVER_PID
  # A host on the flag standard embeds its schema; an older C++ server still takes --schema FILE.
  set -- "$1" --port 0 --dsl "${2:-$WORK/router.dslb}"
  "$1" --dsl-schema --json >/dev/null 2>&1 || set -- "$@" --schema "$SCHEMA"
  "$@" >"$WORK/s.out" 2>"$WORK/s.err" & SERVER_PID=$!; PIDS="$PIDS $SERVER_PID"
  PORT=""; for _ in $(seq 1 50); do PORT="$(sed -n 's/^listening on //p' "$WORK/s.out")"; [ -n "$PORT" ] && break; sleep 0.1; done
  [ -n "$PORT" ] || { cat "$WORK/s.err"; fail "$1 did not print its port"; }
}
stop_server() { kill -TERM "$SERVER_PID" 2>/dev/null; wait "$SERVER_PID" 2>/dev/null; }
start_server "$SERVER"
RESPONSE="$(curl -s -i "http://127.0.0.1:$PORT/")"; echo "$RESPONSE" | sed 's/^/    /'
echo "$RESPONSE" | head -1 | grep -q '^HTTP/1.1 200 OK' || fail "GET status: $RESPONSE"
[ "$(echo "$RESPONSE" | tail -1 | tr -d '\r')" = "$TODAY" ] || fail "GET body is today's date ($TODAY): $RESPONSE"
ok "Rust runtime: GET / -> 200 with body $TODAY"
POST_CODE="$(curl -s -o "$WORK/post.body" -w '%{http_code}' -X POST -d 'x=1' "http://127.0.0.1:$PORT/")"
[ "$POST_CODE" = 405 ] && [ "$(cat "$WORK/post.body")" = "method not allowed" ] || fail "POST: $POST_CODE $(cat "$WORK/post.body")"
ok "Rust runtime: POST / -> 405 method not allowed"
stop_server; grep -q '^stopped$' "$WORK/s.err" || fail "clean stop: $(cat "$WORK/s.err")"
ok "Rust router stops cleanly on SIGTERM"

# 3b. _dsl_ cli="PROGRAM" (lmscript R42) compiles against the binary's own embedded schema; older compilers reject it.
printf 'p = _dsl_ cli="%s";\nres.status(200);\nres.body("cli");\nend _dsl_\nsavedsl(p, "%s");\n' "$SERVER" "$WORK/cli.dslb" >"$WORK/cli.lms"
if HOME="$WORK/home" "$LMSCRIPT" --check "$WORK/cli.lms" >/dev/null 2>&1; then
  HOME="$WORK/home" "$LMSCRIPT" "$WORK/cli.lms" >/dev/null 2>"$WORK/cli.err" || { cat "$WORK/cli.err"; fail "compiling with cli=$SERVER"; }
  start_server "$SERVER" "$WORK/cli.dslb"
  [ "$(curl -s "http://127.0.0.1:$PORT/")" = cli ] || fail "running a cli= compiled program"
  stop_server
  ok "_dsl_ cli=\"router\" compiles against the binary's own schema and serves"
else
  echo "skip - _dsl_ cli= (this lmscript predates lmscript R42)"
fi

# 4. The SAME bytecode file on the C++ runtime (lmweb), when it is built next to this repo.
CPP="${LMWEB_DIR:+$LMWEB_DIR/build/server}"
if [ -x "$CPP" ]; then
  start_server "$CPP"
  [ "$(curl -s "http://127.0.0.1:$PORT/")" = "$TODAY" ] || fail "C++ runtime GET body"
  [ "$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$PORT/")" = 405 ] || fail "C++ runtime POST"
  stop_server
  ok "the unchanged bytecode gives the same answers on the C++ runtime (lmweb/build/server)"
else
  echo "skip - lmweb/build/server not built next to this repo"
fi

# 5. Refusals before serving: schema mismatch, truncated file, wrong magic (exit 1 with a message).
# The mismatch: the same chunk compiled against a schema whose bytes differ from the embedded one.
mkdir -p "$WORK/other/dsl" "$WORK/other/schema"; cp "$HERE/dsl/build_router.lms" "$WORK/other/dsl/"
sed 's/"name": "router"/"name": "router2"/' "$SCHEMA" >"$WORK/other/schema/router.dsl.json"
cmp -s "$SCHEMA" "$WORK/other/schema/router.dsl.json" && fail "the mismatch schema must differ"
(cd "$WORK/other" && HOME="$WORK/home" "$LMSCRIPT" dsl/build_router.lms "$WORK/other.dslb" >"$WORK/other.out" 2>&1) || { cat "$WORK/other.out"; fail "compile against the other schema"; }
out="$("$SERVER" --port 0 --dsl "$WORK/other.dslb" 2>&1)"; code=$?; [ "$code" = 1 ] && case "$out" in *"schema mismatch"*) ;; *) fail "mismatch: $code $out";; esac
ok "refuses a schema-hash mismatch before serving"
head -c 60 "$WORK/router.dslb" >"$WORK/trunc.dslb"; out="$("$SERVER" --port 0 --dsl "$WORK/trunc.dslb" 2>&1)"; case "$out" in *"bytecode is truncated"*) ;; *) fail "truncated: $out";; esac
ok "refuses a truncated file"
{ printf 'XXXX'; tail -c +5 "$WORK/router.dslb"; } >"$WORK/magic.dslb"; out="$("$SERVER" --port 0 --dsl "$WORK/magic.dslb" 2>&1)"; case "$out" in *"bad magic"*) ;; *) fail "magic: $out";; esac
ok "refuses a wrong magic"
echo "PASS $pass checks"
