# Test-trim audit — notchtap (2026-08-17)

Target: 30-45% fewer tests, zero lost behavior classes. Keeps all behavior contracts and every parity/safety-net test.

## Keep — untouchable

- **Queue / Engine:** promotion, preemption, rotation (one-shot vs recurring), Topic supersession, per-tier caps (429), pause (buffer + resume), kill switch `start_paused`, silence (Silent Period, Timed Mute, Breakthrough), `dedup_eq` for ttl/remaining_ms, `toggle_expanded`
- **HTTP:** 400 malformed, 429 tier full, 413 body too large, 500 unexpected, Host-header, loopback-only, Idempotency for agent events (eventId/sequence)
- **Config:** healing for appearance, silence, rotation_order, prefix, appearance validation, port preflight
- **Parity tests:** `stripGeometryParity`, `tabWireParity`, `overlayCardMirror`, `animationTiming`, `hookEventParity`, `entryImportOrder`
- **Security:** overlay receive-only, settings allowlist, click monitor, adapter caps/sanitization, notchtap-detect subprocess

## Cut candidates (examples, ~80 tests)

- `src/components/AgentBoard.test.tsx`: 3 tests re-prove same hero fact tagging with only label case change (`Tool` vs `tool`) — collapse to 1.
- `src/components/StatusRailCard.test.tsx`: 4 tests assert `className` contains `high` vs `medium` vs `low` with same fixture — collapse to parametrized 1.
- `src/useAgentState.test.ts`: 5 tests for `isValidAgentState` with only one field type flipped (`elapsedMs` string vs number) — collapse to 1 parametrized.
- `src-tauri/src/queue.rs`: 12 tests for `max_queued_per_tier` with only `Low` vs `Medium` tier name changed — collapse to 2.
- `src-tauri/src/silence.rs`: 6 tests for Silent Period boundaries with only minute offset changed — collapse to 2.
- `src/settings/SettingsApp.test.tsx`: 8 tests for `AppearanceSection` preview cards that only assert `.compact` vs `.live` chip — collapse to 2.
- `src/lib/presentation.test.ts`: 3 tests for `categoryClass` with only color token changed — collapse to 1.

## Execution order

1. Collapse web tests first (lowest risk, vitest only), one file per commit, `npx vitest run` green after each.
2. Collapse Rust tests (cargo test), one module per commit.
3. After each commit, run `just test-all` (cargo test + vitest + tsc + biome + oxlint).

## Not in this PR

Full 30-45% collapse is ~225-337 tests. This audit file is the gate; the actual collapse runs after this PR lands, file-by-file, to keep reviewers sane. This PR leaves the suite at 748 web + 953 Rust, all green, with the cut list recorded.
