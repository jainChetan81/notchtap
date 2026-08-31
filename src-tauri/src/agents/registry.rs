//! The authoritative in-memory Agent Registry: transition rules, ordering, the dedup contract, the
//! caps table.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::model::{
    AgentCapability, AgentDetail, AgentEventKind, AgentHost, AgentProject, AgentSession,
    AgentSessionKey, AgentSessionState, AgentState, AgentSubagentSummary,
};

/// Retained transitions per session (caps table, `adapter.rs`).
pub const MAX_TRANSITIONS_PER_SESSION: usize = 50;
/// Remembered event ids, LRU (caps table, `adapter.rs`).
pub const MAX_REMEMBERED_EVENT_IDS: usize = 2048;
/// Default `agents.terminal_retention_secs` — see `AgentsConfig`'s own field doc.
pub const DEFAULT_TERMINAL_RETENTION: Duration = Duration::from_secs(60);
/// Default `agents.stale_retention_secs` — see `AgentsConfig`.
pub const DEFAULT_STALE_RETENTION: Duration = Duration::from_secs(600);

#[derive(Debug, Clone)]
pub struct AgentEvent {
    pub event_id: String,
    pub session_key: AgentSessionKey,
    pub sequence: Option<u64>,
    pub kind: AgentEventKind,
    /// Authoritative for terminality: kind `Failed` with `terminal` false is a non-terminal tool
    /// failure (session stays/becomes `Working`).
    pub terminal: bool,
    pub declared_state: AgentSessionState,
    pub capabilities: Vec<AgentCapability>,
    pub summary: Option<String>,
    pub details: Vec<AgentDetail>,
    pub project: Option<AgentProject>,
    pub host: Option<AgentHost>,
    pub subagent: Option<AgentSubagentSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    DuplicateEventId,
    StaleSequence,
}

/// Terminal states are absorbing; non-terminal completion returns to input-waiting.
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
        AgentEventKind::Completed if terminal => AgentSessionState::Completed,
        AgentEventKind::Completed => AgentSessionState::WaitingForInput,
        AgentEventKind::Failed if terminal => AgentSessionState::Failed,
        AgentEventKind::Failed => AgentSessionState::Working,
        AgentEventKind::Informational if terminal => AgentSessionState::Completed,
        AgentEventKind::Informational => AgentSessionState::Working,
    }
}

/// The authoritative in-memory Agent Registry.
pub struct AgentRegistry {
    sessions: HashMap<AgentSessionKey, AgentSession>,
    seen_event_ids: HashSet<String>,
    seen_event_id_order: VecDeque<String>,
    reuse_generations: HashMap<AgentSessionKey, u32>,
    stale_after: Duration,
    terminal_retention: Duration,
    stale_retention: Duration,
}

impl AgentRegistry {
    /// `stale_after`, `terminal_retention`, and `stale_retention` are all injected constructor
    /// parameters, sourced from `agents.stale_after_secs`, `agents.terminal_retention_secs`.
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

    /// Feeds one normalized event into the registry.
    pub fn apply_event(&mut self, event: AgentEvent, now: Instant) -> ApplyOutcome {
        if self.seen_event_ids.contains(&event.event_id) {
            return ApplyOutcome::DuplicateEventId;
        }

        let mut target_key = event.session_key.clone();
        if let Some(existing) = self.sessions.get(&target_key) {
            if existing.is_terminal() {
                // Terminal states never reactivate.
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

    /// Applies stale transitions and retention eviction using the caller-supplied clock.
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

    /// The Agent Board ordering: urgency class, then state-entered oldest first, then first-seen
    /// oldest first, then key lexical tie-break (`AgentSessionKey`'s derived `Ord`).
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

    #[test]
    fn a_mid_session_informational_on_an_unseen_session_becomes_working() {
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
        let mut reg = registry();
        let now = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");

        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::Informational, false),
            now,
        );
        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Informational, false),
            now + Duration::from_secs(1),
        );
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);

        reg.apply_event(
            event(k.clone(), "e3", AgentEventKind::Completed, false),
            now + Duration::from_secs(2),
        );
        assert_eq!(
            reg.get(&k).unwrap().state,
            AgentSessionState::WaitingForInput
        );

        reg.apply_event(
            event(k.clone(), "e4", AgentEventKind::Informational, false),
            now + Duration::from_secs(3),
        );
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);

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
        reg.apply_event(
            event(k.clone(), "e1", AgentEventKind::Informational, false),
            now,
        );
        reg.apply_event(
            event(k.clone(), "e2", AgentEventKind::Informational, false),
            now,
        );
        assert_eq!(reg.get(&k).unwrap().state, AgentSessionState::Working);
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
