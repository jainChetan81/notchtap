# 136 — `agent-state` IPC + Agent Board resting state

> v7 ticket 4 of 13 (spec §6, §6.1, §6.2 resting). Filed 2026-07-26.

**What to build:** the Agent Board's first visible slice. After this
ticket, pushing sessions via `curl` makes the overlay's idle surface
show live, correctly ordered session rows whenever the Slot is empty.

- Rust publishes `agent-state` (revision, Rust-ordered
  `AgentSessionView[]`, `adapterHealth` — the health array may be
  empty/stub until ticket 143) independently of `slot-state` and
  `status-state`. Publish suppression uses ticket-133's `dedup_eq`
  (clock ticks must not publish).
- Overlay adds `useAgentState` (receive-only listen, mirroring
  `useSlotState` patterns) and renders with NO sorting, lifecycle
  inference, expiry, or history merging in TS.
- §6.1 presentation precedence: Visible Notification > Agent Board
  (when any live/retained session exists) > existing clock/weather/
  media idle. When an Agent card finishes, presentation returns to the
  still-current board.
- §6.2 resting layout: one rich card for the highest-ranked session
  (runtime, state, project, elapsed state time, latest safe summary);
  every other session represented individually — never a `+N`
  collapse; clear but non-alarming visual distinction between waiting
  / failure / working / completed. This is core overlay surface —
  visual quality bar applies (visually stunning, not merely
  functional).
- Capability-dependent cells omitted cleanly when undeclared.
- Expanded/hover state is ticket 142 — resting only here.

**Blocked by:** 134. (Independent of 135 — may run in parallel with it,
but coordinate: both touch engine/event surfaces.)

**Status:** ready-for-agent

- [ ] Rust test: `agent-state` publishes on content change only;
      clock-only updates suppressed.
- [ ] Frontend tests: resting render for waiting-permission, working,
      failed, completed, and 3+ independent sessions; precedence tests
      for slot-occupied vs board vs idle.
- [ ] No new invoke commands; overlay remains receive-only;
      `capabilities/default.json` unchanged.
- [ ] Manual: two curl'd sessions in different states render ordered;
      state change reorders; retention expiry drops the row.
- [ ] `cargo test` + `npx vitest run` green.
