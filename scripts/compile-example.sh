#!/usr/bin/env bash
# Fixture-only bytecode generation; the language never exports program files.
set -euo pipefail
node scripts/compile-fixture.mjs examples/rule-entries/rules.lms target/rule-entries.dslb
