#!/usr/bin/env bash
# Pins a copy of lmcode-cpp's conformance vectors (THE contract between runtimes) in conformance/vectors and records the SDK
# commit plus a sha256 over every vector file in vectors.lock. `--verify` checks the copy against the lock (cargo test runs it),
# so an edited or stale copy fails the tests. Source: the sibling checkout ../lmcode-cpp (override with LMCODE_CPP=path).
set -eu
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDK="${LMCODE_CPP:-$HERE/../lmcode-cpp}"
digest() { (cd "$1" && find . -type f | LC_ALL=C sort | xargs sha256sum | sha256sum | cut -c1-64); }
if [ "${1:-}" = "--verify" ]; then
  for suite in vectors stdlib-vectors; do
    dst="$HERE/conformance/$suite"; lock="$HERE/$suite.lock"; label="conformance/$suite"
    [ -f "$lock" ] || { echo "$label lock missing" >&2; exit 1; }
    [ -d "$dst" ] && [ "$(find "$dst" -type f | wc -l)" -gt 0 ] || { echo "$label is empty" >&2; exit 1; }
    want="$(sed -n 's/^sha256=//p' "$lock")"; have="$(digest "$dst")"
    [ "$want" = "$have" ] || { echo "$label digest $have differs from $lock: run scripts/sync-vectors.sh" >&2; exit 1; }
    echo "$label matches $(sed -n 's/^commit=//p' "$lock")"
  done
  exit 0
fi
[ -d "$SDK/conformance/vectors" ] && [ -d "$SDK/conformance/stdlib-vectors" ] || { echo "lmcode-cpp vector directories not found at $SDK (set LMCODE_CPP)" >&2; exit 1; }
for suite in vectors stdlib-vectors; do
  src="$SDK/conformance/$suite"; dst="$HERE/conformance/$suite"; lock="$HERE/$suite.lock"
  rm -rf "$dst"; mkdir -p "$dst"; cp "$src"/* "$dst"/
  printf 'repo=https://github.com/lmctl-ai/lmcode-cpp\ncommit=%s\nsha256=%s\n' "$(git -C "$SDK" rev-parse HEAD)" "$(digest "$dst")" >"$lock"
  echo "synced $(find "$dst" -type f | wc -l) files from lmcode-cpp $(git -C "$SDK" rev-parse --short HEAD)"
done
