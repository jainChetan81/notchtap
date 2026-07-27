# 146 — Silenced (sleep mode + timed mutes) and Priority Preemption

> Spec (PRD) from the 2026-07-27 grilling session. Glossary terms
> (Silenced, Silent Period, Timed Mute, Skip, Breakthrough, and the
> rewritten Priority/Expanded entries) are already in `CONTEXT.md`.

**Status:** implemented 2026-07-27 (both halves in one pass; two-axis
review passed — one real finding, the glanceable tray glyph, fixed.
manual checks pending: tray mute/skip feel, interrupt animation look on
real hardware, macbook notch pass)

> **Note, 2026-07-27 (same day, after this plan shipped)**: the
> telegram connector this plan references (Solution paragraph, story
> 11, Out of Scope) was removed by operator decision shortly after.
> silence remains display-only — it was never a connector concern to
> begin with, and there is currently no shipped connector for it to
> affect. read the telegram mentions below as historical.

## Problem Statement

notchtap treats every hour the same. Overnight, cards rotate to an
empty room; during a meeting, a news card can slide out mid-call. The
only lever is the tray Pause toggle, which is absolute (nothing shows,
not even an agent begging for input) and manual (it never turns itself
back on — forget it and the overlay is dead all day).

Separately, priority only orders the waiting line. A High event — an
agent blocked on a permission prompt, a goal — sits behind whatever
Medium or Low card happens to be mid-rotation. The one moment priority
matters most is the one moment it currently does nothing.

## Solution

Two connected changes.

**Silenced** — a scheduled/timed quiet state distinct from Paused.
While Silenced, Medium and Low events buffer exactly as under Paused
(nothing dropped; pollers untouched, idle clock/weather/Agent Board
unchanged; silence is display-only and any connector fan-out — none
shipped today — would be unaffected the same way); a High event still
promotes (**Breakthrough**)
as a compact card. Silence arrives two ways: the daily **Silent
Period** (default on, 00:00–10:00 local wall clock) and manual **Timed
Mutes** from the tray (30m / 1h / 2h presets, auto-resume). A tray
**Skip** ends today's window early. Overlapping silences union. Paused
stays absolute and sits above all of it. When silence ends, the
backlog drains under normal Rotation.

**Priority Preemption** — a strictly-higher-priority arrival cuts the
currently-Visible card short: High preempts Medium/Low, Medium
preempts Low, equal never preempts. The preempted card re-queues at
the head of its own tier with its remaining turn intact and returns
after every higher card has finished. Alongside this, Low promotions
render compact everywhere (no auto-Expanded opening; the manual expand
hotkey still works).

## User Stories

1. As the operator, I want no Medium/Low cards between midnight and 10:00, so that the overlay is not performing to an empty room overnight.
2. As the operator, I want overnight events buffered rather than dropped, so that sitting down at 10:00 shows me what happened while I slept.
3. As the operator, I want a High agent permission request to card even at 2am, so that an overnight agent run is never silently stalled by the schedule.
4. As the operator, I want Breakthrough cards to render compact, so that a card during a quiet period is as unobtrusive as a card can be.
5. As the operator, I want a one-click timed mute before a meeting, so that the overlay stays still for exactly that hour without me remembering to unmute.
6. As the operator, I want a mute to expire on its own, so that a forgotten mute costs me an hour of cards, not a day.
7. As the operator, I want to cancel a running Timed Mute from the tray, so that a meeting ending early ends the silence early.
8. As the operator, I want a Skip action when I sit down at 9:00, so that one click ends today's Silent Period without touching config.
9. As the operator, I want the schedule to re-arm at the next midnight after a Skip, so that skipping today never disables tomorrow.
10. As the operator, I want the tray Pause toggle and the `start_paused` Kill Switch to stay absolute, so that "show nothing" still means nothing — Breakthrough included.
11. As the operator, I want silence to be display-only, so that any outbound connector (Telegram, at the time this was written; removed 2026-07-27) forwards independently of the overlay's quiet hours — my phone (with its own night mode) remains the complete off-machine trail.
12. As the operator, I want pollers to keep observing during silence, so that no score or news change is missed, only deferred.
13. As the operator, I want the idle clock, weather peek, and Agent Board to behave normally while Silenced, so that the overlay stays alive as a status surface even when notifications are quiet.
14. As the operator, I want the tray icon to tell me the engine is Silenced, so that a quiet overlay is distinguishable from a broken one.
15. As the operator, I want the wake-time backlog to drain under normal Rotation with topic supersede already applied, so that 10:00 shows distinct, current items rather than a stale flood.
16. As the operator, I want the schedule in a config block edited from Settings, so that changing my morning boundary is a settings trip, not a code edit.
17. As the operator, I want the Silent Period on by default at 00:00–10:00, so that the app is quiet at night from first launch without setup.
18. As the operator, I want a High arrival to preempt a visible Medium or Low card immediately, so that an agent needing input never waits out someone else's rotation.
19. As the operator, I want a goal card (High by deliberate default) to take the screen and break sleep, so that a match I care about is never buffered behind quiet hours.
20. As the operator, I want a Medium arrival to preempt a visible Low card, so that a Low card never blocks real content.
21. As the operator, I want equal priority to never preempt, so that two Medium cards can never fight over the Slot.
22. As the operator, I want a preempted card re-queued at the head of its tier with its remaining time, so that an interruption costs the card time, not existence.
23. As the operator, I want the preemption handover animated as a deliberate exit-then-enter, so that a cut-short card reads as "something more important arrived", not as a glitch.
24. As the operator, I want Low cards to promote compact everywhere, so that the least important tier is also the least visually demanding.
25. As the operator, I want the manual expand hotkey to still grow a compact Low or Breakthrough card, so that quiet-by-default never means uninspectable.
26. As a pusher (CLI, `notchtap run`, adapters), I want my push accepted with the same status codes during silence, so that silence is invisible to the API contract.

## Implementation Decisions

- **Silenced is a queue-level gate beside `paused`, not a wrapper.**
  The queue owns promotion; it now consults `silenced` and the
  arriving event's Priority: High promotes, Medium/Low buffer. Paused
  is checked first and wins unconditionally.
- **Schedule evaluation is a pure function**: (configured window,
  local wall-clock time) → in-window or not, one window per day, local
  time, DST handled by wall-clock comparison. Active silences (Silent
  Period, Timed Mutes) union; the engine is Silenced until the last
  ends. The function never reads the clock itself — the caller passes
  time in (same pattern as the presentation-mode function).
- **Skip** clears only the current scheduled span; it re-arms at the
  next window start. Cancelling a Timed Mute clears only that timer.
  Skip/mute state is session-only (like the tray Pause toggle); the
  schedule itself is persisted config.
- **Config**: a `[silence]` block — `enabled` (default true) and one
  daily window string (default `"00:00-10:00"`) — parsed and validated
  with the existing config machinery, edited in the Settings General
  section, save-relaunch as today. No weekday matrix.
- **Preemption is queue-owned**: a strictly-higher-priority enqueue
  while a lower card is Visible triggers an interrupt exit; the
  preempted item re-enters at the head of its own tier carrying its
  remaining turn. Remaining-time bookkeeping lives in the queue item.
  Equal priority keeps the existing finish-your-turn contract.
- **Compact promotion is queue-owned**: the expand-on-promotion flag
  already computed at promotion time becomes priority- and
  state-aware — Low never auto-expands; a Breakthrough promotion never
  auto-expands; Medium/High keep the plan-033 expanded opening. Manual
  expand hotkey behaviour is unchanged. The frontend renders what the
  payload says; no new render path.
- **Default priorities are unchanged.** Manual Medium, news Low,
  football goals High (deliberate: goals preempt and break sleep),
  agent Permission/Input/Failure High, Completed Medium. All remain
  per-source configurable.
- **HTTP contract unchanged**: acceptance, buffering responses, and
  per-tier caps (50) behave exactly as under Paused; topic supersede
  keeps collapsing poller updates while Silenced.
- **Tray**: mute presets (30m / 1h / 2h), Skip-today, and a Silenced
  indicator sit beside the existing Pause toggle. Tray stays
  rust-side; the overlay stays receive-only — no new invoke commands,
  `capabilities/default.json` untouched. Any new Settings-window
  command follows the build.rs allowlist + settings capability + ACL
  test rule.
- **The preemption handover is an animation deliverable, not just a
  state change** — a distinct interrupt exit (faster than the natural
  exit) into the High card's entrance. Visual quality is core product
  work here. No reduced-motion variant (permanent repo non-goal).

## Testing Decisions

- Good tests assert external queue behaviour — what is Visible, what
  is Waiting in which order, what the promoted payload says — never
  internal fields. The queue's existing suite (including the
  invariant-style tests) is the prior art and the bulk of the new
  coverage lands beside it: silenced gating per priority, breakthrough
  promotion, return-to-silence after a breakthrough turn, preemption
  across every priority pair (including the equal-priority
  no-preempt cases), head-of-tier re-queue ordering, remaining-time
  restoration, compact-flag correctness for Low and Breakthrough
  promotions, paused-beats-silenced precedence, and wake-time drain
  ordering.
- The schedule function gets exhaustive clock-free unit tests:
  boundaries inclusive/exclusive at both ends, midnight-crossing
  windows, union of overlapping silences, skip-then-re-arm.
- Config parsing tests follow the existing config test style: default
  on, default window, malformed window strings rejected, disabled
  block honoured.
- Frontend tests (vitest, UTC-pinned) cover compact-vs-expanded
  rendering driven purely by the promoted payload, in the existing
  slot-state and presentation test suites.
- Manual-only (by design, per the testing strategy): tray menu feel,
  the interrupt exit animation's look on real hardware, notch-mode
  behaviour on the macbook.

## Out of Scope

- Calendar-driven silence (Google Calendar busy blocks) — rejected:
  pulls OAuth/network into a loopback-only app.
- Weekday/weekend schedule matrix and multiple daily windows.
- Silencing Connectors — silence is display-only; a Connector (Telegram
  shipped as the only one, then was removed 2026-07-27) would forward
  everything, always, unaffected by it.
- Dropping events (any tier) during silence or at wake; digest cards.
- Hiding the overlay window during silence.
- Per-source breakthrough exclusion lists.
- Breakthrough for the manual Paused state or Kill Switch.
- Changing any shipped default priority.
- Reduced-motion variants of the interrupt animation (permanent
  non-goal).

## Further Notes

- Ship as two plans: **146a Silenced** (gate, schedule, mutes, tray,
  config, breakthrough-compact) and **146b Preemption** (interrupt,
  re-queue, Low-compact-everywhere). 146a is the smaller, self-
  contained half; 146b rewrites the queue's oldest invariant ("a
  visible card always finishes its turn") and every test that pins it,
  plus one new animation.
- The Priority and Expanded glossary entries in `CONTEXT.md` were
  already rewritten for this spec (2026-07-27); code must land to
  match them or the glossary reverts.
- Decision-record follow-up: the Silenced-vs-Paused split and the
  preemption contract change belong in `ARCHITECTURE.md` once
  implementation starts.
