//! Provider-neutral mutable Agent session model and wire-facing snapshots.

use std::time::{Duration, Instant};

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgentRuntime {
    ClaudeCode,
    Codex,
    Kimi,
    OpenCode,
}

/// The UI renders only declared+observed capabilities; this type has no "unknown" variant on
/// purpose — an absent capability is simply not in the `Vec`, never a heuristic guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentCapability {
    SessionLifecycle,
    PermissionRequests,
    InputRequired,
    Completion,
    Failure,
    ToolDetails,
    Subagents,
    OpenOrFocus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentEventKind {
    PermissionRequested,
    InputRequired,
    Completed,
    Failed,
    Informational,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSessionState {
    Starting,
    Working,
    WaitingForPermission,
    WaitingForInput,
    Completed,
    Failed,
    Stale,
}

impl AgentSessionState {
    /// Terminal states (`Completed`, `Failed`) never transition back to active for the same key —
    /// see `AgentRegistry::apply_event`'s terminal-reuse branch.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            AgentSessionState::Completed | AgentSessionState::Failed
        )
    }

    /// Whether a session in this state is on its own reason enough to SUMMON the Agent Board.
    /// Agents that are merely working must not summon it — the Board's job is attention.
    pub fn summons_board(self) -> bool {
        match self {
            AgentSessionState::WaitingForPermission
            | AgentSessionState::WaitingForInput
            | AgentSessionState::Failed
            | AgentSessionState::Completed => true,
            AgentSessionState::Starting | AgentSessionState::Working | AgentSessionState::Stale => {
                false
            }
        }
    }

    /// Urgency class rank used by the ordering key: `WaitingForPermission`, `WaitingForInput`,
    /// `Failed`, `Completed`, `Stale`, `Working`, `Starting`.
    pub fn urgency_rank(self) -> u8 {
        match self {
            AgentSessionState::WaitingForPermission => 0,
            AgentSessionState::WaitingForInput => 1,
            AgentSessionState::Failed => 2,
            AgentSessionState::Completed => 3,
            AgentSessionState::Stale => 4,
            AgentSessionState::Working => 5,
            AgentSessionState::Starting => 6,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelError {
    #[error("agent session key's native_session_id must not be empty")]
    EmptyNativeSessionId,
}

/// A provider without a native session id must use an adapter-made fallback of process identity
/// plus start timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgentSessionKey {
    pub runtime: AgentRuntime,
    pub native_session_id: String,
}

impl AgentSessionKey {
    pub fn new(
        runtime: AgentRuntime,
        native_session_id: impl Into<String>,
    ) -> Result<Self, ModelError> {
        let native_session_id = native_session_id.into();
        if native_session_id.trim().is_empty() {
            return Err(ModelError::EmptyNativeSessionId);
        }
        Ok(Self {
            runtime,
            native_session_id,
        })
    }

    /// Builds the suffixed fallback key used when a provider incorrectly reuses a terminal
    /// session's native id, keeping the terminal-never-reactivates rule intact.
    pub fn suffixed(&self, generation: u32) -> AgentSessionKey {
        AgentSessionKey {
            runtime: self.runtime,
            native_session_id: format!("{}#reuse{generation}", self.native_session_id),
        }
    }
}

/// Produces the process-local identifier used instead of exposing native session IDs.
pub fn session_hash_hex(key: &AgentSessionKey) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDetail {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentProject {
    pub name: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentHost {
    pub name: Option<String>,
    pub bundle_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSubagentSummary {
    pub id: String,
    pub label: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentTransition {
    pub state: AgentSessionState,
    pub entered_at: Instant,
}

/// Clock-agnostic like `queue.rs`'s items: every method that needs "now" takes it as a parameter,
/// no wall-clock read happens inside this module — tests pass a simulated clock.
#[derive(Debug, Clone)]
pub struct AgentSession {
    pub key: AgentSessionKey,
    pub state: AgentSessionState,
    pub capabilities: Vec<AgentCapability>,
    pub summary: Option<String>,
    pub details: Vec<AgentDetail>,
    pub project: Option<AgentProject>,
    pub host: Option<AgentHost>,
    pub subagent: Option<AgentSubagentSummary>,
    pub first_seen_at: Instant,
    pub state_entered_at: Instant,
    pub last_seen_at: Instant,
    pub terminal_at: Option<Instant>,
    pub last_accepted_sequence: Option<u64>,
    pub history: Vec<AgentTransition>,
}

impl AgentSession {
    pub fn new(key: AgentSessionKey, now: Instant) -> Self {
        Self {
            key,
            state: AgentSessionState::Starting,
            capabilities: Vec::new(),
            summary: None,
            details: Vec::new(),
            project: None,
            host: None,
            subagent: None,
            first_seen_at: now,
            state_entered_at: now,
            last_seen_at: now,
            terminal_at: None,
            last_accepted_sequence: None,
            history: vec![AgentTransition {
                state: AgentSessionState::Starting,
                entered_at: now,
            }],
        }
    }

    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    /// Appends a transition, evicting the oldest entry once the history exceeds `cap` (50 retained
    /// transitions per session, per the caps table in `adapter.rs`).
    pub fn push_history(&mut self, state: AgentSessionState, entered_at: Instant, cap: usize) {
        self.history.push(AgentTransition { state, entered_at });
        while self.history.len() > cap {
            self.history.remove(0);
        }
    }

    /// Builds the wire-facing snapshot at `now`.
    pub fn to_state(&self, now: Instant, terminal_retention: Duration) -> AgentState {
        let elapsed_ms = now
            .saturating_duration_since(self.state_entered_at)
            .as_millis() as u64;
        let last_seen_at_ms = now.saturating_duration_since(self.last_seen_at).as_millis() as u64;
        let retention_remaining_ms = self.terminal_at.map(|terminal_at| {
            let since_terminal = now.saturating_duration_since(terminal_at);
            terminal_retention
                .saturating_sub(since_terminal)
                .as_millis() as u64
        });
        AgentState {
            key: self.key.clone(),
            state: self.state,
            capabilities: self.capabilities.clone(),
            summary: self.summary.clone(),
            details: self.details.clone(),
            project: self.project.clone(),
            host: self.host.clone(),
            subagent: self.subagent.clone(),
            history: self.history.clone(),
            first_seen_at: self.first_seen_at,
            state_entered_at: self.state_entered_at,
            last_seen_at_ms,
            elapsed_ms,
            retention_remaining_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentState {
    pub key: AgentSessionKey,
    pub state: AgentSessionState,
    pub capabilities: Vec<AgentCapability>,
    pub summary: Option<String>,
    pub details: Vec<AgentDetail>,
    pub project: Option<AgentProject>,
    pub host: Option<AgentHost>,
    pub subagent: Option<AgentSubagentSummary>,
    pub history: Vec<AgentTransition>,
    pub first_seen_at: Instant,
    pub state_entered_at: Instant,
    pub last_seen_at_ms: u64,
    pub elapsed_ms: u64,
    pub retention_remaining_ms: Option<u64>,
}

impl AgentState {
    /// Dedup-only equality, handwritten — NEVER the derived `PartialEq` above, which stays intact
    /// and honest for tests that want full structural equality.
    pub fn dedup_eq(&self, other: &AgentState) -> bool {
        fn normalized(s: &AgentState) -> AgentState {
            let mut s = s.clone();
            let AgentState {
                key: _,
                state: _,
                capabilities: _,
                summary: _,
                details: _,
                project: _,
                host: _,
                subagent: _,
                history: _,
                first_seen_at: _,
                state_entered_at: _,
                last_seen_at_ms,
                elapsed_ms,
                retention_remaining_ms,
            } = &mut s;
            *last_seen_at_ms = 0;
            *elapsed_ms = 0;
            *retention_remaining_ms = retention_remaining_ms.map(|_| 0);
            s
        }
        normalized(self) == normalized(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_session_key_rejects_empty_native_id() {
        let err = AgentSessionKey::new(AgentRuntime::Codex, "   ").unwrap_err();
        assert_eq!(err, ModelError::EmptyNativeSessionId);
    }

    #[test]
    fn agent_session_key_accepts_nonempty_native_id() {
        let key = AgentSessionKey::new(AgentRuntime::Codex, "sess-1").unwrap();
        assert_eq!(key.native_session_id, "sess-1");
    }

    #[test]
    fn suffixed_key_is_distinct_and_stable() {
        let key = AgentSessionKey::new(AgentRuntime::ClaudeCode, "sess-1").unwrap();
        let s1 = key.suffixed(1);
        let s2 = key.suffixed(2);
        assert_ne!(s1, key);
        assert_ne!(s1, s2);
        assert_eq!(s1.native_session_id, "sess-1#reuse1");
    }

    #[test]
    fn urgency_rank_ascends_from_waiting_for_permission_to_starting() {
        use AgentSessionState::*;
        let ranks = [
            WaitingForPermission,
            WaitingForInput,
            Failed,
            Completed,
            Stale,
            Working,
            Starting,
        ]
        .map(|s| s.urgency_rank());
        let mut sorted = ranks;
        sorted.sort_unstable();
        assert_eq!(
            ranks, sorted,
            "the declared state order must already be rank-ascending"
        );
    }

    #[test]
    fn every_summoning_state_outranks_every_non_summoning_state() {
        use AgentSessionState::*;
        let all = [
            WaitingForPermission,
            WaitingForInput,
            Failed,
            Completed,
            Stale,
            Working,
            Starting,
        ];
        for summoning in all.iter().copied().filter(|s| s.summons_board()) {
            for quiet in all.iter().copied().filter(|s| !s.summons_board()) {
                assert!(
                    summoning.urgency_rank() < quiet.urgency_rank(),
                    "{summoning:?} summons the Board and must outrank {quiet:?}, which does not"
                );
            }
        }
    }

    #[test]
    fn only_attention_states_summon_the_board() {
        use AgentSessionState::*;
        for s in [WaitingForPermission, WaitingForInput, Failed, Completed] {
            assert!(s.summons_board(), "{s:?} must summon the Agent Board");
        }
        for s in [Starting, Working, Stale] {
            assert!(
                !s.summons_board(),
                "{s:?} is not a request for attention and must not summon the Agent Board"
            );
        }
    }

    #[test]
    fn only_completed_and_failed_are_terminal() {
        use AgentSessionState::*;
        for s in [
            Starting,
            Working,
            WaitingForPermission,
            WaitingForInput,
            Stale,
        ] {
            assert!(!s.is_terminal(), "{s:?} must not be terminal");
        }
        for s in [Completed, Failed] {
            assert!(s.is_terminal(), "{s:?} must be terminal");
        }
    }

    #[test]
    fn push_history_evicts_oldest_past_cap() {
        let key = AgentSessionKey::new(AgentRuntime::Kimi, "s").unwrap();
        let now = Instant::now();
        let mut session = AgentSession::new(key, now);
        for i in 0..60u32 {
            session.push_history(
                AgentSessionState::Working,
                now + Duration::from_secs(i as u64),
                50,
            );
        }
        assert_eq!(session.history.len(), 50);
        assert!(session
            .history
            .iter()
            .all(|t| t.state == AgentSessionState::Working));
    }

    #[test]
    fn dedup_eq_ignores_clock_only_changes() {
        let key = AgentSessionKey::new(AgentRuntime::OpenCode, "s").unwrap();
        let base = Instant::now();
        let session = AgentSession::new(key, base);
        let retention = Duration::from_secs(600);
        let a = session.to_state(base + Duration::from_secs(1), retention);
        let b = session.to_state(base + Duration::from_secs(30), retention);
        assert_ne!(a.last_seen_at_ms, b.last_seen_at_ms);
        assert_ne!(a.elapsed_ms, b.elapsed_ms);
        assert!(
            a.dedup_eq(&b),
            "clock-only differences must dedup_eq as equal"
        );
    }

    #[test]
    fn dedup_eq_treats_retention_remaining_change_as_clock_only() {
        let key = AgentSessionKey::new(AgentRuntime::OpenCode, "s").unwrap();
        let base = Instant::now();
        let mut session = AgentSession::new(key, base);
        session.state = AgentSessionState::Completed;
        session.terminal_at = Some(base);
        let retention = Duration::from_secs(600);
        let a = session.to_state(base + Duration::from_secs(1), retention);
        let b = session.to_state(base + Duration::from_secs(100), retention);
        assert_ne!(a.retention_remaining_ms, b.retention_remaining_ms);
        assert!(a.dedup_eq(&b));
    }

    #[test]
    fn dedup_eq_treats_state_change_as_real_change() {
        let key = AgentSessionKey::new(AgentRuntime::OpenCode, "s").unwrap();
        let base = Instant::now();
        let mut session = AgentSession::new(key, base);
        let retention = Duration::from_secs(600);
        let before = session.to_state(base, retention);
        session.state = AgentSessionState::Working;
        let after = session.to_state(base, retention);
        assert!(!before.dedup_eq(&after));
    }

    #[test]
    fn dedup_eq_treats_summary_change_as_real_change() {
        let key = AgentSessionKey::new(AgentRuntime::OpenCode, "s").unwrap();
        let base = Instant::now();
        let mut session = AgentSession::new(key, base);
        let retention = Duration::from_secs(600);
        let before = session.to_state(base, retention);
        session.summary = Some("changed".to_string());
        let after = session.to_state(base, retention);
        assert!(!before.dedup_eq(&after));
    }

    #[test]
    fn dedup_eq_treats_capabilities_change_as_real_change() {
        let key = AgentSessionKey::new(AgentRuntime::OpenCode, "s").unwrap();
        let base = Instant::now();
        let mut session = AgentSession::new(key, base);
        let retention = Duration::from_secs(600);
        let before = session.to_state(base, retention);
        session.capabilities = vec![AgentCapability::ToolDetails];
        let after = session.to_state(base, retention);
        assert!(!before.dedup_eq(&after));
    }

    #[test]
    fn dedup_eq_treats_ordering_timestamp_change_as_real_change() {
        let key = AgentSessionKey::new(AgentRuntime::OpenCode, "s").unwrap();
        let base = Instant::now();
        let mut session = AgentSession::new(key, base);
        let retention = Duration::from_secs(600);
        let before = session.to_state(base, retention);
        session.state_entered_at = base + Duration::from_secs(5);
        let after = session.to_state(base, retention);
        assert!(!before.dedup_eq(&after));
    }

    #[test]
    fn dedup_eq_treats_metadata_change_as_real_change() {
        let key = AgentSessionKey::new(AgentRuntime::OpenCode, "s").unwrap();
        let base = Instant::now();
        let mut session = AgentSession::new(key, base);
        let retention = Duration::from_secs(600);
        let before = session.to_state(base, retention);
        session.project = Some(AgentProject {
            name: Some("notchtap".to_string()),
            cwd: None,
        });
        let after = session.to_state(base, retention);
        assert!(!before.dedup_eq(&after));
    }

    #[test]
    fn dedup_eq_treats_history_change_as_real_change() {
        let key = AgentSessionKey::new(AgentRuntime::OpenCode, "s").unwrap();
        let base = Instant::now();
        let mut session = AgentSession::new(key, base);
        let retention = Duration::from_secs(600);
        let before = session.to_state(base, retention);
        session.push_history(
            AgentSessionState::Working,
            base + Duration::from_secs(1),
            50,
        );
        let after = session.to_state(base, retention);
        assert!(!before.dedup_eq(&after));
    }
}
