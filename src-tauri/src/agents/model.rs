//! Plan 133: the provider-neutral Agent domain model (spec §2).
//!
//! Two shapes live here, mirroring the `queue.rs` / `event.rs` split
//! between the mutable internal item (`QueueItem`) and the wire-facing
//! snapshot (`SlotState`):
//!
//! - [`AgentSession`] is the registry's own mutable, `Instant`-clocked
//!   record of one Agent Session. It is never serialized.
//! - [`AgentState`] is the (future) wire-facing snapshot built from an
//!   `AgentSession` via [`AgentSession::to_state`] — this ticket builds
//!   the type and its handwritten [`AgentState::dedup_eq`] (spec §2.3)
//!   now so later tickets only have to wire emission, not invent the
//!   dedup contract under time pressure.
//!
//! `AgentSessionKey` (runtime + native session id) is the sole registry
//! identity — see its own doc for why metadata never merges sessions.

use std::time::{Duration, Instant};

use thiserror::Error;

/// The four v7 Agent Runtimes (spec §0). Order is declaration order,
/// which also backs `AgentSessionKey`'s derived `Ord` — see that type's
/// doc for why declaration-order stability is good enough for the
/// ordering key's lexical tie-break (spec §2.2 step 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AgentRuntime {
    ClaudeCode,
    Codex,
    Kimi,
    OpenCode,
}

/// A capability an adapter has declared and observed for a session
/// (spec §1's capability matrix, §2's conceptual model). The UI (later
/// tickets) renders only declared+observed capabilities; this type has
/// no "unknown" variant on purpose — an absent capability is simply not
/// in the `Vec`, never a heuristic guess.
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

/// The wire-facing event kind (spec §3.1's `kind` field has exactly
/// these five string values). This is intentionally coarser than the
/// registry's internal transition logic needs — see
/// `registry::next_state`'s doc for how `AgentEventKind` plus the
/// sibling `terminal` flag together drive every §2.1 transition rule,
/// including the ones (session start, generic tool/work progress) that
/// don't get a dedicated wire `kind` value of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentEventKind {
    PermissionRequested,
    InputRequired,
    Completed,
    Failed,
    Informational,
}

/// The seven Agent Session states (spec §2's conceptual model / §2.1's
/// transition rules / §2.2's urgency ordering).
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
    /// Terminal states (`Completed`, `Failed`) never transition back to
    /// active for the same key (spec §2.1) — see
    /// `AgentRegistry::apply_event`'s terminal-reuse branch.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            AgentSessionState::Completed | AgentSessionState::Failed
        )
    }

    /// Urgency class rank used by the ordering key (spec §2.2 step 1):
    /// `WaitingForPermission`, `WaitingForInput`, `Failed`, `Stale`,
    /// `Working`, `Starting`, `Completed`, most urgent first (lowest
    /// rank sorts first).
    ///
    /// Called from `AgentRegistry::ordered_states`, which ticket 136
    /// (agent-state IPC, `agents/board.rs`) wires into the live
    /// `agent-state` publish path.
    pub fn urgency_rank(self) -> u8 {
        match self {
            AgentSessionState::WaitingForPermission => 0,
            AgentSessionState::WaitingForInput => 1,
            AgentSessionState::Failed => 2,
            AgentSessionState::Stale => 3,
            AgentSessionState::Working => 4,
            AgentSessionState::Starting => 5,
            AgentSessionState::Completed => 6,
        }
    }
}

/// Errors constructing domain-model values. Library-module rule
/// (CLAUDE.md): `thiserror`, matchable variants.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelError {
    #[error("agent session key's native_session_id must not be empty")]
    EmptyNativeSessionId,
}

/// The sole registry identity (spec §2). Project path, project name,
/// Host, and display title are mutable metadata on `AgentSession` and
/// must never be used to merge two sessions — only `(runtime,
/// native_session_id)` equality does that. A provider without a native
/// session id must use an adapter-made fallback of process identity
/// plus start timestamp (ticket 134); project path alone is forbidden
/// as a fallback because two unrelated sessions in the same project
/// would collide.
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

    /// Builds the suffixed fallback key used when a provider incorrectly
    /// reuses a terminal session's native id (spec §2.1's last
    /// paragraph, plan 133's "terminal-never-reactivates" requirement).
    /// `generation` is a 1-based reuse counter so repeated collisions on
    /// the same original id keep producing distinct keys
    /// (`...#reuse1`, `...#reuse2`, ...) rather than colliding with each
    /// other.
    pub fn suffixed(&self, generation: u32) -> AgentSessionKey {
        AgentSessionKey {
            runtime: self.runtime,
            native_session_id: format!("{}#reuse{generation}", self.native_session_id),
        }
    }
}

/// A stable, process-local (not cryptographically secret — just
/// non-reversible-in-a-log-line-or-history-file) hash of an
/// [`AgentSessionKey`]. Shared by every place that must refer to a
/// session without ever surfacing its raw `native_session_id`: the §10
/// `agent.session_hash` structured log field (`http.rs`'s
/// `agent_events_handler`, plan 134) and `AgentSignal.session_hash`
/// (`event.rs`, plan 135) — see that struct's doc for why persisting the
/// raw id would violate spec §9. `DefaultHasher::new()` uses a fixed
/// SipHash key (unlike `HashMap`'s own `RandomState`), so the same
/// session hashes identically across every event in this process.
pub fn session_hash_hex(key: &AgentSessionKey) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// One `{label, value}` detail cell (mirrors `event.rs::DetailItem`'s
/// shape but is defined independently here — this module deliberately
/// has no dependency on `event.rs` yet; ticket 135 is what wires the
/// two together).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDetail {
    pub label: String,
    pub value: String,
}

/// Optional project metadata (spec §3.1's `project` object). Persists
/// across events that don't repeat it — see
/// `AgentRegistry::apply_event`'s metadata-merge comment for why this
/// differs from `summary`/`details`, which are always replaced with the
/// latest event's value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentProject {
    pub name: Option<String>,
    pub cwd: Option<String>,
}

/// Optional Host metadata (spec §3.1's `host` object; spec §0's
/// Host-dependent Open/Focus Session action, later tickets). Same
/// merge-if-present persistence as `AgentProject`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentHost {
    pub name: Option<String>,
    pub bundle_id: Option<String>,
}

/// A session's own subagent summary (spec §3.1's `subagent` object).
/// Always replaced with the latest event's value — a session's
/// subagent state is a point-in-time fact, not an accumulating one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSubagentSummary {
    pub id: String,
    pub label: Option<String>,
    pub state: Option<String>,
}

/// One entry in a session's bounded transition history (spec §2's
/// "bounded independent transition history", capped at 50 per
/// `registry::MAX_TRANSITIONS_PER_SESSION`).
#[derive(Debug, Clone, PartialEq)]
pub struct AgentTransition {
    pub state: AgentSessionState,
    pub entered_at: Instant,
}

/// The registry's own mutable, `Instant`-clocked record of one Agent
/// Session (spec §2's `AgentSession` field list). Clock-agnostic like
/// `queue.rs`'s items: every method that needs "now" takes it as a
/// parameter, no wall-clock read happens inside this module — tests
/// pass a simulated clock (CLAUDE.md's injected-clock rule).
#[derive(Debug, Clone)]
pub struct AgentSession {
    // `key`/`first_seen_at` are read back by `to_state` (ticket 136,
    // `agents/board.rs`'s live `agent-state` publish path) in addition to
    // `apply_event`/`ordered_states`' own write/compare uses.
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
    /// A brand-new session always starts in `Starting` at `now` — see
    /// `registry::next_state`'s doc for why the very first event is run
    /// through the same transition function as every later one instead
    /// of being special-cased.
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

    /// Appends a transition, evicting the oldest entry once the history
    /// exceeds `cap` (spec §3.2's caps table: 50 retained transitions
    /// per session).
    pub fn push_history(&mut self, state: AgentSessionState, entered_at: Instant, cap: usize) {
        self.history.push(AgentTransition { state, entered_at });
        while self.history.len() > cap {
            self.history.remove(0);
        }
    }

    /// Builds the wire-facing snapshot at `now`. `terminal_retention` is
    /// only used to compute `retention_remaining_ms` for terminal
    /// sessions — see that field's doc on `AgentState`.
    ///
    /// Ticket 136 (agent-state IPC, `agents/board.rs`) calls this from
    /// the live `agent-state` publish path, via
    /// `AgentRegistry::ordered_states`.
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

/// The (future) wire-facing Agent snapshot (spec §2.3), built by
/// [`AgentSession::to_state`]. Not serialized or emitted anywhere yet
/// (ticket 136, `agents/board.rs`'s `AgentBoardPublisher`).
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
    /// Absolute, `now`-independent — only changes at a real first-seen
    /// event, so (like `SlotState::ttl_ms`) it stays IN `dedup_eq`'s
    /// comparison.
    pub first_seen_at: Instant,
    /// Absolute, `now`-independent — only changes at a real transition,
    /// so it stays IN `dedup_eq`'s comparison too.
    pub state_entered_at: Instant,
    /// Clock-derived (EXCLUDED from `dedup_eq`): milliseconds since this
    /// session last accepted an event, recomputed fresh against the
    /// `now` passed to `to_state` on every call — it differs on almost
    /// every call even with zero real content change, exactly the
    /// `SlotState::remaining_ms` shape this field is modeled on.
    pub last_seen_at_ms: u64,
    /// Clock-derived (EXCLUDED): milliseconds since `state_entered_at`.
    pub elapsed_ms: u64,
    /// Clock-derived (EXCLUDED): only `Some` for terminal sessions,
    /// counting down as `terminal_retention_secs` elapses.
    pub retention_remaining_ms: Option<u64>,
}

impl AgentState {
    /// Dedup-only equality (spec §2.3), handwritten — NEVER the derived
    /// `PartialEq` above, which stays intact and honest for tests that
    /// want full structural equality. Same invariant as
    /// `SlotState::dedup_eq` (CLAUDE.md's `dedup_eq` rule): continuously
    /// varying wire fields must be excluded here explicitly, never by
    /// deriving `PartialEq` and using that for publish suppression.
    ///
    /// The struct is destructured with every field named below — no
    /// `..` wildcard — as a compile-time guard: a future continuously
    /// varying field can't silently join this struct and this function
    /// unchanged; adding one is a compile error here until the author
    /// decides in/out, exactly like `SlotState::dedup_eq`'s match.
    ///
    /// Ticket 136's `agents/board.rs::AgentBoardPublisher` calls this
    /// (slice-wise, via its own `states_dedup_eq`) from the live
    /// `agent-state` publish path.
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
    fn urgency_rank_matches_spec_order() {
        use AgentSessionState::*;
        let ranks = [
            WaitingForPermission,
            WaitingForInput,
            Failed,
            Stale,
            Working,
            Starting,
            Completed,
        ]
        .map(|s| s.urgency_rank());
        let mut sorted = ranks;
        sorted.sort_unstable();
        assert_eq!(
            ranks, sorted,
            "spec §2.2 order must already be rank-ascending"
        );
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
        // The oldest surviving entries are the ones closest to eviction —
        // the very first `Starting` seed entry must have been evicted.
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
        // `state_entered_at` participates in ordering (spec §2.2) and is
        // absolute/now-independent, so unlike the clock-derived fields it
        // must stay IN the comparison.
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
