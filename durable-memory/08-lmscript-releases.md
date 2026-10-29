# Tests and releases

Root test.lms drives cargo workspace tests, release builds, both vector catalogs and the annotated Score example. Install current npm @lmctl-ai/lmctl and Rust stable. CI runs on push main, PR and manual dispatch across five platforms; successful main builds publish router, dsl-conformance and rule-entries binaries with licenses/checksums. test.sh forwards to lmscript. The optional router HTTP integration script remains available separately.
