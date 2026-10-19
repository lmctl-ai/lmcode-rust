#!/usr/bin/env bash
# Pins a copy of cpp-dsl-sdk's conformance vectors (THE contract between runtimes) in conformance/vectors and records the SDK
# commit plus a sha256 over every vector file in vectors.lock. `--verify` checks the copy against the lock (cargo test runs it),
# so an edited or stale copy fails the tests. Source: the sibling checkout ../cpp-dsl-sdk (override with CPP_DSL_SDK=path).
set -eu
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDK="${CPP_DSL_SDK:-$HERE/../cpp-dsl-sdk}"; DST="$HERE/conformance/vectors"; LOCK="$HERE/vectors.lock"
digest() { (cd "$1" && find . -type f | LC_ALL=C sort | xargs sha256sum | sha256sum | cut -c1-64); }
if [ "${1:-}" = "--verify" ]; then
  [ -f "$LOCK" ] || { echo "vectors.lock missing" >&2; exit 1; }
  want="$(sed -n 's/^sha256=//p' "$LOCK")"; have="$(digest "$DST")"
  [ "$want" = "$have" ] || { echo "conformance/vectors digest $have differs from vectors.lock $want: run scripts/sync-vectors.sh (or restore the pinned copy)" >&2; exit 1; }
  echo "conformance/vectors matches vectors.lock ($(sed -n 's/^commit=//p' "$LOCK"))"; exit 0
fi
[ -d "$SDK/conformance/vectors" ] || { echo "cpp-dsl-sdk checkout not found at $SDK (set CPP_DSL_SDK)" >&2; exit 1; }
rm -rf "$DST"; mkdir -p "$DST"; cp "$SDK"/conformance/vectors/* "$DST"/
printf 'repo=https://github.com/lmctl-ai/cpp-dsl-sdk\ncommit=%s\nsha256=%s\n' "$(git -C "$SDK" rev-parse HEAD)" "$(digest "$DST")" >"$LOCK"
echo "synced $(ls "$DST" | wc -l) files from cpp-dsl-sdk $(git -C "$SDK" rev-parse --short HEAD)"
