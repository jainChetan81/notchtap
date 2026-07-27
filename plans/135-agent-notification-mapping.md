# 135 — Noteworthy Agent Events become Notifications

> v7 ticket 3 of 13 (spec §5). Filed 2026-07-26.

**What to build:** noteworthy Agent Events enter the existing Engine as
real Notifications. After this ticket, `curl`-ing a
`permission_requested` event makes a High-priority card appear in the
overlay Slot, while progress events update only the registry.

- New `agents/notification.rs` mapping per the §5 table: Permission
  Requested / Input Required / Failed → High one-shot; Completed →
  Medium one-shot; Informational → off by default (policy hook for
  later config).
- Introduce `Origin`/`SourceKind` value `Agent` and
  `EventType::AgentEvent` with an `AgentSignal` (runtime, session key,
  kind, sanitized summary); `EventMeta.agent` carries presentation
  metadata. Origin is always `Agent` regardless of runtime/Host.
  (This ticket only ADDS `Agent`; removing/aliasing `Cmux` is
  ticket 137.)
- Starting/Working/tool/subagent progress never create cards.
- Noteworthy events obey all existing Queue/Slot rules (Priority,
  Rotation Order, tier caps, Paused, Promotion). Queue-full
  independence: registry update still accepted, endpoint returns `202`
  with diagnostic `notificationQueued: false` (also the §10
  `agent.notification_queued` log field).
- Notification creation never deletes or replaces session history.

**Blocked by:** 134 (`POST /agent/events`).

**Status:** ready-for-agent

- [ ] Mapping tests: every `AgentEventKind` → expected
      priority/one-shot or no-card, informational suppressed by
      default.
- [ ] Queue-full test proves registry accepts + `202` +
      `notificationQueued: false` when the tier is full.
- [ ] Progress-event test proves no card is created and history is
      preserved across a card's lifetime.
- [ ] Manual: `curl` permission event → card visible in overlay;
      completed event → Medium card.
- [ ] `cargo test` green.
