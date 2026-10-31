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
  DOC_LOCK="$HERE/std-docs.lock"
  DOC="$HERE/std/lmcode-std.docs.json"
  doc_want="$(sed -n 's/^sha256=//p' "$DOC_LOCK")"
  doc_have="$(sha256sum "$DOC" | cut -d' ' -f1)"
  [ "$doc_want" = "$doc_have" ] || { echo "std docs digest $doc_have differs from std-docs.lock $doc_want" >&2; exit 1; }
  echo "std schema matches $(sed -n 's/^commit=//p' "$LOCK"); std docs match $(sed -n 's/^commit=//p' "$DOC_LOCK")"
  exit 0
fi
commit="$(git -C "$SDK" log -1 --format=%H -- std/lmcode-std.schema.json)"
[ -n "$commit" ] || { echo "canonical std schema has no committed revision" >&2; exit 1; }
doc_commit="$(git -C "$SDK" log -1 --format=%H -- std/lmcode-std.docs.json)"
[ -n "$doc_commit" ] || { echo "canonical std docs have no committed revision" >&2; exit 1; }
mkdir -p "$HERE/std"
git -C "$SDK" show "$commit:std/lmcode-std.schema.json" > "$DST"
git -C "$SDK" show "$doc_commit:std/lmcode-std.docs.json" > "$HERE/std/lmcode-std.docs.json"
printf 'repo=https://github.com/lmctl-ai/lmcode-cpp\ncommit=%s\nsha256=%s\n' "$commit" "$(sha256sum "$DST" | cut -d' ' -f1)" > "$LOCK"
printf 'repo=https://github.com/lmctl-ai/lmcode-cpp\ncommit=%s\nsha256=%s\n' "$doc_commit" "$(sha256sum "$HERE/std/lmcode-std.docs.json" | cut -d' ' -f1)" > "$HERE/std-docs.lock"
echo "synced canonical std schema/docs from lmcode-cpp $commit/$doc_commit"
