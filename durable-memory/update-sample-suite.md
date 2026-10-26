# Sample-suite update rule

The canonical Shop/Ledger sample suite is maintained in the sibling
`lmcode-cpp/sample-suite/`. Changes to the language, VM, standard catalog,
schema binding, bytecode format, host protocol, or sample contract must update
that suite first and regenerate schema/bytecode/expected fixtures. Run
`sample-suite/run-all-samples.sh --all` with every configured SDK; missing
ports, failed builds, protocol failures, and case failures are errors, never
skips. Use the reviewed companion checkout paths or environment-file overrides.
