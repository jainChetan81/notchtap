# Agent session viewing: wire the missing display + add auto-advance

**Date:** 2026-08-02
**Status:** approved, pending implementation plan

## Problem

Two related gaps, discovered together while designing this feature:

1. **The manual prefix-key session cycling isn't wired to the display.**
   `PrefixAction::NextSession`/`PreviousSession` (`src-tauri/src/prefix.rs`)
   is dispatched from `handle_prefix_followup`
   (`src-tauri/src/lib.rs:2129-2159`), which mutates `tab_wire.viewed_session`
   and emits `agent-viewed-session-changed` with `{ index: next }`
   (`lib.rs:2151-2157`). But `StatusRailCard.tsx:1107-1112` calls
   `TabBelowBlock` without ever passing `viewedSessionIndex` — it's
   `undefined`, so `AgentBelowBlock` always defaults to the first session
   (`TabBelowBlock.tsx:96-104`'s own comment documents this default and
   notes it predates the prefix wiring landing). Pressing the prefix +
   next/prev keys today changes rust state that nothing displays.

2. **There's no auto-advance.** The user wants the Agent tab's session
   view to automatically cycle through sessions on a timer (pausing while
   hovered), instead of requiring a manual keypress every time.

## Non-goals

- No new UI dependency. `shadcn`/`tailwind` are deliberately scoped to
  the settings window (`src/settings/**`) only — every existing consumer
  of `src/components/ui/*` lives under `src/settings/`. The overlay
  (`src/components/*` outside settings) has never used them; it's a
  hand-rolled CSS system built around GPU-only transform/opacity
  animations and the `--ease-notchtap-*` token family, with its own
  animation-review discipline. This spec does not add
  `embla-carousel-react` or any shadcn Carousel to the overlay.
  `PositionBar` (already built, plan 171 slice F) is the fit-for-purpose
  position indicator and needs no changes to its own logic.
- No change to `AgentBoard.tsx` (the automatic full-board surface,
  hero + scrollable rest-list). This spec is scoped to the Agent tab on
  `StatusRailCard` (`AgentBelowBlock`/`TabBelowBlock`) only.
- No change to the queue's own rotation (`engine.rs`'s `spawn_rotation`,
  which decides which CARD — football/news/weather/agent/manual — is
  promoted into the single slot). This spec is about which SESSION is
  shown once the Agent tab/card is already selected — a narrower,
  unrelated concept that happens to share the word "rotate."

## Approach

### Part 1 — wire the missing display (prerequisite)

Add `src/useAgentViewedSession.ts`, mirroring `src/useTabSelection.ts`'s
exact shape (listen-only hook, strict payload validator, dead-listener
`console.error`, no boot seed — same rationale `useTabSelection.ts`'s own
header comment gives, since a viewed-session index is only meaningful
while sessions exist):

```ts
export type AgentViewedSessionPayload = { index: number };
export function isValidAgentViewedSession(v: unknown): v is AgentViewedSessionPayload
export function useAgentViewedSession(): number // defaults to 0
```

Wire it into `StatusRailCard.tsx`: call the hook once, pass the result as
`TabBelowBlock`'s `viewedSessionIndex` prop (currently omitted at
`StatusRailCard.tsx:1107-1112`). `TabBelowBlock`'s existing
`viewedSessionIndex?: number` prop and its pass-through to
`AgentBelowBlock` (`TabBelowBlock.tsx:124-129`) need no changes — they
were already built to receive this value, just never given a real
source. Once this lands, `⌃⇧[`/`⌃⇧]` (or whatever the followup keys
resolve to per `prefix.rs`'s `PrefixKey::BracketLeft`/`BracketRight`)
will visibly cycle sessions for the first time.

### Part 2 — rust-side auto-advance

A new periodic task, spawned once alongside the app's other background
work (`src-tauri/src/lib.rs`'s `.setup()`), advances the SAME
`tab_wire.viewed_session` the manual path uses — one source of truth, per
the decision above. Each tick (interval TBD by implementation, default
recommendation: 6 seconds — no existing precedent for "auto-cycle
through display cards" in this codebase, so this is a fresh choice, not
a matched convention; easy to retune) checks, in order:

1. Is the Agent tab currently selected (`tab_wire.tabs.selection() ==
   Some(Tab::Agent)`)? If not, skip this tick — no point advancing
   something nobody's looking at, and it avoids the session jumping
   several positions the moment the user switches back to the Agent tab
   after being away.
2. Are there 2+ sessions (`tab_wire.agent_sessions.load(..) > 1`)? If
   not, skip — nothing to advance to.
3. Is the app currently hovered (`was_hovered`, the same
   `Arc<Mutex<bool>>` already threaded through `lib.rs`'s hover-consumer
   call sites)? If hovered, skip this tick — matches the auto-advance
   request's own "pausing on hover" requirement.
4. Is the app paused (`status.paused`)? If paused, skip — matches every
   other "the engine isn't delivering anything right now" precedent in
   this codebase (`StatusDots`' own pause handling, plan 092).

If none of those skip conditions apply: increment `viewed_session`
(same wraparound arithmetic `PrefixAction::NextSession` already uses,
`lib.rs:2143-2150`) and emit `agent-viewed-session-changed` — the exact
same event `Part 1`'s hook already listens for, so the auto-timer and
the manual keys are indistinguishable to the frontend.

**Manual navigation resets the auto-advance clock.** A manual
`NextSession`/`PreviousSession` should restart the auto-advance
interval's countdown (not just let the next auto-tick land arbitrarily
soon after) — otherwise a user cycling manually would feel the
auto-advance "fighting" them by jumping again almost immediately after
their own keypress. Implementation detail: this likely means the
auto-advance task tracks "time of last advance (manual OR auto)" rather
than firing on a bare fixed-cadence `tokio::time::interval`.

**Deliberately simpler than `queue.rs`'s hover-banking precedent.**
`queue.rs`'s TTL rotation banks exact elapsed time across hover
sessions (`hover_enter`/`hover_exit`/`rotate_out_if_elapsed`) because a
visible countdown's exact remaining time matters. This feature has no
visible countdown — it's "skip this tick if hovered, try again next
tick" — so a full elapsed-time-banking implementation is more than this
needs. If it feels janky in practice (e.g. a long hover followed by an
immediate advance the instant the cursor leaves), escalate to the
banked-time pattern; don't build it preemptively.

## Data flow

```
rust: tick fires -> gate checks -> tab_wire.viewed_session.store(next)
    -> emit("agent-viewed-session-changed", {index: next})
frontend: useAgentViewedSession() listener -> StatusRailCard
    -> TabBelowBlock viewedSessionIndex -> AgentBelowBlock -> PositionBar
```

Same path for both manual (`handle_prefix_followup`) and automatic
(new timer) advances — they differ only in what triggers the store+emit,
never in how the frontend receives it.

## Testing

- `useAgentViewedSession.test.ts` (new), mirroring
  `useTabSelection.test.ts`'s structure: valid payload updates state,
  malformed payload is ignored, listener registration failure logs to
  console.
- `StatusRailCard.test.tsx`: a test confirming `viewedSessionIndex` from
  the hook actually reaches `AgentBelowBlock`/`PositionBar` (an
  integration pin, not just the hook's own unit test).
- Rust: unit tests for the gate-check logic (tab not selected → skip,
  1 session → skip, hovered → skip, paused → skip, none of the above →
  advances) as a pure function taking the relevant booleans/counts,
  matching this codebase's established "keep the decision logic
  unit-testable, keep the subprocess/async-runtime call thin" split
  (`docs/TESTING_STRATEGY.md` §4.4, already used for
  `presentation_mode`). Manual-reset-clears-auto-timer behavior needs
  its own test too.

## Escape hatch

If `tab_wire.tabs.selection()` or `was_hovered` turn out not to be
cleanly readable from a freshly-spawned async task without restructuring
how they're currently threaded through `.setup()`'s closures, STOP and
report back rather than widening this task's access in an ad hoc way —
that's a sign the state needs a cleaner shared handle, which is worth
doing deliberately rather than as a side effect of this feature.
