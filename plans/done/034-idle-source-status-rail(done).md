# Plan 034: Idle source-status rail (`status-state` channel)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving on.
> If anything in "STOP conditions" occurs, stop and report. When done,
> update this plan's status row in `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat d926977..HEAD -- src-tauri/src/poller.rs src-tauri/src/lib.rs src/components/IdleView.tsx src/useSlotState.ts`
> On any change, re-verify the excerpts below; mismatch = STOP.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (first *second* event channel into the overlay — the
  receive-only rule is preserved, but the race-shield discipline of
  slot-state must be duplicated exactly)
- **Depends on**: **036 — land it first** (functional, not just
  ordering: Step 3's whole design premise is "the heartbeat is the sole
  emitter because every mutation's `notify_waiters()` reaches it" —
  and plan 036 exists precisely because the current heartbeat loop can
  LOSE those notifications (waiter registered after the lock is
  released). On the unfixed loop, a status change can go unemitted
  until the next unrelated wake. 036 is P1/S and independent — do it
  first); soft: 032 for the idle CSS region
- **Reviewed**: 2026-07-18 at `1add02e` (review-plan pass) — all
  excerpts re-verified (poller snapshot ownership, both
  `spawn_heartbeat` sites at lib.rs:342/:1061, capabilities content,
  IdleView clock-only); 036 promoted to a functional dependency,
  git-status criterion added
- **Category**: engine/ui
- **Planned at**: commit `d926977`, 2026-07-18, prototype rev-3 session
  (idle demos: "all clear", "football live", "news paused · 2 queued").

## Decisions locked (2026-07-18)

1. **One combined `status-state` event**, not extra fields on slot-state:
   poller state changes independently of promotions, and slot-state's
   change-guard is keyed to display content. The new channel duplicates
   the slot-state delivery pattern *exactly*: rust emits on change +
   plants `window.__NOTCHTAP_STATUS_STATE__` via eval on page load
   (same `escape_for_eval_splice` helper, lib.rs:402's pattern).
2. **Payload shape** (camelCase on the wire):

   ```json
   {
     "paused": false,
     "waiting": 3,
     "football": { "enabled": true, "live": { "label": "Arsenal 2–0", "minute": "45'" } },
     "news": { "enabled": true }
   }
   ```

   `football.live` is `null` when no watched match is in-play. "News
   paused" in the idle rail means `news.enabled === false` (polling gates
   are boot-config since v6 — there is no runtime poll pause to report).
3. **The overlay stays receive-only** — this adds a listen-only channel,
   no invoke, no capability change (`capabilities/default.json`
   byte-identical at the end).

## Why this matters

Today the idle card is a clock. The operator wants it to answer "what's
happening / what's next": a live match with score+minute, news on/off,
and how many items sit behind the empty slot.

## Current state (verified at `d926977`)

- `IdleView.tsx` renders the clock + day-progress timeline only.
- `poller.rs:124-127` — `Snapshot = HashMap<String, MatchSnapshot>`; the
  poll loop owns `snapshots` (:575-610) and already knows which matches
  are in-play, their scores, and status text (it diffs them every tick).
  Nothing of this leaves the module today.
- `lib.rs:375-422` — `on_page_load` plants `__NOTCHTAP_SLOT_STATE__` and
  `__NOTCHTAP_APPEARANCE__` via eval; `escape_for_eval_splice` (plan 014)
  is the tested escaping helper both sites use.
- `queue.rs` — `total_waiting()` (:353-355) and `is_paused()` exist.
- Frontend channel discipline to copy: `useSlotState.ts` (validator +
  global-seed + listener + dead-listener console.error).
- `capabilities/default.json` grants `core:event:allow-listen`
  name-agnostically — the new channel needs **no** capability edit; the
  byte-identical requirement in Done criteria is satisfiable (verified
  at planning time, 2026-07-18).

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cargo test` from `src-tauri/` | all pass |
| Frontend tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope**:
- `src-tauri/src/status.rs` (new) — `StatusState` struct + serde +
  change-guard (`status_state_if_changed`) + `emit_status_state`
- `src-tauri/src/poller.rs` — a shared
  `Arc<Mutex<Option<LiveMatchSummary>>>` the poll loop refreshes each
  tick (label like `"Arsenal 2–0"`, minute/status text; `None` when
  nothing in-play)
- `src-tauri/src/lib.rs` — status recompute+emit on heartbeat wake,
  enqueue, and poller tick; seed global on page load
- `src/useStatusState.ts` (new) — validator mirroring `useSlotState.ts`
- `src/components/IdleView.tsx`, `src/styles.css` (status rail + chips),
  `src/settings/preview-overlay.css` (mirror)
- tests; `docs/TESTING_STRATEGY.md` §0 counts

**Out of scope**:
- A scoreboard *card* with Topic supersession (that's plan 031's spike —
  this rail is a readout, not a queue item)
- Any settings-window surface, any connector surface
- `capabilities/default.json` (must not change)

## Git workflow

- Current branch; one rust commit, one frontend commit. Do NOT push.

## Steps

### Step 1: `status.rs`

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusState {
    pub paused: bool,
    pub waiting: usize,
    pub football: FootballStatus,
    pub news: NewsStatus,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FootballStatus { pub enabled: bool, pub live: Option<LiveMatchSummary> }
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveMatchSummary { pub label: String, pub minute: String }
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsStatus { pub enabled: bool }
```

Plus `STATUS_STATE_EVENT: &str = "status-state"` pinned by a test (same
reasoning as `SLOT_STATE_EVENT`), a change-guard, and an emit fn mirroring
`emit_slot_state`. Unit-test serialization (camelCase, live=null case).

### Step 2: Poller live surface

`poller.rs` takes an `Arc<Mutex<Option<LiveMatchSummary>>>` written after
each poll tick: first in-play match across watched leagues →
`{ label: "Home X–Y Away", minute: status detail }`; none → `None`.
Tested via the existing fixture-diff harness (a live fixture populates
the summary; a full-time fixture clears it).

### Step 3: Wire the emissions + seed

**Precondition**: plan 036's lost-wakeup fix must already be in
`spawn_heartbeat` (its waiter registered under the queue lock /
`Notified` created before the deadline read — see
`plans/036-heartbeat-lost-wakeup.md`). If `lib.rs` still has the
plan-015 loop shape 036 describes as buggy, STOP and execute 036 first
— this step's sole-emitter design is unsound on the racy loop.

The heartbeat is the **sole emitter** — no second change-guard anywhere:
every mutation already reaches it, because plan 015's shared `Notify` is
fired by `enqueue_and_emit` (all push paths), the pollers, and the
hotkey/tray handlers. Each loop pass recomputes the `StatusState` under
the same queue lock as the slot-state tick and emits only when it differs
from the previous one (a `last_status` local in the heartbeat task).
The poller's only status-side act is writing the live-match handle after
each tick and calling `wake.notify_waiters()` when (and only when) the
summary actually changed. Lock discipline: nobody holds the live handle
and the queue lock at the same time — read/clone/drop the handle, then
lock the queue.
Page-load seed: `window.__NOTCHTAP_STATUS_STATE__ = {safe_json}` beside
the slot-state eval, same `escape_for_eval_splice` helper, plus one
`emit_status_state` (the dual-path race shield, exactly as slot-state).
Both `spawn_heartbeat` call sites take the new args (live handle,
`espn_enabled`, `rss_enabled`): the production spawn (lib.rs:342) and
the heartbeat test (lib.rs:1061).

### Step 4: Frontend

`useStatusState.ts`: validator (`paused` bool, `waiting` non-negative int,
`football.enabled` bool, `live` null-or-{label,minute} strings,
`news.enabled` bool), global seed + listener + dead-listener error —
mirror `useSlotState.ts` including the unmount guard. `IdleView` renders
the clock plus a `.src-rail` of chips: live match (pulsing green dot),
News / News paused (dimmed), `N queued` or `clear`. Idle card widens to
`calc(460px * var(--card-scale))` only while status chips are present
(new class, e.g. `.rail-card.idle.status`) so the plain clock idle keeps
its 270px. CSS in both files. Tests: validator accept/reject cases;
IdleView chip rendering for the three demo states.

## Test plan

- status.rs: serialization, change-guard, event-name pin
- poller.rs: summary populated/cleared from fixtures (no live network)
- lib.rs: seed-path eval uses the escaping helper (grep-level + existing
  splice tests stay green)
- frontend: useStatusState validator, IdleView three states
- §0 counts updated

## Done criteria

- [ ] `capabilities/default.json` byte-identical (`git diff` shows nothing)
- [ ] `cargo test`, `npx vitest run`, `npx tsc --noEmit`, `npx vite build` green
- [ ] `STATUS_STATE_EVENT` pinned by a rust test
- [ ] `rg "escape_for_eval_splice" src-tauri/src/lib.rs` shows the new seed
      site using the helper
- [ ] `git status --short` shows, beyond whatever was already dirty
      before your first edit (concurrent sessions share this checkout —
      snapshot it first, never revert/stage/commit those paths),
      modifications ONLY to in-scope files
- [ ] Manual: idle shows "all clear" with `espn_enabled=false`; a live
      fixture poll shows the match chip; `plans/README.md` row updated

## STOP conditions

- The poll loop's snapshot ownership has drifted (no single place a tick
  ends) — the summary write needs that chokepoint; report before adding
  a second writer
- Any pressure to let the overlay *request* status (pull) — that breaks
  the receive-only rule; stop
- `football.live` needs more than one live match (multi-league evenings) —
  stop and take it back to the operator rather than guessing a layout

## Maintenance notes

- The change-guard keeps this channel silent at steady state; the 12s
  `shade-drift` lesson (plan 018) applies — no per-tick re-renders in the
  webview for a status that didn't change.
- If plan 031 (scoreboard Topic card) ships later, the rail's live chip
  and the card coexist: rail = "is football on", card = the score itself.
