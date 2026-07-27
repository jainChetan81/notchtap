# 140 — Kimi adapter (version-gated)

> v7 ticket 8 of 13 (spec §4.4). Filed 2026-07-26.

**What to build:** `notchtap-agent hook kimi` for hook-supporting Kimi
versions, and an honest `unavailable` state below them.

- Pure Kimi parser + redacted fixtures for the Kimi hook events
  equivalent to the §4.2 set, per the Kimi Code hooks docs.
- Version gate: install/report only when the local Kimi version
  advertises hook support; otherwise the adapter reports
  `unavailable` with the minimum supported version. NO terminal
  scraping fallback, ever.
- Detected compatibility state + setup snippet surface through
  Adapter Health (consumed by ticket 143's Settings UI; until then
  visible via `notchtap-agent status`).
- Same sanitization, fail-open, and delivery rules as ticket 138.

**Blocked by:** 138 (shared helper).

**Status:** ready-for-agent

- [ ] Fixture test per supported native event; capability declaration
      and fixtures agree.
- [ ] Version-gate tests: below-minimum → `unavailable` + minimum
      version reported; supported → available.
- [ ] Manual: real Kimi session smoke on a hook-supporting version.
- [ ] `cargo test` green.
