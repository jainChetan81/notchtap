//! Plan 133: the authoritative in-memory Agent Registry (spec §2's
//! transition rules, §2.2's ordering, §2.3's dedup contract, §3.2's
//! caps table).
//!
//! Clock-agnostic like `queue.rs`: [`AgentRegistry::apply_event`] and
//! [`AgentRegistry::tick`] both take `now: Instant` from the caller —
//! no wall-clock read happens inside this module, so tests drive a
//! simulated clock instead of sleeping (CLAUDE.md's injected-clock
//! rule).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::model::{
    AgentCapability, AgentDetail, AgentEventKind, AgentHost, AgentProject, AgentSession,
    AgentSessionKey, AgentSessionState, AgentState, AgentSubagentSummary,
};

/// Retained transitions per session (spec §3.2 caps table).
pub const MAX_TRANSITIONS_PER_SESSION: usize = 50;
/// Remembered event ids, LRU (spec §3.2 caps table).
pub const MAX_REMEMBERED_EVENT_IDS: usize = 2048;
/// Default `agents.terminal_retention_secs` (spec §2.1; 600 -> 60 by
/// operator decision 2026-07-27 — see `AgentsConfig`'s field doc).
pub const DEFAULT_TERMINAL_RETENTION: Duration = Duration::from_secs(60);
/// Default `agents.stale_retention_secs` (plan 146 follow-up; 1800 -> 600
/// with the 2026-07-27 stale-timing tighten — see `AgentsConfig`).
pub const DEFAULT_STALE_RETENTION: Duration = Duration::from_secs(600);

/// The registry-internal normalized event — the input to
/// [`AgentRegistry::apply_event`]. This is deliberately a superset of
/// the wire-facing [`AgentEventKind`]'s five values: the wire `kind` +
/// the sibling `terminal` flag together are enough to drive every
/// §2.1 transition, including the ones (session start, generic
/// tool/work progress) that don't get a dedicated wire `kind` of their
/// own — see [`next_state`]'s doc. Building this from an actual
/// `/agent/events` POST body is ticket 134's `adapter.rs`; this ticket
/// only needs the type to exist so the registry can be exercised
/// directly from tests.
#[derive(Debug, Clone)]
pub struct AgentEvent {
    pub event_id: String,
    pub session_key: AgentSessionKey,
    pub sequence: Option<u64>,
    pub kind: AgentEventKind,
    /// Mirrors spec §3.1's top-level `terminal` boolean. Authoritative
    /// for terminality: kind `Failed` with `terminal` false is a
    /// non-terminal tool failure (session stays/becomes `Working`);
    /// kind `Informational` with `terminal` true is treated as a
    /// graceful `Completed` — see [`next_state`].
    pub terminal: bool,
    /// The adapter's OWN belief about the session's state (spec §3.1's
    /// `state`). NOT authoritative — [`next_state`] still decides every
    /// transition (spec §2.1). It is carried for exactly one purpose:
    /// telling a real session start apart from a mid-session
    /// informational event, which are otherwise identical on the wire
    /// (both `kind: informational`, both non-terminal). See
    /// `apply_event`'s `is_session_start`.
    pub declared_state: AgentSessionState,
    pub capabilities: Vec<AgentCapability>,
    pub summary: Option<String>,
    pub details: Vec<AgentDetail>,
    pub project: Option<AgentProject>,
    pub host: Option<AgentHost>,
    pub subagent: Option<AgentSubagentSummary>,
}

/// What happened when an event was fed to the registry. None of these
/// are errors — duplicate/stale events are a valid, expected part of
/// at-least-once delivery (spec §3.2: both map to an idempotent `202`
/// at the HTTP layer, ticket 134).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// A new or existing session accepted the event; its state may or
    /// may not have changed as a result (a second `Working` event while
    /// already `Working` is still `Applied`, just a no-op transition).
    Applied,
    /// This `event_id` was already seen — no registry change.
    DuplicateEventId,
    /// The event carried a `sequence` at or below the session's last
    /// accepted sequence — no registry change.
    StaleSequence,
}

/// Pure transition function (spec §2.1), unit-testable independent of
/// the registry's bookkeeping. `current` is the session's state going
/// into this event.
///
/// Note this function alone cannot implement "SessionStart → Starting,
/// then a work/tool event → Working" (spec §2.1's first rule): the wire
/// taxonomy has no dedicated `SessionStart` kind (spec §4.2 — a
/// provider's generic/lifecycle notifications, including session start,
/// arrive as `Informational`, same as ordinary tool progress), so an
/// `Informational` event is ambiguous between "this session just
/// started" and "this session is still working" from `kind` alone. The
/// caller (`AgentRegistry::apply_event`) resolves that ambiguity using
/// information this pure function doesn't have — whether the event is
/// the session's first — by skipping the call to `next_state` entirely
/// for a first-event `Informational`, leaving the freshly created
/// session at its `Starting` baseline. Every other kind (including a
/// first event that's already e.g. `PermissionRequested`) always runs
/// through `next_state` normally, first event or not.
///
/// Terminal states never transition back to active (spec §2.1's last
/// paragraph) — enforced here defensively (`current.is_terminal()`
/// short-circuits to `current`) even though `AgentRegistry::apply_event`
/// never calls this with a terminal `current` in practice (it redirects
/// to a suffixed fallback key first).
pub fn next_state(
    current: AgentSessionState,
    kind: AgentEventKind,
    terminal: bool,
) -> AgentSessionState {
    if current.is_terminal() {
        return current;
    }
    match kind {
        AgentEventKind::PermissionRequested => AgentSessionState::WaitingForPermission,
        AgentEventKind::InputRequired => AgentSessionState::WaitingForInput,
        // A completed event that is NOT terminal (a per-turn provider
        // `Stop`, OpenCode `session.idle`) keeps the session live: the
        // turn finished and the agent awaits the user, so it lands in
        // `WaitingForInput` rather than the terminal `Completed` state.
        // Only an explicit session-end event is terminal (spec §2.1,
        // operator decision 2026-07-26).
        AgentEventKind::Completed if terminal => AgentSessionState::Completed,
        AgentEventKind::Completed => AgentSessionState::WaitingForInput,
        // Non-terminal tool failure: informational/failure Notification
        // (ticket 135) while the session remains `Working` (spec §2.1).
        // This also clears an existing waiting state, same as any other
        // tool/work event — a failed tool call is still evidence of
        // work happening.
        AgentEventKind::Failed if terminal => AgentSessionState::Failed,
        AgentEventKind::Failed => AgentSessionState::Working,
        // A terminal `Informational` event (no dedicated `Completed`/
        // `Failed` kind, but the adapter marked it session-ending) is
        // treated as a graceful completion rather than left unresolved.
        AgentEventKind::Informational if terminal => AgentSessionState::Completed,
        AgentEventKind::Informational => AgentSessionState::Working,
    }
}

/// The authoritative in-memory Agent Registry (spec §2).
pub struct AgentRegistry {
    sessions: HashMap<AgentSessionKey, AgentSession>,
    /// Global (not per-session) LRU of accepted event ids — duplicate
    /// detection is cross-session by design, matching spec §3.2's caps
    /// table having a single "remembered event IDs" row, not one per
    /// session.
    seen_event_ids: HashSet<String>,
    seen_event_id_order: VecDeque<String>,
    /// How many times each original key has been reused after going
    /// terminal — feeds `AgentSessionKey::suffixed`'s generation number
    /// so repeated collisions on the same id keep producing distinct
    /// keys.
    reuse_generations: HashMap<AgentSessionKey, u32>,
    // Both read by `tick`/`ordered_states`, wired into the live
    // `agent-state` publish path (`agents/board.rs`'s
    // `AgentBoardPublisher`) from real `[agents]` config at the
    // `lib.rs` construction site (plan 137). `stale_retention` mirrors
    // `terminal_retention`'s role for `Stale` sessions (plan 146
    // follow-up) — see `tick`'s doc.
    stale_after: Duration,
    terminal_retention: Duration,
    stale_retention: Duration,
}

impl AgentRegistry {
    /// `stale_after`, `terminal_retention`, and `stale_retention` are all
    /// injected constructor parameters, sourced from `agents.stale_after_secs`,
    /// `agents.terminal_retention_secs`, and `agents.stale_retention_secs`
    /// respectively. Use [`DEFAULT_TERMINAL_RETENTION`] for the spec's
    /// default terminal retention.
    pub fn new(
        stale_after: Duration,
        terminal_retention: Duration,
        stale_retention: Duration,
    ) -> Self {
        Self {
            sessions: HashMap::new(),
            seen_event_ids: HashSet::new(),
            seen_event_id_order: VecDeque::new(),
            reuse_generations: HashMap::new(),
            stale_after,
            terminal_retention,
            stale_retention,
        }
    }

    // Only called from this module's tests and from
    // `AgentRegistryHandle::session_count` (itself `#[cfg(test)]`) until
    // a live caller (e.g. Settings' session count, or a future test
    // hook) exists outside test builds.
    #[allow(dead_code)]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn get(&self, key: &AgentSessionKey) -> Option<&AgentSession> {
        self.sessions.get(key)
    }

    fn remember_event_id(&mut self, event_id: String) {
        if !self.seen_event_ids.insert(event_id.clone()) {
            return;
        }
        self.seen_event_id_order.push_back(event_id);
        while self.seen_event_id_order.len() > MAX_REMEMBERED_EVENT_IDS {
            if let Some(oldest) = self.seen_event_id_order.pop_front() {
                self.seen_event_ids.remove(&oldest);
            }
        }
    }

    /// Feeds one normalized event into the registry (spec §2.1). See
    /// [`ApplyOutcome`] for the three possible results.
    ///
    /// Metadata merge policy (not spelled out verbatim by the spec, so
    /// resolved here): `summary`/`details`/`subagent` are always
    /// replaced with the incoming event's values (spec's own wording —
    /// "latest sanitized summary and bounded detail cells" — implies
    /// each event's payload is authoritative, even to clear a previous
    /// value). `project`/`host`/`capabilities` instead merge-if-present
    /// (an event that omits them doesn't erase what a previous event
    /// already established) since they read as persistent identity-
    /// adjacent facts rather than a point-in-time status line.
    pub fn apply_event(&mut self, event: AgentEvent, now: Instant) -> ApplyOutcome {
        if self.seen_event_ids.contains(&event.event_id) {
            return ApplyOutcome::DuplicateEventId;
        }

        let mut target_key = event.session_key.clone();
        if let Some(existing) = self.sessions.get(&target_key) {
            if existing.is_terminal() {
                // Terminal states never reactivate (spec §2.1). Treat
                // this as a provider incorrectly reusing a terminal id:
                // redirect to a suffixed fallback key so the original
                // session's history is left completely untouched.
                let generation = self
                    .reuse_generations
                    .entry(event.session_key.clone())
                    .or_insert(0);
                *generation += 1;
                target_key = event.session_key.suffixed(*generation);
            } else if let Some(seq) = event.sequence {
                if let Some(last) = existing.last_accepted_sequence {
                    if seq <= last {
                        return ApplyOutcome::StaleSequence;
                    }
                }
            }
        }

        self.remember_event_id(event.event_id);

        let is_new_session = !self.sessions.contains_key(&target_key);
        let session = self
            .sessions
            .entry(target_key.clone())
            .or_insert_with(|| AgentSession::new(target_key, now));

        // See `next_state`'s doc: a session-start `Informational` is
        // SessionStart in disguise (no dedicated wire kind exists for
        // it) and must leave the session at its `Starting` baseline
        // rather than immediately advancing to `Working`.
        //
        // `declared_state` is what disambiguates it. "New to this
        // registry" is NOT enough on its own: notchtap restarting while
        // an agent session is already running makes that session's next
        // ordinary event — a `PostToolUse`, say — the first one this
        // registry has ever seen, and it is `Informational` and
        // non-terminal just like a real SessionStart. Keying only on
        // novelty pinned every live session at `Starting` after every
        // restart, with its elapsed timer ticking up beside a summary
        // that plainly said work had happened. The adapters already
        // distinguish the two (`state: "starting"` vs `state:
        // "working"`, see `providers/claude_code.rs`); this reads that
        // rather than guessing.
        let is_session_start = is_new_session
            && event.kind == AgentEventKind::Informational
            && !event.terminal
            && event.declared_state == AgentSessionState::Starting;
        let new_state = if is_session_start {
            session.state
        } else {
            next_state(session.state, event.kind, event.terminal)
        };
        if new_state != session.state {
            session.state = new_state;
            session.state_entered_at = now;
            session.push_history(new_state, now, MAX_TRANSITIONS_PER_SESSION);
            if new_state.is_terminal() {
                session.terminal_at = Some(now);
            }
        }
        session.last_seen_at = now;
        if event.sequence.is_some() {
            session.last_accepted_sequence = event.sequence;
        }
        if !event.capabilities.is_empty() {
            session.capabilities = event.capabilities;
        }
        session.summary = event.summary;
        session.details = event.details;
        if event.project.is_some() {
            session.project = event.project;
        }
        if event.host.is_some() {
            session.host = event.host;
        }
        session.subagent = event.subagent;

        ApplyOutcome::Applied
    }

    /// Advances time-only state (spec §2.1): non-terminal, non-stale
    /// sessions with no accepted event for `stale_after` become `Stale`
    /// (including waiting sessions — spec: "waiting sessions ... can
    /// become Stale only through the explicit stale threshold", i.e.
    /// this same rule applies to them too, just not any TTL-style
    /// rotation). Terminal sessions past `terminal_retention` since
    /// going terminal are purged from the live registry entirely (spec:
    /// "then leave the live registry view"), and — mirroring that —
    /// `Stale` sessions past `stale_retention` since entering `Stale`
    /// are purged too, using `state_entered_at` (reset the instant a
    /// session goes `Stale`, above) as the stale-entry timestamp. Without
    /// this a stale session would sit on the board forever, permanently
    /// suppressing the idle face.
    ///
    /// Driven by `agents::board::AgentBoardPublisher::spawn_tick`'s
    /// periodic loop (ticket 136), mirroring the Engine's own rotation
    /// loop shape but time-interval- rather than deadline/wake-driven —
    /// see that method's doc.
    pub fn tick(&mut self, now: Instant) {
        for session in self.sessions.values_mut() {
            if session.is_terminal() || session.state == AgentSessionState::Stale {
                continue;
            }
            if now.saturating_duration_since(session.last_seen_at) >= self.stale_after {
                session.state = AgentSessionState::Stale;
                session.state_entered_at = now;
                session.push_history(AgentSessionState::Stale, now, MAX_TRANSITIONS_PER_SESSION);
            }
        }
        let terminal_retention = self.terminal_retention;
        let stale_retention = self.stale_retention;
        self.sessions.retain(|_, session| {
            if let Some(terminal_at) = session.terminal_at {
                return now.saturating_duration_since(terminal_at) < terminal_retention;
            }
            if session.state == AgentSessionState::Stale {
                return now.saturating_duration_since(session.state_entered_at) < stale_retention;
            }
            true
        });
    }

    /// The Agent Board ordering (spec §2.2): urgency class, then
    /// state-entered oldest first, then first-seen oldest first, then
    /// key lexical tie-break (`AgentSessionKey`'s derived `Ord`).
    ///
    /// Called from `agents::board::AgentBoardPublisher::publish_if_changed`
    /// (ticket 136), the live `agent-state` IPC publish path.
    pub fn ordered_states(&self, now: Instant) -> Vec<AgentState> {
        let mut sessions: Vec<&AgentSession> = self.sessions.values().collect();
        sessions.sort_by(|a, b| {
            a.state
                .urgency_rank()
                .cmp(&b.state.urgency_rank())
                .then_with(|| a.state_entered_at.cmp(&b.state_entered_at))
                .then_with(|| a.first_seen_at.cmp(&b.first_seen_at))
                .then_with(|| a.key.cmp(&b.key))
        });
        sessions
            .into_iter()
            .map(|s| s.to_state(now, self.terminal_retention))
            .collect()
    }
}

/// Cheaply-cloned handle to an [`AgentRegistry`], living behind the same
/// application-state boundary as `Engine` (`engine.rs`'s own
/// `Arc<tokio::sync::Mutex<SingleSlotQueue>>` shape, mirrored here —
/// see that type's doc for why: by-value construction once, then only
/// clones of the handle cross module/task boundaries). Ticket 134 wires
/// this into `http::AppState` and constructs it once in `lib.rs`'s
/// `setup` closure, next to the Engine.
#[derive(Clone)]
pub struct AgentRegistryHandle(Arc<tokio::sync::Mutex<AgentRegistry>>);

impl AgentRegistryHandle {
    pub fn new(registry: AgentRegistry) -> Self {
        Self(Arc::new(tokio::sync::Mutex::new(registry)))
    }

    /// See [`AgentRegistry::apply_event`].
    pub async fn apply_event(&self, event: AgentEvent, now: Instant) -> ApplyOutcome {
        self.0.lock().await.apply_event(event, now)
    }

    /// The session's current state, if it still exists in the live
    /// registry — used by the `/agent/events` handler (`http.rs`) to
    /// populate the §10 `agent.state` log field after `apply_event`.
    pub async fn state_for(
        &self,
        key: &AgentSessionKey,
        now: Instant,
    ) -> Option<AgentSessionState> {
        let _ = now; // reserved: no time-derived read needed today, kept for symmetry with the other handle methods.
        self.0.lock().await.get(key).map(|s| s.state)
    }

    /// See [`AgentRegistry::tick`].
    pub async fn tick(&self, now: Instant) {
        self.0.lock().await.tick(now);
    }

    /// See [`AgentRegistry::ordered_states`].
    pub async fn ordered_states(&self, now: Instant) -> Vec<AgentState> {
        self.0.lock().await.ordered_states(now)
    }

    #[cfg(test)]
    pub async fn session_count(&self) -> usize {
        self.0.lock().await.session_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::model::AgentRuntime;

    fn key(runtime: AgentRuntime, id: &str) -> AgentSessionKey {
        AgentSessionKey::new(runtime, id).unwrap()
    }

    fn event(
        session_key: AgentSessionKey,
        event_id: &str,
        kind: AgentEventKind,
        terminal: bool,
    ) -> AgentEvent {
        AgentEvent {
            event_id: event_id.to_string(),
            session_key,
            sequence: None,
            kind,
            declared_state: AgentSessionState::Starting,
            terminal,
            capabilities: Vec::new(),
            summary: None,
            details: Vec::new(),
            project: None,
            host: None,
            subagent: None,
        }
    }

    /// Same as [`event`], but lets a test state what the adapter
    /// declared — the field that tells a real SessionStart apart from a
    /// mid-session informational event.
    fn event_declaring(
        session_key: AgentSessionKey,
        event_id: &str,
        kind: AgentEventKind,
        declared_state: AgentSessionState,
    ) -> AgentEvent {
        AgentEvent {
            declared_state,
            ..event(session_key, event_id, kind, false)
        }
    }

    fn registry() -> AgentRegistry {
        AgentRegistry::new(
            Duration::from_secs(300),
            DEFAULT_TERMINAL_RETENTION,
            DEFAULT_STALE_RETENTION,
        )
    }

    // --- session-start disambiguation (2026-07-28 regression) --------

    #[test]
    fn a_mid_session_informational_on_an_unseen_session_becomes_working() {
        // notchtap restarting while an agent session is already running
        // makes that session's next ordinary event the first one this
        // registry has ever seen. It is `Informational` and non-terminal,
        // exactly like a real SessionStart — but the adapter declared
        // `working`, so it must NOT be mistaken for a session start.
        // Before this fix every live session sat at `Starting` after
        // every restart, elapsed timer ticking, summary saying otherwise.
        let mut registry = registry();
        let k = key(AgentRuntime::ClaudeCode, "restart-mid-session");
        let outcome = registry.apply_event(
            event_declaring(
                k.clone(),
                "e1",
                AgentEventKind::Informational,
                AgentSessionState::Working,
            ),
            Instant::now(),
        );
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(registry.get(&k).unwrap().state, AgentSessionState::Working);
    }

    #[test]
    fn a_declared_session_start_keeps_the_starting_baseline() {
        // The other half of the same rule: a genuine SessionStart (the
        // adapters send `state: "starting"`) still parks at `Starting`
        // instead of jumping straight to `Working`.
        let mut registry = registry();
        let k = key(AgentRuntime::ClaudeCode, "genuine-start");
        registry.apply_event(
            event_declaring(
                k.clone(),
                "e1",
                AgentEventKind::Informational,
                AgentSessionState::Starting,
            ),
            Instant::now(),
        );
        assert_eq!(registry.get(&k).unwrap().state, AgentSessionState::Starting);
    }

    // --- §2.1 transition rules -------------------------------------

    #[test]
    fn session_start_then_work_event_reaches_working() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::Informational, false),
            now,
        );
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Starting);
        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Informational, false),
            now,
        );
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);
    }

    #[test]
    fn permission_event_waits_for_permission() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::PermissionRequested, false),
            now,
        );
        assert_eq!(
            reg.get(&k).unwrap().state,
            AgentSessionState::WaitingForPermission
        );
    }

    #[test]
    fn input_required_event_waits_for_input() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::InputRequired, false),
            now,
        );
        assert_eq!(
            reg.get(&k).unwrap().state,
            AgentSessionState::WaitingForInput
        );
    }

    #[test]
    fn work_event_clears_waiting_for_permission() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::PermissionRequested, false),
            now,
        );
        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Informational, false),
            now,
        );
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);
    }

    #[test]
    fn work_event_clears_waiting_for_input() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::InputRequired, false),
            now,
        );
        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Informational, false),
            now,
        );
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);
    }

    #[test]
    fn completed_event_terminates_session() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(event(k.clone(), "e1", AgentEventKind::Completed, true), now);
        let s = reg.get(&k).unwrap();
        assert_eq!(s.state, AgentSessionState::Completed);
        assert!(s.is_terminal());
        assert_eq!(s.terminal_at, Some(now));
    }

    #[test]
    fn non_terminal_completed_event_stays_live_waiting_for_input() {
        // Operator decision 2026-07-26 (spec §2.1): a per-turn provider
        // `Stop`/OpenCode `session.idle` posts kind Completed with
        // terminal:false. The session must NOT go terminal — it stays
        // live, transitioning to WaitingForInput.
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        let outcome = reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::Completed, false),
            now,
        );
        assert_eq!(outcome, ApplyOutcome::Applied);
        let s = reg.get(&k).unwrap();
        assert_eq!(s.state, AgentSessionState::WaitingForInput);
        assert!(!s.is_terminal());
        assert_eq!(s.terminal_at, None);
    }

    #[test]
    fn multi_turn_session_cycles_through_one_key_no_suffixing() {
        // Stop -> PreToolUse/PostToolUse (Working) -> Stop again, all
        // under the SAME session key, proving a per-turn Stop never
        // fragments one session into suffixed terminal rows.
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");

        // Session starts, does some work.
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::Informational, false),
            now,
        );
        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Informational, false),
            now + Duration::from_secs(1),
        );
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);

        // First per-turn Stop: non-terminal Completed -> WaitingForInput.
        reg.apply_event(
            event(k.clone(), "e3", AgentEventKind::Completed, false),
            now + Duration::from_secs(2),
        );
        assert_eq!(
            reg.get(&k).unwrap().state,
            AgentSessionState::WaitingForInput
        );

        // User sends another turn: tool events resume -> Working.
        reg.apply_event(
            event(k.clone(), "e4", AgentEventKind::Informational, false),
            now + Duration::from_secs(3),
        );
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);

        // Second per-turn Stop -> WaitingForInput again, same key.
        let outcome = reg.apply_event(
            event(k.clone(), "e5", AgentEventKind::Completed, false),
            now + Duration::from_secs(4),
        );
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(
            reg.get(&k).unwrap().state,
            AgentSessionState::WaitingForInput
        );
        assert_eq!(
            reg.session_count(),
            1,
            "one session key throughout, no suffixed reuse keys"
        );
        assert!(!reg.get(&k).unwrap().is_terminal());

        // Explicit session end IS terminal.
        reg.apply_event(
            event(k.clone(), "e6", AgentEventKind::Completed, true),
            now + Duration::from_secs(5),
        );
        let s = reg.get(&k).unwrap();
        assert_eq!(s.state, AgentSessionState::Completed);
        assert!(s.is_terminal());
        assert_eq!(reg.session_count(), 1);
    }

    #[test]
    fn terminal_failure_event_fails_session() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(event(k.clone(), "e1", AgentEventKind::Failed, true), now);
        let s = reg.get(&k).unwrap();
        assert_eq!(s.state, AgentSessionState::Failed);
        assert!(s.is_terminal());
    }

    #[test]
    fn non_terminal_tool_failure_leaves_session_working() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        // Get to Working first.
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::Informational, false),
            now,
        );
        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Informational, false),
            now,
        );
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);
        // A non-terminal tool failure keeps it Working.
        let outcome = reg.apply_event(event(k.clone(), "e3", AgentEventKind::Failed, false), now);
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);
    }

    #[test]
    fn stale_after_secs_marks_non_terminal_sessions_stale() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::Informational, false),
            now,
        );
        reg.tick(now + Duration::from_secs(299));
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Starting);
        reg.tick(now + Duration::from_secs(300));
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Stale);
    }

    #[test]
    fn stale_applies_to_waiting_sessions_too() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::PermissionRequested, false),
            now,
        );
        reg.tick(now + Duration::from_secs(300));
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Stale);
    }

    #[test]
    fn new_event_revives_a_stale_session() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::Informational, false),
            now,
        );
        reg.tick(now + Duration::from_secs(300));
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Stale);
        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Informational, false),
            now + Duration::from_secs(301),
        );
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);
    }

    #[test]
    fn terminal_sessions_are_purged_after_retention() {
        let mut reg = AgentRegistry::new(
            Duration::from_secs(300),
            Duration::from_secs(600),
            DEFAULT_STALE_RETENTION,
        );
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(event(k.clone(), "e1", AgentEventKind::Completed, true), now);
        assert_eq!(reg.session_count(), 1);
        reg.tick(now + Duration::from_secs(599));
        assert_eq!(reg.session_count(), 1);
        reg.tick(now + Duration::from_secs(600));
        assert_eq!(reg.session_count(), 0);
    }

    #[test]
    fn stale_sessions_are_purged_after_stale_retention() {
        // stale_after=300, stale_retention=600: a session goes Stale at
        // t=300 (no events since t=0) and must be purged once it has
        // been Stale for 600s, i.e. at t=900 — not before.
        let mut reg = AgentRegistry::new(
            Duration::from_secs(300),
            DEFAULT_TERMINAL_RETENTION,
            Duration::from_secs(600),
        );
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::Informational, false),
            now,
        );
        reg.tick(now + Duration::from_secs(300));
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Stale);
        reg.tick(now + Duration::from_secs(899));
        assert_eq!(reg.session_count(), 1);
        reg.tick(now + Duration::from_secs(900));
        assert_eq!(reg.session_count(), 0);
    }

    #[test]
    fn live_non_stale_sessions_are_never_purged_by_stale_retention() {
        // stale_after=300 > stale_retention=100: were `stale_retention`
        // mistakenly applied to every non-terminal session rather than
        // only ones already `Stale`, this session would be purged well
        // before it ever goes stale. It must survive untouched.
        let mut reg = AgentRegistry::new(
            Duration::from_secs(300),
            DEFAULT_TERMINAL_RETENTION,
            Duration::from_secs(100),
        );
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::Informational, false),
            now,
        );
        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Informational, false),
            now,
        );
        reg.tick(now + Duration::from_secs(299));
        assert_eq!(reg.session_count(), 1);
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);
    }

    #[test]
    fn terminal_state_never_reactivates_for_the_same_key() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(event(k.clone(), "e1", AgentEventKind::Completed, true), now);
        // A further genuinely-new event under the SAME native session id
        // must not reactivate the terminal session.
        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Informational, false),
            now + Duration::from_secs(1),
        );
        let original = reg.get(&k).unwrap();
        assert_eq!(original.state, AgentSessionState::Completed);
        assert_eq!(
            original.history.len(),
            2,
            "original history untouched beyond its own transitions"
        );
    }

    #[test]
    fn reused_terminal_id_gets_a_suffixed_fallback_key() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(event(k.clone(), "e1", AgentEventKind::Completed, true), now);
        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Informational, false),
            now + Duration::from_secs(1),
        );
        let fallback = k.suffixed(1);
        let revived = reg
            .get(&fallback)
            .expect("suffixed fallback session must exist");
        assert_eq!(revived.state, AgentSessionState::Starting);
        assert_eq!(reg.session_count(), 2);
    }

    #[test]
    fn repeated_terminal_reuse_produces_distinct_suffixed_keys() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(event(k.clone(), "e1", AgentEventKind::Completed, true), now);
        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Completed, true),
            now + Duration::from_secs(1),
        );
        reg.apply_event(
            event(k.clone(), "e3", AgentEventKind::Informational, false),
            now + Duration::from_secs(2),
        );
        assert_eq!(reg.session_count(), 3);
        assert!(reg.get(&k.suffixed(1)).is_some());
        assert!(reg.get(&k.suffixed(2)).is_some());
    }

    // --- §2.2 ordering ------------------------------------------------

    #[test]
    fn ordering_is_urgency_then_fifo_within_class() {
        let mut reg = registry();
        let now = Instant::now();
        let working = key(AgentRuntime::Codex, "working");
        let waiting_perm_older = key(AgentRuntime::Codex, "waiting-perm-older");
        let waiting_perm_newer = key(AgentRuntime::Codex, "waiting-perm-newer");
        let failed = key(AgentRuntime::Codex, "failed");

        reg.apply_event(
            event(working.clone(), "e1", AgentEventKind::Informational, false),
            now,
        );
        reg.apply_event(
            event(working.clone(), "e1b", AgentEventKind::Informational, false),
            now,
        );
        reg.apply_event(
            event(
                waiting_perm_older.clone(),
                "e2",
                AgentEventKind::PermissionRequested,
                false,
            ),
            now + Duration::from_secs(1),
        );
        reg.apply_event(
            event(
                waiting_perm_newer.clone(),
                "e3",
                AgentEventKind::PermissionRequested,
                false,
            ),
            now + Duration::from_secs(2),
        );
        reg.apply_event(
            event(failed.clone(), "e4", AgentEventKind::Failed, true),
            now + Duration::from_secs(3),
        );

        let ordered: Vec<AgentSessionKey> = reg
            .ordered_states(now + Duration::from_secs(10))
            .into_iter()
            .map(|s| s.key)
            .collect();
        assert_eq!(
            ordered,
            vec![waiting_perm_older, waiting_perm_newer, failed, working]
        );
    }

    #[test]
    fn state_change_re_enqueues_into_destination_urgency_class() {
        let mut reg = registry();
        let now = Instant::now();
        let a = key(AgentRuntime::Codex, "a");
        let b = key(AgentRuntime::Codex, "b");

        // `a` starts waiting-for-permission (higher urgency) before `b`
        // even exists.
        reg.apply_event(
            event(a.clone(), "e1", AgentEventKind::PermissionRequested, false),
            now,
        );
        reg.apply_event(
            event(b.clone(), "e2", AgentEventKind::PermissionRequested, false),
            now + Duration::from_secs(1),
        );
        let ordered_before: Vec<AgentSessionKey> = reg
            .ordered_states(now + Duration::from_secs(2))
            .into_iter()
            .map(|s| s.key)
            .collect();
        assert_eq!(ordered_before, vec![a.clone(), b.clone()]);

        // `a` clears to Working (lower urgency class) at a later time —
        // it must now sort AFTER `b`, in the Working class, keyed by its
        // own new state-entered time.
        reg.apply_event(
            event(a.clone(), "e3", AgentEventKind::Informational, false),
            now + Duration::from_secs(5),
        );
        let ordered_after: Vec<AgentSessionKey> = reg
            .ordered_states(now + Duration::from_secs(6))
            .into_iter()
            .map(|s| s.key)
            .collect();
        assert_eq!(ordered_after, vec![b, a]);
    }

    #[test]
    fn ordering_tie_breaks_on_key_lexical_order() {
        let mut reg = registry();
        let now = Instant::now();
        let a = key(AgentRuntime::Codex, "aaa");
        let b = key(AgentRuntime::Codex, "bbb");
        // Same kind, same instant — first_seen/state_entered tie too.
        reg.apply_event(
            event(b.clone(), "e1", AgentEventKind::Informational, false),
            now,
        );
        reg.apply_event(
            event(a.clone(), "e2", AgentEventKind::Informational, false),
            now,
        );
        let ordered: Vec<AgentSessionKey> =
            reg.ordered_states(now).into_iter().map(|s| s.key).collect();
        assert_eq!(ordered, vec![a, b]);
    }

    // --- identity -------------------------------------------------

    #[test]
    fn two_sessions_sharing_runtime_and_project_never_merge() {
        let mut reg = registry();
        let now = Instant::now();
        let k1 = key(AgentRuntime::Codex, "native-1");
        let k2 = key(AgentRuntime::Codex, "native-2");
        let mut e1 = event(k1.clone(), "e1", AgentEventKind::Informational, false);
        e1.project = Some(AgentProject {
            name: Some("notchtap".to_string()),
            cwd: Some("/repo".to_string()),
        });
        let mut e2 = event(k2.clone(), "e2", AgentEventKind::Informational, false);
        e2.project = e1.project.clone();
        reg.apply_event(e1, now);
        reg.apply_event(e2, now);
        assert_eq!(
            reg.session_count(),
            2,
            "identical project metadata must not merge distinct native ids"
        );
    }

    #[test]
    fn duplicate_event_id_produces_no_state_change() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::Informational, false),
            now,
        );
        reg.apply_event(
            event(k.clone(), "e1b", AgentEventKind::Informational, false),
            now,
        );
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);

        // Re-deliver "e1" (already seen) with a kind that WOULD change
        // state if accepted — it must be a no-op.
        let outcome = reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::PermissionRequested, false),
            now,
        );
        assert_eq!(outcome, ApplyOutcome::DuplicateEventId);
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);
    }

    #[test]
    fn stale_sequence_produces_no_state_change() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        let mut e1 = event(k.clone(), "e1", AgentEventKind::Informational, false);
        e1.sequence = Some(5);
        reg.apply_event(e1, now);

        let mut e2 = event(k.clone(), "e2", AgentEventKind::PermissionRequested, false);
        e2.sequence = Some(5); // equal, not greater -> stale
        let outcome = reg.apply_event(e2, now);
        assert_eq!(outcome, ApplyOutcome::StaleSequence);
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Starting);

        let mut e3 = event(k.clone(), "e3", AgentEventKind::PermissionRequested, false);
        e3.sequence = Some(4); // lower -> stale
        let outcome = reg.apply_event(e3, now);
        assert_eq!(outcome, ApplyOutcome::StaleSequence);
    }

    #[test]
    fn higher_sequence_is_accepted() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        let mut e1 = event(k.clone(), "e1", AgentEventKind::Informational, false);
        e1.sequence = Some(5);
        reg.apply_event(e1, now);

        let mut e2 = event(k.clone(), "e2", AgentEventKind::PermissionRequested, false);
        e2.sequence = Some(6);
        let outcome = reg.apply_event(e2, now);
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(
            reg.get(&k).unwrap().state,
            AgentSessionState::WaitingForPermission
        );
    }

    // --- caps -------------------------------------------------------

    #[test]
    fn remembered_event_ids_are_bounded_lru() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        for i in 0..(MAX_REMEMBERED_EVENT_IDS + 10) {
            reg.apply_event(
                event(
                    k.clone(),
                    &format!("e{i}"),
                    AgentEventKind::Informational,
                    false,
                ),
                now,
            );
        }
        assert_eq!(reg.seen_event_id_order.len(), MAX_REMEMBERED_EVENT_IDS);
        // The oldest ids should have been evicted: re-delivering "e0"
        // (evicted) must be accepted again (not treated as duplicate).
        let outcome = reg.apply_event(
            event(k.clone(), "e0", AgentEventKind::Informational, false),
            now,
        );
        assert_eq!(outcome, ApplyOutcome::Applied);
    }

    #[test]
    fn transition_history_is_bounded_to_fifty() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        // Alternate two kinds to force a state change on every event.
        for i in 0..80u64 {
            let kind = if i % 2 == 0 {
                AgentEventKind::PermissionRequested
            } else {
                AgentEventKind::Informational
            };
            reg.apply_event(
                event(k.clone(), &format!("e{i}"), kind, false),
                now + Duration::from_secs(i),
            );
        }
        assert_eq!(
            reg.get(&k).unwrap().history.len(),
            MAX_TRANSITIONS_PER_SESSION
        );
    }

    // --- metadata merge policy ---------------------------------------

    #[test]
    fn project_metadata_persists_when_a_later_event_omits_it() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        let mut e1 = event(k.clone(), "e1", AgentEventKind::Informational, false);
        e1.project = Some(AgentProject {
            name: Some("notchtap".to_string()),
            cwd: None,
        });
        reg.apply_event(e1, now);
        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Informational, false),
            now,
        );
        assert_eq!(
            reg.get(&k)
                .unwrap()
                .project
                .as_ref()
                .unwrap()
                .name
                .as_deref(),
            Some("notchtap")
        );
    }

    #[test]
    fn summary_is_replaced_including_being_cleared() {
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        let mut e1 = event(k.clone(), "e1", AgentEventKind::Informational, false);
        e1.summary = Some("first".to_string());
        reg.apply_event(e1, now);
        assert_eq!(reg.get(&k).unwrap().summary.as_deref(), Some("first"));

        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Informational, false),
            now,
        );
        assert_eq!(reg.get(&k).unwrap().summary, None);
    }

    // --- AgentRegistryHandle (ticket 134) ----------------------------

    #[tokio::test]
    async fn handle_apply_event_and_state_for_round_trip() {
        let handle = AgentRegistryHandle::new(registry());
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        let outcome = handle
            .apply_event(
                event(k.clone(), "e1", AgentEventKind::PermissionRequested, false),
                now,
            )
            .await;
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(
            handle.state_for(&k, now).await,
            Some(AgentSessionState::WaitingForPermission)
        );
    }

    #[tokio::test]
    async fn handle_duplicate_event_id_is_a_zero_mutation_no_op() {
        let handle = AgentRegistryHandle::new(registry());
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        handle
            .apply_event(
                event(k.clone(), "e1", AgentEventKind::Informational, false),
                now,
            )
            .await;
        assert_eq!(handle.session_count().await, 1);
        let before = handle.state_for(&k, now).await;

        let outcome = handle
            .apply_event(
                event(k.clone(), "e1", AgentEventKind::PermissionRequested, false),
                now,
            )
            .await;
        assert_eq!(outcome, ApplyOutcome::DuplicateEventId);
        assert_eq!(handle.state_for(&k, now).await, before);
        assert_eq!(handle.session_count().await, 1);
    }
}
