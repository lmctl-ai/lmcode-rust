#!/usr/bin/env bash
# Fixture-only bytecode generation; the language never exports program files.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
node scripts/compile-fixture.mjs examples/rule-entries/rules.lms target/rule-entries.dslb
node scripts/compile-fixture.mjs dslvm/tests/fixtures/rule-methods/program.lms target/rule-methods.dslb
cmp target/rule-methods.dslb dslvm/tests/fixtures/rule-methods/program.dslb
