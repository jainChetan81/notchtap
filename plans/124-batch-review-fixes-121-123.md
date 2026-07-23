# Plan 124: deep-review fix batch for plans 121–123

> Filed 2026-07-23 from the four-lens deep review of `0c456b2..b312e23`
> (rust correctness, security, frontend/choreography, test quality —
> full findings in the review report; provenance in plans/README.md row
> 124). Drift check: authored against master `341a0f3`. Two
> file-disjoint parts, parallel-executable. Every finding below was
> verified against live code by a reviewer AND re-verified by the
> review coordinator before filing.

## Part F (frontend executor) — files: `src/settings/sections/QueueSection.tsx`, `src/useExitChoreography.ts`, `src/components/StatusRailCard.tsx`, `src/components/StatusRailCard.test.tsx`, `src/settings/SettingsApp.test.tsx`, `docs/TESTING_STRATEGY.md` (§0 frontend line only)

**F1 (Medium) — QueueSection: build the missing manual Refresh control.**
Plan 121 specced "fetch-on-open + manual Refresh (Diagnostics-section
precedent)"; the plumbing exists (`refresh(announce)`, comments at
`QueueSection.tsx:26-28` describe the button) but no button renders.
Add the Refresh control row following `DiagnosticsSection.tsx:59-70`
exactly, `onClick={() => refresh(true)}` (announced — plan-108
user-initiated rule; the mount fetch stays `refresh(false)`).

**F2 (Medium) — exit-to-bare must not fight an active hover.**
`useExitChoreography.ts:133`: `exitToBare = shellExiting &&
restingState === "notch"` → add `&& !hovered`. Mechanism (verified):
with the pointer on the card at settle, the next render is
`bare && hovered` → `.bare:has(.idle-peek)` re-widens `--cw`
(`overlay-card.css:165-167`) and `.bare.hovered` repaints flanks — a
175ms shrink then 320ms rebound wobble. With `!hovered`, a hovered
exit falls back to the plain `.exiting` (wide) rule, which was
seamless pre-123; a mid-window hover flip retargets continuously (CSS
transitions retarget from current computed values). Extend the hook's
doc comment (keep its voice) explaining the `!hovered` term. Update
BOTH plan-123 tests' assumptions if needed and ADD: a notch-mode exit
with `hovered` true never applies `exit-to-bare`; a hover arriving
mid-window drops the class.

**F3 (Low) — rail chrome must fade with the flank during exit-to-bare.**
During the window the flank paint animates to transparent while
FlankClock/StatusDots stay mounted fully opaque
(`StatusRailCard.tsx:418, :472` — `railRevealed &&`), so white text
sits on a see-through flank mid-window; their AnimatePresence exit
fires only after they're clipped. Gate both mounts on
`railRevealed && !exitToBare` so the existing 260ms exit fade overlaps
the window. Add a test: during a notch-mode unhovered exit the clock
and dots unmount at t=0 of the window (and rail mode is unaffected).

**F4 — string-pin the plan-123 CSS convergence invariant (test only).**
The five-property invariant (`overlay-card.css` exit-to-bare block vs
`.bare` rules) is hand-duplicated literals with no enforcement. Using
the `ruleBody`-style helper pattern (see `IdleHoverPeek.test.tsx`'s
helper added in plan 122, and `celebrationStacking.test.tsx`
precedent), assert byte-equality: exit rule's `--cw` == `.bare`'s;
flank end-state background/padding values == `.bare`'s; exit rule's
two `border-bottom-*-radius` values == the `.bare` synthetic-cutout
rule's. Helper must throw on a missing selector (no vacuous pass).

**F5 — QueueSection test pins.** (a) The "priority tag" test
(`SettingsApp.test.tsx:1233`) must assert tag CONTENT:
`["High", "Low"]` from the existing mixed fixtures. (b) Add the
literal-text/injection pin the section's own comment cites: a mocked
title `<img src=x onerror=x>`-style must render as literal text with
`querySelector("script,img") === null` in the row (History precedent
`SettingsApp.test.tsx:1110`), plus a 300-char unbroken-token title
asserting the `.queue-title` class carries the overflow-wrap utility.

**F6 — queue load-failure state.** On a failed mount fetch `items`
stays `null` → "Loading…" renders forever under the sticky error.
Render an error-aware state instead (e.g. "Couldn't load the queue —
Refresh to retry") when the load errored. Small; follow the
Diagnostics feel. Test: mock a rejected `get_queue`, assert no
"Loading…" text remains.

## Part R (rust executor) — files: `src-tauri/src/queue.rs`, `src-tauri/src/status.rs`, `src-tauri/src/settings_commands.rs`, `src-tauri/src/engine.rs` (comment only), `docs/TESTING_STRATEGY.md` (§0 rust line only)

**R1 (Low, pre-existing) — skip of a re-promoted same-id item must
re-anchor the wire.** Scenario (verified): lone Recurring visible,
auto-expanded first half-window; `skip_visible` requeues + re-promotes
the same item with fresh `promoted_at`, but the new `SlotState::
Showing` is `dedup_eq`-equal (only `remaining_ms` differs, deliberately
excluded) → no emit; the overlay TTL bar drains and sits stale up to
~half a window. Fix in `queue.rs`: when `skip_visible`'s re-promotion
lands an item whose id equals the last-emitted Showing id, force the
next `slot_state_if_changed` to emit (e.g. clear `last_emitted`) — the
fix must live at the skip/re-promote site, NOT weaken `dedup_eq`
(CLAUDE.md rule: `remaining_ms` stays excluded). ALSO handle the
ttl-restart detector: `observe_emission_for_ttl_restart`
(queue.rs:823-885) would log its warning for this intentional restart
— reset/feed its sample at the same site so a skip never trips the
warning (STOP if that can't be done without weakening the detector's
real cases; its tests must stay green unmodified). Tests: skip of a
lone Recurring produces exactly one fresh emit with restarted
`remaining_ms`; no ttl-restart warning logged for it; a genuine
restart still warns.

**R2 — pin `rain_pct: None`'s wire shape.** Extend the camelCase
serialization test (`status.rs` ~:322, plan-104 "`current` serializes
as `null`" precedent): a `None` rain_pct serializes as `"rainPct":
null` with the KEY PRESENT (`.get("rainPct").is_some()`); this is the
guard against a future `skip_serializing_if` blanking the whole
overlay StatusState (the TS validator rejects a missing key).

**R3 — pin all five `source_kind_label` spellings** (one assert per
variant; today only "manual" is pinned and `types.ts` claims the
five-string union as a typed wire contract).

**R4 — composition tests.** (a) `clear_waiting` then fresh `enqueue`:
`queue_progress` reads `(done + 2, done)` (append to the existing
invariant test). (b) skip of a Recurring visible leaves it present,
last in its tier, in `waiting_summaries` (the settings window's
user-visible contract; the UI mock currently encodes the OneShot
intuition only).

**R5 — hardening one-liners in `settings_commands.rs` parity tests.**
(a) Guard #1: assert the FULL permission set of settings.json — every
entry is either `allow-<one of SETTINGS_COMMANDS>` or in the pinned
extras list (`core:event:allow-listen`, `core:event:allow-unlisten`);
a namespaced plugin grant (e.g. `shell:allow-execute`) must now FAIL.
(b) Guard #2: assert `lib.rs` contains exactly one
`tauri::generate_handler![` occurrence.

**R6 — fix the stale comment** on
`clear_queue_apply_emits_nothing_when_nothing_was_ever_visible`
(engine.rs ~:1180: says "second enqueue" — the test performs one).

## Shared rules

- Do NOT touch the other part's files; §0: Part F edits only the
  frontend count line, Part R only the rust line.
- No behavior changes beyond those specified. `capabilities/*` and
  `build.rs` untouched. No new duration literals (F2/F3 reuse existing
  vars/mount gates).
- Verification ladder per part: Part F — tsc, vitest, biome ci, vite
  build. Part R — cargo test --locked, clippy --all-targets -D
  warnings, fmt --check.
- STOP conditions: R1's detector interaction (above); F2/F3 breaking
  any existing rail-mode or plan-123 test in a way that requires
  weakening an assertion (report, don't weaken).
