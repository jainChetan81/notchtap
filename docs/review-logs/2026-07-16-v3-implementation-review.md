# review log — v3 implementation (two-axis code review) — 2026-07-16

**setup**: mattpocock two-axis review (standards + spec), two parallel
general-purpose sub-agents. fixed point `HEAD` = `22bfc3d`; subject =
the full uncommitted working tree (v3 telegram connector + the
pre-existing uncommitted doc/prototype edits), plus the untracked
`src-tauri/src/notifier.rs` and `docs/V3_TECHNICAL_SPEC.md` read
directly (untracked files don't appear in `git diff HEAD`).

**prior attempts, recorded for honesty**: a kimi-code delegation review
died on a provider quota 403 before producing findings; a pal
`consensus` two-reviewer round timed out twice at the 300s MCP idle
limit (pal itself healthy) and was halted per that skill's fail-loud
rule. neither produced a verdict; this two-axis run is the review of
record. reviewed by: two claude general-purpose sub-agents (standards
axis, spec axis) — no external models.

## standards axis — 4 hard + 4 judgement calls

hard (all doc-consistency, none in the v3 code itself):
1. `CLAUDE.md` said "v3 planned but not started" while the same tree
   implemented v3 — **fixed** (project-state paragraph rewritten).
2. test counts duplicated in `CLAUDE.md` against `TESTING_STRATEGY.md`
   §0's counts-live-here-only rule, and stale — **fixed** (CLAUDE.md
   now points at §0; §0 table updated to the true counts).
3. `TESTING_STRATEGY.md` §0/§8 still said "twilio/whatsapp lands with
   v3" after §4.9 locked telegram-first — **fixed** (both rows).
4. glossary drift: `CONTEXT.md` locks **Connector**, module is
   `notifier.rs`, docs mention a `Notifier` trait that doesn't exist in
   code (`ConnectorHandle` does) — **left open**, naming decision for
   the user (rename module vs add term to glossary).

judgement calls (recorded, not acted on): `FailureKind` as a plain enum
vs the thiserror rule (classification, not an error type — accepted);
data-clump in `telegram_connector`'s three params; duplicated test
event-builders across three files; `SEND_TIMEOUT` const while
`retry_delay` is injectable (wiremock tests build their own client).

## spec axis — 2 missing, 3 scope-creep, 2 questionable

§4.9 test crosswalk: complete (sole gap covered below).

1. **poller fan-out gap (the finding of the review)**: plan §3 says
   "every accepted event goes to every enabled connector, always," but
   the espn poller enqueued directly and never called `offer` — score
   events could never reach telegram despite the spec defining their
   ⚽/🕐 templates. the spec §6 "defer until a second call site exists"
   line didn't cover it: the poller already was that call site.
   **fixed**: `poller::enqueue_and_fan_out` (testable helper, the
   poller-side twin of http.rs's seam), connectors threaded through
   `spawn_espn_poller`, regression test
   `poller_accepted_events_fan_out_and_rejected_do_not`. spec §6
   updated to record the resolution.
2. **`RetryAfter(Duration)` carried value discarded** — the worker
   slept `cfg.retry_delay` while the pure fn returned a const; the
   unit-tested value wasn't the one used. **fixed**: `on_send_failure`
   now takes `retry_after: Duration` and the worker sleeps exactly the
   carried value; spec §2 signature updated; test asserts the carried
   value round-trips.
3. unknown-type template row (spec §3) unreachable in code — closed
   enum rejects unknowns at deserialization. **left as-is**, moot in
   practice; noted here rather than adding a dead match arm.
4. scope-creep notes (all from the pre-existing uncommitted set, not
   the v3 work): the CLAUDE.md "not started" line (fixed above),
   doc-tests/`pub mod` changes (v4's line), `prototype-notch-morph.html`
   + plan §3.5 — bundled knowingly into the same commit on the user's
   merge instruction.
5. `mode != 0o600` rejects stricter perms (0400) — literal-compliant
   with spec §4, noted only.

## action taken

both real code defects fixed with regression tests; three doc-staleness
items fixed; glossary naming left open. all gates re-run green after
fixes: cargo test 88/88 + 4 doc-tests, clippy `-D warnings` clean,
fmt clean, tsc + vite build clean, vitest 11/11. committed on the
user's explicit instruction ("let's merge it"); no push without a
separate say-so.
