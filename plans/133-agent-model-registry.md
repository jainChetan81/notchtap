# 133 — Agent domain model + registry core

> v7 ticket 1 of 13, from `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md`
> (spec §2). Filed 2026-07-26 from the operator-approved /to-tickets
> breakdown.

**What to build:** the provider-neutral Agent domain model and the
authoritative in-memory Agent Registry — the foundation every other v7
ticket builds on. After this ticket, Rust code can feed normalized
Agent Events into a registry and get back correctly transitioned,
ordered, bounded session state, all proven by `cargo test`.

New `src-tauri/src/agents/` module with `mod.rs`, `model.rs`,
`registry.rs` (the other spec-listed files arrive with later tickets).
Implement spec §2 exactly:

- `AgentRuntime`, `AgentCapability`, `AgentEventKind`,
  `AgentSessionState`, `AgentSessionKey`, `AgentSession` (fields per
  §2), and a registry-internal normalized event type.
- `AgentSessionKey` is the sole identity; metadata never merges
  sessions; fallback IDs are process-identity + start-timestamp, never
  project path alone.
- §2.1 transition rules, including: waiting states cleared by
  work/tool events; non-terminal tool failure leaves state `Working`;
  `Stale` via `stale_after_secs`; terminal retention
  (`terminal_retention_secs`, default 600); terminal states never
  reactivate — an incorrectly reused terminal ID gets a suffixed
  fallback key, old history untouched.
- §2.2 ordering key (urgency class → state-entered → first-seen → key
  lexical tie-break), computed in Rust.
- Bounded per-session transition history (50) and remembered event IDs
  (2,048 LRU) per §3.2's caps table (the caps constants can live here
  or move to `adapter.rs` in ticket 134 — either way they are
  centralized once).
- Handwritten `AgentState::dedup_eq` per §2.3: clock-derived fields
  (elapsed, retention remaining, `last_seen_at_ms`) are not content;
  state/summary/capabilities/ordering/metadata/history are. Never
  derived `PartialEq` for publish suppression (same invariant as
  `SlotState::dedup_eq` — see CLAUDE.md).
- Registry lives behind the same application-state boundary as the
  Engine but is NOT part of the Notification Queue. No IPC, no HTTP,
  no notification mapping in this ticket.
- `thiserror` for registry error variants (library module rule).

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] Every §2.1 transition rule has a passing unit test, including
      terminal-never-reactivates and the suffixed-fallback path.
- [ ] Ordering tests prove urgency-then-FIFO and that a state change
      re-enqueues into the destination class.
- [ ] Identity tests prove two sessions sharing runtime+project never
      merge, and duplicate `eventId` / stale `sequence` produce no
      state change.
- [ ] `dedup_eq` tests prove clock-only changes compare equal and each
      content field compares unequal.
- [ ] Retention/stale tests use injected clocks (no wall-clock sleeps).
- [ ] `cargo test` green from `src-tauri/`; no changes outside the new
      module besides module registration.
