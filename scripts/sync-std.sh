#!/usr/bin/env bash
set -eu
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDK="${LMCODE_CPP:-$HERE/../lmcode-cpp}"
SRC="$SDK/std/lmcode-std.schema.json"
DST="$HERE/std/lmcode-std.schema.json"
LOCK="$HERE/std.lock"
[ -f "$SRC" ] || { echo "canonical std schema not found at $SRC (set LMCODE_CPP)" >&2; exit 1; }
if [ "${1:-}" = "--verify" ]; then
  want="$(sed -n 's/^sha256=//p' "$LOCK")"
  have="$(sha256sum "$DST" | cut -d' ' -f1)"
  [ "$want" = "$have" ] || { echo "std schema digest $have differs from std.lock $want" >&2; exit 1; }
  cmp -s "$SRC" "$DST" || { echo "std schema differs from canonical C++ copy" >&2; exit 1; }
  echo "std schema matches $(sed -n 's/^commit=//p' "$LOCK")"
  exit 0
fi
cp "$SRC" "$DST"
printf 'repo=https://github.com/lmctl-ai/lmcode-cpp\ncommit=%s\nsha256=%s\n' "$(git -C "$SDK" rev-parse HEAD)" "$(sha256sum "$DST" | cut -d' ' -f1)" > "$LOCK"
echo "synced canonical std schema from lmcode-cpp $(git -C "$SDK" rev-parse --short HEAD)"
