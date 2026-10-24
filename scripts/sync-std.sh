#!/usr/bin/env bash
set -eu
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDK="${LMCODE_CPP:-$HERE/../lmcode-cpp}"
SRC="$SDK/std/lmcode-std.schema.json"
DST="$HERE/std/lmcode-std.schema.json"
LOCK="$HERE/std.lock"
if [ "${1:-}" = "--verify" ]; then
  want="$(sed -n 's/^sha256=//p' "$LOCK")"
  have="$(sha256sum "$DST" | cut -d' ' -f1)"
  [ "$want" = "$have" ] || { echo "std schema digest $have differs from std.lock $want" >&2; exit 1; }
  echo "std schema matches $(sed -n 's/^commit=//p' "$LOCK")"
  exit 0
fi
commit="$(git -C "$SDK" log -1 --format=%H -- std/lmcode-std.schema.json)"
[ -n "$commit" ] || { echo "canonical std schema has no committed revision" >&2; exit 1; }
mkdir -p "$HERE/std"
git -C "$SDK" show "$commit:std/lmcode-std.schema.json" > "$DST"
printf 'repo=https://github.com/lmctl-ai/lmcode-cpp\ncommit=%s\nsha256=%s\n' "$commit" "$(sha256sum "$DST" | cut -d' ' -f1)" > "$LOCK"
echo "synced canonical std schema from lmcode-cpp $commit"
