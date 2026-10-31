# Result sink

ResultSink owns copied JSON values independently of the Context arena. The 24
standard return imports are unchanged. R3 adds host-owned ResultContract validation
and sealing; use the [invocation collector](10-invocation-results.md) to bind one
sink per program and write a single envelope after accepted work drains.

set_result/put_result validate before replacement. Cloned bindings share a slot,
not a fresh invocation: after completion they reject writes and reset. New runs
need new collectors. Sink-only framing and the old report helpers are removed.
