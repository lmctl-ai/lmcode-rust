#!/usr/bin/env bash
# End-to-end test of the Rust router host. Needs cargo (1.92 used), curl and the released lmscript CLI
# (`npm install -g @lmctl-ai/lmctl`, >= 0.2.328; LMSCRIPT_BIN overrides). Proves that the UNCHANGED bytecode compiled from
# poc-web's router script runs on this Rust runtime and, when ../../../poc-web/build/server exists, on the C++ runtime too.
set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; ROOT="$(cd "$HERE/../.." && pwd)"
LMSCRIPT="${LMSCRIPT_BIN:-lmscript}"; MIN_LMSCRIPT=0.2.328
if ! command -v "$LMSCRIPT" >/dev/null 2>&1 && [ ! -f "$LMSCRIPT" ]; then echo "FAIL: lmscript not found ($LMSCRIPT). Install the released CLI: npm install -g @lmctl-ai/lmctl (>= $MIN_LMSCRIPT), or set LMSCRIPT_BIN." >&2; exit 1; fi
HAVE_LMSCRIPT="$("$LMSCRIPT" --version 2>/dev/null | sed -n 's/^lmscript //p')"
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

# 2. Compile poc-web's router chunk with the released lmscript (same schema bytes as poc-web).
cmp -s "$SCHEMA" "$ROOT/../poc-web/schema/router.dsl.json" 2>/dev/null && ok "schema is byte-identical to poc-web's schema/router.dsl.json" || echo "skip - poc-web checkout not found next to this repo; schema identity not checked"
(cd "$HERE" && HOME="$WORK/home" "$LMSCRIPT" dsl/build_router.lms "$WORK/router.dslb" >"$WORK/compile.out" 2>&1) || { cat "$WORK/compile.out"; fail "lmscript compiles dsl/build_router.lms"; }
[ "$(head -c 4 "$WORK/router.dslb")" = "DSLB" ] || fail "magic"
ok "lmscript $HAVE_LMSCRIPT compiles the router to $(wc -c <"$WORK/router.dslb") bytes (bytecode v3)"

# 3. Serve it on the Rust runtime.
start_server() { # $1 binary; sets PORT and SERVER_PID
  "$1" --port 0 --dsl "$WORK/router.dslb" --schema "$SCHEMA" >"$WORK/s.out" 2>"$WORK/s.err" & SERVER_PID=$!; PIDS="$PIDS $SERVER_PID"
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

# 4. The SAME bytecode file on the C++ runtime (poc-web), when it is built next to this repo.
CPP="$ROOT/../poc-web/build/server"
if [ -x "$CPP" ]; then
  start_server "$CPP"
  [ "$(curl -s "http://127.0.0.1:$PORT/")" = "$TODAY" ] || fail "C++ runtime GET body"
  [ "$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$PORT/")" = 405 ] || fail "C++ runtime POST"
  stop_server
  ok "the unchanged bytecode gives the same answers on the C++ runtime (poc-web/build/server)"
else
  echo "skip - poc-web/build/server not built next to this repo"
fi

# 5. Refusals before serving: schema mismatch, truncated file, wrong magic (exit 1 with a message).
sed 's/"name": "router"/"name": "router2"/' "$SCHEMA" >"$WORK/other.json"
out="$("$SERVER" --port 0 --dsl "$WORK/router.dslb" --schema "$WORK/other.json" 2>&1)"; code=$?; [ "$code" = 1 ] && case "$out" in *"schema mismatch"*) ;; *) fail "mismatch: $code $out";; esac
ok "refuses a schema-hash mismatch before serving"
head -c 60 "$WORK/router.dslb" >"$WORK/trunc.dslb"; out="$("$SERVER" --port 0 --dsl "$WORK/trunc.dslb" --schema "$SCHEMA" 2>&1)"; case "$out" in *"bytecode is truncated"*) ;; *) fail "truncated: $out";; esac
ok "refuses a truncated file"
{ printf 'XXXX'; tail -c +5 "$WORK/router.dslb"; } >"$WORK/magic.dslb"; out="$("$SERVER" --port 0 --dsl "$WORK/magic.dslb" --schema "$SCHEMA" 2>&1)"; case "$out" in *"bad magic"*) ;; *) fail "magic: $out";; esac
ok "refuses a wrong magic"
echo "PASS $pass checks"
