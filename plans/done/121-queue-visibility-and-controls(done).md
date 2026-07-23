# Plan 121: queue visibility + clear/skip from the settings window

> Filed 2026-07-23 (operator request, post-close-out bug list). Drift
> check: authored against master `7ee95d3`. If `src-tauri/src/queue.rs`,
> `engine.rs`, `settings.rs`, `settings_commands.rs`, or
> `src/settings/` have changed since, re-verify every line citation
> before executing.

## Problem

The operator cannot see what's waiting in the notification queue, nor
act on it. Today the only queue visibility is the `queue_total`/
`queue_done` progress counts on a showing card. Waiting items live in
`SingleSlotQueue.waiting: [VecDeque<QueueItem>; 3]` (`queue.rs:52`),
private; `waiting_titles()` exists only as `#[cfg(test)]`
(`queue.rs:980`). No invoke command, HTTP route, or event exposes the
list, and no clear/flush operation exists (mutators are
`dismiss_visible`, `skip_visible`, `pause`, `resume`,
`queue.rs:528-577`).

## Scope

Three new settings-window invoke commands + one new settings section:

- `get_queue` → list of waiting items (read-only summary).
- `clear_queue` → drop ALL waiting items (visible card untouched —
  it finishes its normal TTL/rotation).
- `skip_current` → dismiss the visible card now, promoting the next
  waiting item (routes through the existing `skip_visible` semantics).

"Release all at once" from the operator's ask maps to `skip_current`
(repeatable) + the queue's own rotation — a single-slot UI cannot show
N cards simultaneously; do NOT invent a burst-display mode.

## Non-goals / hard constraints

- `capabilities/default.json` must remain byte-identical (overlay
  stays receive-only). Done criterion.
- No new HTTP routes on `/notify`'s router.
- Queue item titles/bodies are UNTRUSTED wire data. In the settings
  UI render them as plain text only — no `dangerouslySetInnerHTML`,
  no `<a href>`, no markdown rendering (same rule as the History
  section's link-as-literal-text precedent).
- Do not restructure the queue; add accessors/mutators only.

## Steps

1. **Rust — queue.rs.** Add:
   - `pub fn waiting_summaries(&self) -> Vec<QueueItemSummary>` —
     iterate tiers high→normal→low in promotion order; summary fields:
     `title: String`, `priority: String` (the tier), `source: String`
     (derive from the item's existing origin/source field — read
     `QueueItem`'s actual shape first and use what's there; if no
     source-ish field exists, omit the field rather than fabricating
     one). Derive `Serialize` on the DTO (lives in `queue.rs` or
     `event.rs`, executor's call — follow existing DTO placement
     conventions, e.g. `ConnectorHealthDto`).
   - `pub fn clear_waiting(&mut self) -> usize` — empties all three
     tiers, returns count dropped. Decide and PIN with a test what
     happens to `batch_total`/`batch_done` (`queue.rs:75-76`): the
     invariant to preserve is "the visible card's progress dots never
     show done > total"; simplest correct move is to recompute/clamp
     `batch_total` so the visible card shows `done/done`-style
     completion, mirroring how `reset_batch_if_idle`
     (`queue.rs:607-614`) already reasons. STOP if this can't be done
     without touching promotion logic.
2. **Rust — engine.rs.** Expose both through the Engine's existing
   apply/read discipline (`apply`/`read`/`*_blocking` — mutation MUST
   go through `apply` so wake/emit ordering is preserved). After
   `clear_queue` and `skip_current`, the overlay must receive a fresh
   slot-state emit so the progress dots update — reuse the existing
   emit path that `dismiss`/`skip` flows already use; do not add a new
   emit mechanism.
3. **Rust — settings commands.** `#[tauri::command]`s `get_queue`,
   `clear_queue`, `skip_current` in `settings.rs` (follow
   `clear_history`/`get_history` shape). Register in ALL required
   places: `settings_commands.rs` `SETTINGS_COMMANDS` allowlist
   (`:42-54` — the single source `build.rs` includes),
   `capabilities/settings.json` (three `allow-*` perms), lib.rs
   `generate_handler!`. The existing parity tests in
   `settings_commands.rs` must pass — they are the gate that all
   three lists agree. 11 → 14 commands.
4. **Frontend.** 
   - `src/settings/types.ts`: `QueueItemSummary` type.
   - `src/settings/ipc.ts`: add the three commands to
     `SettingsCommands` (`get_queue` → `QueueItemSummary[]`,
     `clear_queue` → `null` (match the rust return through the
     existing convention — if rust returns the dropped count, type it
     `number`), `skip_current` → `null`).
   - New `src/settings/sections/QueueSection.tsx`: fetch-on-open +
     manual Refresh (Diagnostics-section precedent), list rendered as
     plain text rows (title + priority tag), empty state ("Queue is
     empty"), buttons **Skip current** and **Clear queue** wired
     through the shared `useActionStatus`/`ActionStatus` mechanism
     (plan-108 conventions: user-initiated actions report outcome,
     aria-live). Refetch the list after either action.
   - Register the section in `SettingsApp.tsx` nav (follow an
     existing section registration exactly).
5. **Docs.** Every present-tense "eleven invoke commands" count →
   fourteen, with the three new names added to enumerations: CLAUDE.md
   ("ipc & security" section), AGENTS.md (same section),
   `docs/V5_TECHNICAL_SPEC.md` §2, and any ARCHITECTURE.md
   present-tense count (grep `eleven`; per the plan-106 governing
   rule, dated/historical statements stay untouched).
6. **Tests.** Rust: `clear_waiting` empties all tiers + returns count
   + batch-counter invariant; `waiting_summaries` ordering (tiers,
   FIFO within tier); engine-level test that clear triggers an emit.
   Frontend: QueueSection render (list, empty state), clear/skip
   invoke + refetch + ActionStatus assertions (mock via `mockIPC`).
   Update `docs/TESTING_STRATEGY.md` §0 counts.

## Verification ladder

`cargo test --locked` (from src-tauri/), `cargo clippy -- -D
warnings`, `cargo fmt --check`, `npx tsc --noEmit`, `npx vitest run`,
`npx biome ci .`, `npx vite build`. Plus: `git diff
src-tauri/capabilities/default.json` is empty.

## STOP conditions

- `QueueItem` lacks a usable title field (would mean the test helper
  lie — investigate, don't improvise).
- The batch-counter invariant can't be preserved without touching
  promotion logic.
- Any need to change `capabilities/default.json`.
