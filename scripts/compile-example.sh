#!/usr/bin/env bash
set -euo pipefail
"${LMSCRIPT_BIN:-lmscript}" --no-preflight examples/rule-entries/rules.lms target/rule-entries.dslb
