#!/usr/bin/env bash
# Pins a copy of the shared REST conformance suite (lmcode/tests/rest-suite; the opt-in `rest` std module) in
# conformance/rest-suite and records the spec commit plus a portable sha256 in rest-suite.lock.
#   scripts/sync-rest.sh            copy ../lmcode/tests/rest-suite (or $LMCODE_SPEC_DIR) and write the lock
#   scripts/sync-rest.sh --verify   check the copy against the lock (CI)
# Digest: sha256 over rows "<sha256(file)>  <relative path>\n", paths sorted bytewise.
set -eu
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SPEC="${LMCODE_SPEC_DIR:-$HERE/../lmcode}"
DST="$HERE/conformance/rest-suite"; LOCK="$HERE/rest-suite.lock"
digest() { (cd "$1" && find . -type f | sed 's|^\./||' | LC_ALL=C sort | while IFS= read -r f; do printf '%s  %s\n' "$(sha256sum < "$f" | cut -d' ' -f1)" "$f"; done) | sha256sum | cut -c1-64; }
if [ "${1:-}" = "--verify" ]; then
  [ -f "$LOCK" ] || { echo "conformance/rest-suite lock missing" >&2; exit 1; }
  [ -d "$DST" ] && [ "$(find "$DST" -type f | wc -l)" -gt 0 ] || { echo "conformance/rest-suite is empty" >&2; exit 1; }
  want="$(sed -n 's/^sha256=//p' "$LOCK" | tr -d '\r')"; have="$(digest "$DST")"
  [ "$want" = "$have" ] || { echo "conformance/rest-suite digest $have differs from $want (rest-suite.lock): run scripts/sync-rest.sh" >&2; exit 1; }
  echo "conformance/rest-suite matches $(sed -n 's/^commit=//p' "$LOCK" | tr -d '\r')"
  exit 0
fi
[ -d "$SPEC/tests/rest-suite" ] || { echo "spec checkout not found at $SPEC (set LMCODE_SPEC_DIR)" >&2; exit 1; }
rm -rf "$DST"; mkdir -p "$DST"; cp -R "$SPEC/tests/rest-suite/." "$DST/"; chmod -R u+rw "$DST"
printf 'repo=https://github.com/lmctl-ai/lmcode\ncommit=%s\nsha256=%s\n' "$(git -C "$SPEC" rev-parse HEAD)" "$(digest "$DST")" >"$LOCK"
echo "synced $(find "$DST" -type f | wc -l) files from lmcode $(git -C "$SPEC" rev-parse --short HEAD)"
