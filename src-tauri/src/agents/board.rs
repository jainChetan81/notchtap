//! Plan 136 (v7 ticket 4 of 13, `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md`
//! §6/§6.1/§6.2 resting): the `agent-state` IPC — a Rust-ordered
//! `AgentSessionView[]` wire snapshot, published independently of
//! `slot-state`/`status-state` (`event.rs`/`status.rs`).
//!
//! Publish suppression mirrors those two channels' own dedup discipline
//! (CLAUDE.md's `SlotState::dedup_eq` rule): [`AgentState::dedup_eq`]
//! (model.rs, handwritten, spec §2.3) decides whether a fresh
//! `AgentRegistry::ordered_states` read differs from the last published
//! snapshot; [`AgentBoardPublisher`]'s revision counter increments ONLY
//! when it does — a clock-only tick (elapsed-time/retention-countdown
//! drift alone) never bumps it and never emits.
//!
//! This module is also the SINGLE place the Agent Board's PRESENCE is
//! decided (operator decision 2026-08-02, `[agents] board_show_working`):
//! [`AgentBoardPublisher::gate_presence`] runs one layer above the dedup
//! comparison, so a Board nobody needs publishes as zero sessions and
//! every downstream consumer — the overlay's `presentationMode`
//! (`src/lib/presentation.ts`) and `lib.rs`'s hover-expand alike — stays
//! ignorant of the knob and simply reads the published snapshot.

use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::Emitter;

use super::adapter::{capability_wire_label, runtime_wire_label, state_wire_label};
use super::model::{session_hash_hex, AgentState};
use super::registry::AgentRegistryHandle;

/// The overlay's own listener string (`src/useAgentState.ts`). Change
/// both together.
pub const AGENT_STATE_EVENT: &str = "agent-state";

/// The full `agent-state` wire snapshot (spec §6). `sessions` arrives
/// already ordered by `AgentRegistry::ordered_states` (spec §2.2) — the
/// overlay performs no sorting, lifecycle inference, expiry, or history
/// merging of its own (spec §6's own words).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStateSnapshot {
    pub revision: u64,
    /// Wall-clock epoch millis at the moment this snapshot was built —
    /// the anchor the frontend derives LIVE per-session elapsed-in-state
    /// time from locally on its own interval, same `capturedAtMs` +
    /// `elapsedMs` pattern `NowPlayingSummary` already uses (status.rs)
    /// rather than Rust publishing a per-second clock tick (CLAUDE.md's
    /// `dedup_eq` rule: continuously varying fields must never drive a
    /// wire emission).
    pub captured_at_ms: i64,
    pub sessions: Vec<AgentSessionView>,
    /// Always empty until ticket 143 (`health.rs`) populates real
    /// per-runtime adapter health — typed now (spec §6: "the health
    /// array may be empty/stub until ticket 143") so the wire shape is
    /// stable across that later ticket rather than changing shape then.
    pub adapter_health: Vec<AdapterHealthView>,
}

/// One Agent Session on the wire (spec §6.2's resting-card field list:
/// runtime, state, project, elapsed state time, latest safe summary).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionView {
    /// `agents::model::session_hash_hex` — never the raw native session
    /// id (spec §9's persistence/privacy rule), same discipline
    /// `AgentSignal.session_hash` already follows (event.rs, plan 135).
    /// Doubles as the frontend's React list key.
    pub id: String,
    /// The same wire token an adapter itself would send (spec §3.1),
    /// via `adapter::runtime_wire_label` — not a display label; a
    /// display label is a frontend/Settings rendering concern.
    pub runtime: String,
    /// Same "wire token, not a display label" rule, via
    /// `adapter::state_wire_label`.
    pub state: String,
    pub capabilities: Vec<String>,
    /// Already sanitized/capped by `agents::adapter::parse_wire_event` —
    /// this view never re-derives or further truncates it.
    pub summary: Option<String>,
    pub details: Vec<AgentDetailView>,
    pub project: Option<AgentProjectView>,
    pub host: Option<AgentHostView>,
    /// Plan 147: the session's own subagent summary (spec §3.1's
    /// `subagent` object), mirrored 1:1 from `AgentState.subagent` — was
    /// stubbed `None` unconditionally at this ticket's predecessor
    /// (plan 136); the registry (`registry.rs:288`) has populated the
    /// domain field since plan 133/134, this view just hadn't surfaced it.
    pub subagent: Option<AgentSubagentView>,
    /// Clock-derived: milliseconds since `state_entered_at`, as of
    /// `captured_at_ms` above — changes on every publish even with zero
    /// real content change (mirrors `AgentState.elapsed_ms`, which is
    /// excluded from `dedup_eq` for exactly that reason).
    pub elapsed_ms: u64,
    /// `Some` only for a terminal (`Completed`/`Failed`) session,
    /// counting down as `agents.terminal_retention_secs` elapses.
    pub retention_remaining_ms: Option<u64>,
    /// Plan 142 (v7 ticket 10 of 13, spec §6.2 expanded): the session's
    /// bounded transition history (`AgentState.history`, capped at
    /// `registry::MAX_TRANSITIONS_PER_SESSION` upstream — this view maps
    /// 1:1, it never re-derives or re-caps), oldest first, exactly the
    /// order `AgentSession::push_history` appends in. The overlay's
    /// expanded per-row disclosure renders this as-is (no re-sort,
    /// mirroring the "no sorting/lifecycle inference" rule the wire
    /// snapshot as a whole already carries).
    pub history: Vec<AgentTransitionView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDetailView {
    pub label: String,
    pub value: String,
}

/// One entry of `AgentSessionView.history` (plan 142). `elapsed_ms` is
/// CLOCK-DERIVED (milliseconds since that transition started, as of
/// `captured_at_ms`) — same live-tick shape as `AgentSessionView.
/// elapsed_ms` — but that's safe here specifically because publish
/// suppression (`states_dedup_eq`/`AgentState::dedup_eq`) runs one layer
/// BELOW this view, against the domain `AgentTransition`'s `Instant`
/// (stable unless a real transition happens), never against this view.
/// A clock-only tick therefore still can't drive a re-publish, even
/// though this field itself changes on every call to `to_view`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTransitionView {
    pub state: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProjectView {
    pub name: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHostView {
    pub name: Option<String>,
    pub bundle_id: Option<String>,
}

/// Plan 147: mirrors `AgentSubagentSummary` (model.rs) 1:1 — same
/// "wire-shape view struct, no re-derivation" idiom as
/// `AgentProjectView`/`AgentHostView` above.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSubagentView {
    pub id: String,
    pub label: Option<String>,
    pub state: Option<String>,
}

/// Wire shape for one Adapter Health card (ticket 143, spec §6/§10) —
/// built from [`super::health::AdapterHealth`] via [`health_to_view`].
/// `status` keeps its original ticket-136-stub name (rather than
/// `availability`) since the overlay's permissive
/// `isValidAdapterHealth` (`useAgentState.ts`) already pins that field
/// name; every other field is additive.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterHealthView {
    pub runtime: String,
    pub status: String,
    pub enabled: bool,
    pub capabilities: Vec<String>,
    pub last_accepted_event_ms: Option<i64>,
    pub last_error_category: Option<String>,
    pub compatibility_message: Option<String>,
}

/// Builds one wire row from a domain [`super::health::AdapterHealth`] —
/// the same "wire token, not a display label" discipline [`to_view`]
/// above already follows for runtime/state/capabilities. `pub(crate)`:
/// `settings.rs`'s `get_agent_health` command reuses this exact
/// conversion rather than re-deriving its own, so the overlay's
/// `agent-state` snapshot and the Settings Agents section read the
/// identical wire shape for Adapter Health.
pub(crate) fn health_to_view(health: &super::health::AdapterHealth) -> AdapterHealthView {
    AdapterHealthView {
        runtime: runtime_wire_label(health.runtime).to_string(),
        status: health.availability.label().to_string(),
        enabled: health.enabled,
        capabilities: health
            .capabilities
            .iter()
            .copied()
            .map(capability_wire_label)
            .map(str::to_string)
            .collect(),
        last_accepted_event_ms: health.last_accepted_event_ms,
        last_error_category: health.last_error_category.map(|c| c.label().to_string()),
        compatibility_message: health.compatibility_message.clone(),
    }
}

/// `now` (plan 142) is used ONLY to derive each history entry's
/// `elapsed_ms` (see `AgentTransitionView`'s own doc for why that's safe
/// dedup-wise) — every other field here was already `now`-independent
/// before this ticket.
fn to_view(state: &AgentState, now: Instant) -> AgentSessionView {
    AgentSessionView {
        id: session_hash_hex(&state.key),
        runtime: runtime_wire_label(state.key.runtime).to_string(),
        state: state_wire_label(state.state).to_string(),
        capabilities: state
            .capabilities
            .iter()
            .copied()
            .map(capability_wire_label)
            .map(str::to_string)
            .collect(),
        summary: state.summary.clone(),
        details: state
            .details
            .iter()
            .map(|d| AgentDetailView {
                label: d.label.clone(),
                value: d.value.clone(),
            })
            .collect(),
        project: state.project.as_ref().map(|p| AgentProjectView {
            name: p.name.clone(),
            cwd: p.cwd.clone(),
        }),
        host: state.host.as_ref().map(|h| AgentHostView {
            name: h.name.clone(),
            bundle_id: h.bundle_id.clone(),
        }),
        subagent: state.subagent.as_ref().map(|s| AgentSubagentView {
            id: s.id.clone(),
            label: s.label.clone(),
            state: s.state.clone(),
        }),
        elapsed_ms: state.elapsed_ms,
        retention_remaining_ms: state.retention_remaining_ms,
        history: state
            .history
            .iter()
            .map(|t| AgentTransitionView {
                state: state_wire_label(t.state).to_string(),
                elapsed_ms: now.saturating_duration_since(t.entered_at).as_millis() as u64,
            })
            .collect(),
    }
}

/// Wall-clock epoch millis "now" — same technique as
/// `now_playing.rs::now_ms`/`history.rs::now_ms`, each module's own
/// private copy rather than a shared crate-internal helper (neither of
/// those two is in this ticket's scope to refactor).
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Slice-level `dedup_eq` (spec §2.3's per-session invariant lifted to
/// the whole snapshot, CLAUDE.md's `SlotState::dedup_eq` rule): a
/// different session COUNT is always a change; equal-length slices
/// compare pairwise, in order. A genuine reordering (spec §2.2: a state
/// change re-enqueues into a new urgency-class FIFO position) is already
/// caught this way without any special-case, because the fields that
/// drive ordering (`state`, `state_entered_at`) are themselves IN
/// `AgentState::dedup_eq`'s own comparison — reordering can't happen
/// without at least one of those changing on at least one session.
fn states_dedup_eq(a: &[AgentState], b: &[AgentState]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.dedup_eq(y))
}

struct PublishState {
    last: Option<Vec<AgentState>>,
    revision: u64,
}

/// Cheaply-cloned publisher, generic over the tauri runtime like
/// `Engine<R>` (`engine.rs`) — so it's constructible against
/// `tauri::test::mock_app()`'s `MockRuntime` in tests, not just the real
/// `tauri::Wry` app.
///
/// Unlike `status.rs`'s `last_status` (a bare local the rotation loop
/// alone owns), the dedup/revision bookkeeping here must be shared
/// across TWO independent publish call sites — the `/agent/events`
/// handler (`http.rs`, after every `Applied` mutation) and the periodic
/// tick (`spawn_tick` below, driving spec §2.1's stale/retention
/// transitions) — so it lives behind an `Arc<Mutex<_>>` instead of a
/// single owning task's stack.
pub struct AgentBoardPublisher<R: tauri::Runtime = tauri::Wry> {
    app: tauri::AppHandle<R>,
    registry: AgentRegistryHandle,
    state: Arc<StdMutex<PublishState>>,
    /// Plan 143 (ticket 11 of 13): the shared Adapter Health bookkeeping
    /// `http.rs`'s `/agent/events` handler writes to on every
    /// accepted/rejected event — read fresh into every EMITTED snapshot
    /// below (not itself part of `states_dedup_eq`'s comparison: a
    /// health-only change with no session-state change simply rides
    /// along on the next real state-triggered publish, same as
    /// `captured_at_ms`/`elapsed_ms` already do for other clock-derived
    /// fields — see [`states_dedup_eq`]'s own doc for why the CLAUDE.md
    /// dedup rule only needs to cover fields that alone should suppress
    /// a publish).
    health: Arc<super::health::HealthTracker>,
    /// Captured once at construction — `[agents.runtimes.*]` only ever
    /// changes via `save_config_and_relaunch`, which restarts the whole
    /// process (`settings.rs`), so there is no live-mutation case this
    /// copy could go stale against within one process's lifetime (same
    /// assumption `http.rs`'s `AppState::agent_runtimes` field already
    /// relies on).
    runtimes_cfg: crate::config::AgentRuntimesConfig,
    /// `[agents] board_show_working` (config.rs), captured once at
    /// construction for the same reason `runtimes_cfg` above is. `false`
    /// (the default) makes this publisher apply the Board PRESENCE gate
    /// documented on [`Self::publish_if_changed`]; `true` restores the
    /// pre-2026-08-02 behavior where any live session shows the Board.
    board_show_working: bool,
    /// Plan 171: live-session count mirror for the agent icon — stored
    /// UNGATED (before `gate_presence`) in `publish_if_changed`, because
    /// the icon's "a session is genuinely running" tier must see Working
    /// sessions even when `board_show_working` hides them from the Board.
    tab_wire: std::sync::Arc<crate::tabs::TabWire>,
}

impl<R: tauri::Runtime> Clone for AgentBoardPublisher<R> {
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            registry: self.registry.clone(),
            state: self.state.clone(),
            health: self.health.clone(),
            runtimes_cfg: self.runtimes_cfg,
            board_show_working: self.board_show_working,
            tab_wire: self.tab_wire.clone(),
        }
    }
}

impl<R: tauri::Runtime> AgentBoardPublisher<R> {
    pub fn new(
        app: tauri::AppHandle<R>,
        registry: AgentRegistryHandle,
        health: Arc<super::health::HealthTracker>,
        runtimes_cfg: crate::config::AgentRuntimesConfig,
        board_show_working: bool,
        tab_wire: std::sync::Arc<crate::tabs::TabWire>,
    ) -> Self {
        Self {
            app,
            registry,
            state: Arc::new(StdMutex::new(PublishState {
                last: None,
                revision: 0,
            })),
            health,
            runtimes_cfg,
            board_show_working,
            tab_wire,
        }
    }

    /// Reads `AgentRegistry::ordered_states` at `now`, applies the Board
    /// PRESENCE gate ([`Self::gate_presence`]), and emits `agent-state`
    /// ONLY if the gated slice differs from the last published snapshot
    /// per [`states_dedup_eq`]. The revision counter bumps strictly in
    /// lockstep with an actual emit — never independently — so a
    /// suppressed no-op call can't leave the counter ahead of what the
    /// wire last actually carried. Returns whether it emitted (test
    /// hook).
    ///
    /// The gate runs BEFORE the dedup comparison, not after, which is
    /// what makes this the single place Board presence is decided:
    /// `PublishState.last` — and therefore
    /// [`Self::last_session_count`], which `lib.rs`'s hover primitive
    /// reads to answer "is the Board what's on screen?" — always holds
    /// exactly what the overlay was last told. A gated-off snapshot
    /// publishes as ZERO sessions, so the frontend's own
    /// `presentationMode` (src/lib/presentation.ts) falls through to
    /// idle with no knowledge of the knob, and hover-expand declines for
    /// the same reason, without a second gate in either layer.
    pub async fn publish_if_changed(&self, now: Instant) -> bool {
        let ungated = self.registry.ordered_states(now).await;
        // Plan 171: the agent icon counts LIVE sessions (spec §6:
        // "a session is genuinely running") — non-terminal, non-stale —
        // from the ungated registry view, independent of the Board's own
        // show-working presence gate.
        self.tab_wire.agent_sessions.store(
            ungated
                .iter()
                .filter(|st| {
                    !st.state.is_terminal()
                        && st.state != crate::agents::model::AgentSessionState::Stale
                })
                .count(),
            std::sync::atomic::Ordering::Relaxed,
        );
        let states = self.gate_presence(ungated);
        // poison-tolerant, matching this codebase's other `StdMutex`
        // guards — a panic elsewhere while holding this lock must not
        // permanently wedge every later publish attempt.
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let changed = match &guard.last {
            None => true,
            Some(prev) => !states_dedup_eq(prev, &states),
        };
        if !changed {
            return false;
        }
        guard.revision += 1;
        let revision = guard.revision;
        guard.last = Some(states.clone());
        drop(guard);

        let adapter_health = self
            .health
            .snapshot(&self.runtimes_cfg, now)
            .iter()
            .map(health_to_view)
            .collect();
        let snapshot = AgentStateSnapshot {
            revision,
            captured_at_ms: now_ms(),
            sessions: states.iter().map(|s| to_view(s, now)).collect(),
            adapter_health,
        };
        if let Err(e) = self.app.emit(AGENT_STATE_EVENT, &snapshot) {
            tracing::error!("failed to emit agent-state: {e}");
        }
        true
    }

    /// The Agent Board's PRESENCE gate (operator decision 2026-08-02:
    /// "agents that are merely working must not summon the board"). All
    /// or nothing, never a row filter: with `board_show_working = false`
    /// (the default), a slice holding no attention-state session at all
    /// (`AgentSessionState::summons_board`) publishes as EMPTY — the
    /// Board simply isn't present — but the moment ONE session needs the
    /// operator, the whole ordered slice publishes unchanged, working
    /// sessions and all. Presence is gated; content is not.
    ///
    /// `board_show_working = true` is the identity function, i.e. the
    /// pre-2026-08-02 behavior.
    fn gate_presence(&self, states: Vec<AgentState>) -> Vec<AgentState> {
        if self.board_show_working || states.iter().any(|s| s.state.summons_board()) {
            states
        } else {
            Vec::new()
        }
    }

    /// Plan 142 (v7 ticket 10 of 13, spec §6.2 expanded): a cheap,
    /// SYNCHRONOUS read of the session count in the last published
    /// `agent-state` snapshot. `lib.rs`'s hover primitive needs "is the
    /// Board currently showing any sessions at all" from inside an
    /// AppKit mouse-event callback, which never runs on the tokio
    /// runtime — `self.registry` (`AgentRegistryHandle`) is async-only
    /// (`tokio::sync::Mutex`), so it isn't reachable there, but this
    /// publisher's own dedup bookkeeping (`PublishState.last`) is already
    /// behind a plain `StdMutex`, updated in lockstep with every real
    /// publish — always at least as fresh as the last `agent-state`
    /// event the frontend itself just rendered.
    ///
    /// Because [`Self::gate_presence`] runs before that bookkeeping is
    /// written, this reads `0` whenever the Board is gated off, so
    /// hovering the idle rail can never expand a Board that isn't
    /// displayed — no separate presence check is needed at the hover
    /// call sites (`lib.rs`).
    pub fn last_session_count(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last
            .as_ref()
            .map(Vec::len)
            .unwrap_or(0)
    }

    /// The periodic driver for spec §2.1's time-only transitions (a
    /// non-terminal session going `Stale` after `stale_after_secs` of
    /// silence; a terminal session leaving the live registry view after
    /// `terminal_retention_secs`). Ticks the registry, then publishes —
    /// which itself only actually emits if that tick produced a real
    /// content change (a state flip, or a session disappearing), never
    /// on elapsed-time drift alone (`publish_if_changed`'s own dedup
    /// handles that).
    ///
    /// Time-interval-driven, unlike `Engine::spawn_rotation`'s
    /// deadline/wake loop: there's no external mutation-wake source to
    /// arm against here (an `/agent/events` mutation publishes its own
    /// change directly, via `publish_if_changed` above, at the http
    /// layer) — only the wall clock needs polling, for the stale/
    /// retention sweep alone.
    pub fn spawn_tick(&self, interval: Duration) {
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            // `MissedTickBehavior::Delay` (tokio's default) is fine here:
            // unlike a rotation deadline, a late stale/retention sweep
            // has no visible timing contract to violate — the resting
            // board just recognizes staleness a little later, once.
            loop {
                ticker.tick().await;
                let now = Instant::now();
                this.registry.tick(now).await;
                this.publish_if_changed(now).await;
            }
        });
    }
}

/// Default interval for [`AgentBoardPublisher::spawn_tick`] — fine
/// enough granularity for a resting-card elapsed-time/stale sweep
/// without being a busy poll; not yet config-wired (plan 137's job, same
/// as `stale_after`/`terminal_retention_secs` themselves — see
/// `AgentRegistry`'s own doc).
pub const DEFAULT_TICK_INTERVAL: Duration = Duration::from_secs(5);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::model::{
        AgentEventKind, AgentRuntime, AgentSessionKey, AgentSessionState, AgentSubagentSummary,
    };
    use crate::agents::registry::{AgentEvent, AgentRegistry};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn key(runtime: AgentRuntime, id: &str) -> AgentSessionKey {
        AgentSessionKey::new(runtime, id).unwrap()
    }

    // plan 147: same "build an AgentState directly" idiom as
    // `focus.rs::state_with_host` — `to_view` is private to this module,
    // so its unit tests live here rather than round-tripping through the
    // registry/publisher.
    fn state_with_subagent(subagent: Option<AgentSubagentSummary>) -> AgentState {
        let now = Instant::now();
        AgentState {
            key: key(AgentRuntime::ClaudeCode, "session-1"),
            state: AgentSessionState::Working,
            capabilities: Vec::new(),
            summary: None,
            details: Vec::new(),
            project: None,
            host: None,
            subagent,
            history: Vec::new(),
            first_seen_at: now,
            state_entered_at: now,
            last_seen_at_ms: 0,
            elapsed_ms: 0,
            retention_remaining_ms: None,
        }
    }

    #[test]
    fn to_view_maps_subagent_when_present() {
        let state = state_with_subagent(Some(AgentSubagentSummary {
            id: "sub-1".to_string(),
            label: Some("Explorer".to_string()),
            state: Some("running".to_string()),
        }));
        let view = to_view(&state, Instant::now());
        let subagent = view.subagent.expect("subagent must be mapped when present");
        assert_eq!(subagent.id, "sub-1");
        assert_eq!(subagent.label.as_deref(), Some("Explorer"));
        assert_eq!(subagent.state.as_deref(), Some("running"));
    }

    #[test]
    fn to_view_subagent_is_none_when_absent() {
        let state = state_with_subagent(None);
        let view = to_view(&state, Instant::now());
        assert!(view.subagent.is_none());
    }

    fn event(session_key: AgentSessionKey, event_id: &str, kind: AgentEventKind) -> AgentEvent {
        AgentEvent {
            event_id: event_id.to_string(),
            session_key,
            sequence: None,
            kind,
            declared_state: AgentSessionState::Starting,
            terminal: false,
            capabilities: Vec::new(),
            summary: None,
            details: Vec::new(),
            project: None,
            host: None,
            subagent: None,
        }
    }

    fn publisher_with(
        app: &tauri::App<tauri::test::MockRuntime>,
        board_show_working: bool,
    ) -> AgentBoardPublisher<tauri::test::MockRuntime> {
        let registry = AgentRegistryHandle::new(AgentRegistry::new(
            Duration::from_secs(300),
            Duration::from_secs(600),
            Duration::from_secs(1800),
        ));
        AgentBoardPublisher::new(
            app.handle().clone(),
            registry,
            Arc::new(crate::agents::health::HealthTracker::new()),
            crate::config::AgentRuntimesConfig::default(),
            board_show_working,
            std::sync::Arc::new(crate::tabs::TabWire::default()),
        )
    }

    /// The dedup/revision/wire-shape tests below predate the Board's
    /// presence gate (operator decision 2026-08-02) and assert against
    /// the UNGATED snapshot, so they build the publisher with
    /// `board_show_working = true`. The gate's own behavior has its own
    /// tests at the bottom of this module, built via `publisher_with(app,
    /// false)`.
    fn publisher(
        app: &tauri::App<tauri::test::MockRuntime>,
    ) -> AgentBoardPublisher<tauri::test::MockRuntime> {
        publisher_with(app, true)
    }

    fn listen_count(app: &tauri::App<tauri::test::MockRuntime>) -> Arc<AtomicUsize> {
        use tauri::Listener;
        let count = Arc::new(AtomicUsize::new(0));
        let counter = count.clone();
        app.handle().listen(AGENT_STATE_EVENT, move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        count
    }

    #[test]
    fn event_name_is_pinned() {
        // The frontend listens for exactly this literal
        // (src/useAgentState.ts) — same rationale as
        // `status_state_event_name_is_pinned` (status.rs).
        assert_eq!(AGENT_STATE_EVENT, "agent-state");
    }

    #[tokio::test]
    async fn first_publish_with_a_session_emits_and_starts_revision_at_one() {
        let app = tauri::test::mock_app();
        let publisher = publisher(&app);
        let count = listen_count(&app);
        let now = Instant::now();
        publisher
            .registry
            .apply_event(
                event(
                    key(AgentRuntime::Codex, "s1"),
                    "e1",
                    AgentEventKind::PermissionRequested,
                ),
                now,
            )
            .await;

        let emitted = publisher.publish_if_changed(now).await;
        assert!(emitted);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(publisher.state.lock().unwrap().revision, 1);
    }

    #[tokio::test]
    async fn clock_only_re_publish_is_suppressed_and_does_not_bump_revision() {
        let app = tauri::test::mock_app();
        let publisher = publisher(&app);
        let count = listen_count(&app);
        let base = Instant::now();
        publisher
            .registry
            .apply_event(
                event(
                    key(AgentRuntime::Codex, "s1"),
                    "e1",
                    AgentEventKind::PermissionRequested,
                ),
                base,
            )
            .await;

        assert!(publisher.publish_if_changed(base).await);
        // Same content, only wall-clock time (and therefore elapsed_ms/
        // last_seen_at_ms) has moved — must NOT re-emit, and the
        // revision must stay exactly where the first real publish left
        // it (CLAUDE.md: "a revision counter must NOT defeat
        // suppression").
        let later = base + Duration::from_secs(30);
        let emitted = publisher.publish_if_changed(later).await;
        assert!(!emitted, "a clock-only tick must not publish");
        assert_eq!(count.load(Ordering::SeqCst), 1, "no second emit landed");
        assert_eq!(
            publisher.state.lock().unwrap().revision,
            1,
            "revision must not advance on a suppressed publish"
        );
    }

    #[tokio::test]
    async fn a_real_state_change_publishes_again_and_bumps_revision() {
        let app = tauri::test::mock_app();
        let publisher = publisher(&app);
        let count = listen_count(&app);
        let base = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        publisher
            .registry
            .apply_event(
                event(k.clone(), "e1", AgentEventKind::PermissionRequested),
                base,
            )
            .await;
        assert!(publisher.publish_if_changed(base).await);

        // A work event clears the waiting state — a genuine content
        // change (spec §2.1) — must publish again with an incremented
        // revision.
        publisher
            .registry
            .apply_event(event(k, "e2", AgentEventKind::Informational), base)
            .await;
        let emitted = publisher.publish_if_changed(base).await;
        assert!(emitted);
        assert_eq!(count.load(Ordering::SeqCst), 2);
        assert_eq!(publisher.state.lock().unwrap().revision, 2);
    }

    #[tokio::test]
    async fn stale_transition_via_tick_publishes() {
        let app = tauri::test::mock_app();
        let publisher = publisher(&app);
        let count = listen_count(&app);
        let base = Instant::now();
        publisher
            .registry
            .apply_event(
                event(
                    key(AgentRuntime::Codex, "s1"),
                    "e1",
                    AgentEventKind::Informational,
                ),
                base,
            )
            .await;
        assert!(publisher.publish_if_changed(base).await);

        // Advance past `stale_after` (300s in this test's registry) and
        // run the same tick+publish sequence `spawn_tick`'s loop body
        // performs — the resulting `Stale` transition is a real content
        // change and must publish.
        let past_stale = base + Duration::from_secs(300);
        publisher.registry.tick(past_stale).await;
        let emitted = publisher.publish_if_changed(past_stale).await;
        assert!(emitted, "a stale transition must publish");
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn terminal_retention_purge_via_tick_publishes() {
        let app = tauri::test::mock_app();
        let publisher = publisher(&app);
        let count = listen_count(&app);
        let base = Instant::now();
        publisher
            .registry
            .apply_event(
                event(
                    key(AgentRuntime::Codex, "s1"),
                    "e1",
                    AgentEventKind::Completed,
                ),
                base,
            )
            .await;
        // terminal: true is required for a real Completed transition —
        // build it directly rather than extending the `event` helper.
        {
            let mut e = event(
                key(AgentRuntime::Codex, "s2"),
                "e2",
                AgentEventKind::Completed,
            );
            e.terminal = true;
            publisher.registry.apply_event(e, base).await;
        }
        assert!(publisher.publish_if_changed(base).await);

        // Past terminal_retention (600s in this test's registry): the
        // session leaves the live registry view entirely — session
        // count 1 -> 0 is a real change (states_dedup_eq's length check)
        // and must publish.
        let past_retention = base + Duration::from_secs(600);
        publisher.registry.tick(past_retention).await;
        let emitted = publisher.publish_if_changed(past_retention).await;
        assert!(emitted, "a terminal-retention purge must publish");
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn empty_registry_clock_only_tick_never_publishes() {
        let app = tauri::test::mock_app();
        let publisher = publisher(&app);
        let count = listen_count(&app);
        let base = Instant::now();
        // No sessions at all: the very first publish of an empty
        // registry still counts as a real "0 sessions" snapshot (None ->
        // Some(empty) is a change), so seed that once first.
        assert!(publisher.publish_if_changed(base).await);
        count.store(0, Ordering::SeqCst);

        let later = base + Duration::from_secs(60);
        publisher.registry.tick(later).await;
        let emitted = publisher.publish_if_changed(later).await;
        assert!(!emitted);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn snapshot_wire_shape_is_camel_case_and_carries_expected_fields() {
        let app = tauri::test::mock_app();
        let publisher = publisher(&app);
        let now = Instant::now();
        publisher
            .registry
            .apply_event(
                event(
                    key(AgentRuntime::Codex, "s1"),
                    "e1",
                    AgentEventKind::PermissionRequested,
                ),
                now,
            )
            .await;
        let states = publisher.registry.ordered_states(now).await;
        let snapshot = AgentStateSnapshot {
            revision: 1,
            captured_at_ms: 0,
            sessions: states.iter().map(|s| to_view(s, now)).collect(),
            adapter_health: Vec::new(),
        };
        let json = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(json["revision"], 1);
        assert!(json.get("capturedAtMs").is_some());
        assert_eq!(json["adapterHealth"], serde_json::json!([]));
        let session = &json["sessions"][0];
        assert_eq!(session["runtime"], "codex");
        assert_eq!(session["state"], "waiting_for_permission");
        assert!(session.get("elapsedMs").is_some());
        assert!(session.get("id").is_some());
        // never the raw native session id on the wire (spec §9)
        assert_ne!(session["id"], "s1");
    }

    // --- plan 142 (v7 ticket 10 of 13, spec §6.2 expanded): the wire
    // view's new `history` field + the publisher's synchronous session
    // count read. ---

    #[tokio::test]
    async fn snapshot_wire_shape_carries_bounded_history_oldest_first() {
        let app = tauri::test::mock_app();
        let publisher = publisher(&app);
        let base = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        // Starting (seeded by AgentSession::new) -> WaitingForPermission
        // -> Working: three history entries, oldest first.
        publisher
            .registry
            .apply_event(
                event(k.clone(), "e1", AgentEventKind::PermissionRequested),
                base,
            )
            .await;
        publisher
            .registry
            .apply_event(event(k, "e2", AgentEventKind::Informational), base)
            .await;

        let states = publisher.registry.ordered_states(base).await;
        let snapshot = AgentStateSnapshot {
            revision: 1,
            captured_at_ms: 0,
            sessions: states.iter().map(|s| to_view(s, base)).collect(),
            adapter_health: Vec::new(),
        };
        let json = serde_json::to_value(&snapshot).unwrap();
        let history = json["sessions"][0]["history"].as_array().unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0]["state"], "starting");
        assert_eq!(history[1]["state"], "waiting_for_permission");
        assert_eq!(history[2]["state"], "working");
        assert!(history[0].get("elapsedMs").is_some());
    }

    #[tokio::test]
    async fn a_new_transition_appended_to_history_still_publishes_and_a_clock_only_tick_still_does_not(
    ) {
        let app = tauri::test::mock_app();
        let publisher = publisher(&app);
        let count = listen_count(&app);
        let base = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
        publisher
            .registry
            .apply_event(
                event(k.clone(), "e1", AgentEventKind::PermissionRequested),
                base,
            )
            .await;
        assert!(publisher.publish_if_changed(base).await);
        assert_eq!(count.load(Ordering::SeqCst), 1);

        // A real state change appends a new history entry — must publish.
        publisher
            .registry
            .apply_event(event(k, "e2", AgentEventKind::Informational), base)
            .await;
        assert!(publisher.publish_if_changed(base).await);
        assert_eq!(count.load(Ordering::SeqCst), 2);

        // Wall-clock time alone moving (every history entry's `elapsed_ms`
        // would differ in a freshly-built view) must NOT publish — dedup
        // runs on the domain `AgentState.history`'s stable `Instant`
        // values, one layer below the view this ticket added the
        // clock-derived field to.
        let later = base + Duration::from_secs(10);
        let emitted = publisher.publish_if_changed(later).await;
        assert!(
            !emitted,
            "a clock-only tick must not publish even with history present"
        );
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn last_session_count_reflects_the_last_published_snapshot() {
        let app = tauri::test::mock_app();
        let publisher = publisher(&app);
        let base = Instant::now();
        assert_eq!(publisher.last_session_count(), 0);

        publisher
            .registry
            .apply_event(
                event(
                    key(AgentRuntime::Codex, "s1"),
                    "e1",
                    AgentEventKind::PermissionRequested,
                ),
                base,
            )
            .await;
        // Not yet published — the count still reads the OLD (empty) snapshot.
        assert_eq!(publisher.last_session_count(), 0);

        assert!(publisher.publish_if_changed(base).await);
        assert_eq!(publisher.last_session_count(), 1);
    }

    // --- operator decision 2026-08-02: the Board's PRESENCE gate
    // (`[agents] board_show_working`, default false). "Agents that are
    // merely WORKING must not summon the Agent Board" — presence is
    // gated, content is not. ---

    /// An ordinary progress event from a session that is ALREADY
    /// working. `declared_state: Working` is what
    /// `AgentRegistry::apply_event` reads to tell a real progress event
    /// apart from a session-start `Informational` (which must stay at
    /// the `Starting` baseline) — see that method's `is_session_start`
    /// branch.
    fn working_event(session_key: AgentSessionKey, event_id: &str) -> AgentEvent {
        let mut e = event(session_key, event_id, AgentEventKind::Informational);
        e.declared_state = AgentSessionState::Working;
        e
    }

    /// What the last publish actually put on the wire (`PublishState.
    /// last` is written from the GATED slice), as `(state, ...)` wire
    /// tokens in Board order.
    fn published_states(
        publisher: &AgentBoardPublisher<tauri::test::MockRuntime>,
    ) -> Vec<&'static str> {
        publisher
            .state
            .lock()
            .unwrap()
            .last
            .as_ref()
            .map(|states| states.iter().map(|s| state_wire_label(s.state)).collect())
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn working_only_sessions_do_not_summon_the_board() {
        let app = tauri::test::mock_app();
        let publisher = publisher_with(&app, false);
        let base = Instant::now();
        for (i, id) in ["s1", "s2"].iter().enumerate() {
            publisher
                .registry
                .apply_event(
                    working_event(key(AgentRuntime::ClaudeCode, id), &format!("e{i}")),
                    base,
                )
                .await;
        }

        publisher.publish_if_changed(base).await;
        assert!(
            published_states(&publisher).is_empty(),
            "working-only sessions must publish as zero sessions, so the overlay's presentationMode falls to idle"
        );
        // hover-expand reads exactly this — an idle rail must not expand
        // into a Board that isn't displayed.
        assert_eq!(publisher.last_session_count(), 0);
    }

    #[tokio::test]
    async fn a_starting_or_stale_session_alone_does_not_summon_the_board_either() {
        let app = tauri::test::mock_app();
        let publisher = publisher_with(&app, false);
        let base = Instant::now();
        // A brand-new session's first Informational leaves it at the
        // `Starting` baseline (registry's `is_session_start` branch).
        publisher
            .registry
            .apply_event(
                event(
                    key(AgentRuntime::Codex, "s1"),
                    "e1",
                    AgentEventKind::Informational,
                ),
                base,
            )
            .await;
        publisher.publish_if_changed(base).await;
        assert!(published_states(&publisher).is_empty(), "starting alone");

        // Past `stale_after` (300s in this test's registry): Starting ->
        // Stale. A session that went quiet on its own is the absence of
        // news, not a request for attention.
        let past_stale = base + Duration::from_secs(300);
        publisher.registry.tick(past_stale).await;
        publisher.publish_if_changed(past_stale).await;
        assert!(published_states(&publisher).is_empty());
        assert_eq!(publisher.last_session_count(), 0);
    }

    #[tokio::test]
    async fn one_waiting_session_summons_the_board_and_the_working_ones_ride_along() {
        let app = tauri::test::mock_app();
        let publisher = publisher_with(&app, false);
        let base = Instant::now();
        publisher
            .registry
            .apply_event(
                working_event(key(AgentRuntime::ClaudeCode, "worker"), "e1"),
                base,
            )
            .await;
        publisher
            .registry
            .apply_event(
                event(
                    key(AgentRuntime::Codex, "asker"),
                    "e2",
                    AgentEventKind::PermissionRequested,
                ),
                base,
            )
            .await;

        assert!(publisher.publish_if_changed(base).await);
        // Presence is gated; CONTENT is not — the working session is
        // still listed once something else has summoned the Board.
        assert_eq!(
            published_states(&publisher),
            vec!["waiting_for_permission", "working"],
            "the whole ordered slice publishes, in Board order, not just the attention session"
        );
        assert_eq!(publisher.last_session_count(), 2);
    }

    #[tokio::test]
    async fn a_terminal_completed_session_summons_the_board_while_retained() {
        let app = tauri::test::mock_app();
        let publisher = publisher_with(&app, false);
        let base = Instant::now();
        let mut e = event(
            key(AgentRuntime::Kimi, "s1"),
            "e1",
            AgentEventKind::Completed,
        );
        e.terminal = true;
        publisher.registry.apply_event(e, base).await;

        assert!(publisher.publish_if_changed(base).await);
        assert_eq!(published_states(&publisher), vec!["completed"]);

        // ...and stops summoning it once the registry's own
        // `terminal_retention` (600s here) evicts it.
        let past_retention = base + Duration::from_secs(600);
        publisher.registry.tick(past_retention).await;
        assert!(publisher.publish_if_changed(past_retention).await);
        assert_eq!(publisher.last_session_count(), 0);
    }

    #[tokio::test]
    async fn the_board_leaves_when_the_attention_session_goes_back_to_work() {
        let app = tauri::test::mock_app();
        let publisher = publisher_with(&app, false);
        let base = Instant::now();
        let k = key(AgentRuntime::OpenCode, "s1");
        publisher
            .registry
            .apply_event(
                event(k.clone(), "e1", AgentEventKind::PermissionRequested),
                base,
            )
            .await;
        assert!(publisher.publish_if_changed(base).await);
        assert_eq!(publisher.last_session_count(), 1);

        // Permission granted, back to ordinary work — nothing needs the
        // operator any more, so the Board goes away again.
        publisher
            .registry
            .apply_event(event(k, "e2", AgentEventKind::Informational), base)
            .await;
        assert!(publisher.publish_if_changed(base).await);
        assert!(published_states(&publisher).is_empty());
        assert_eq!(publisher.last_session_count(), 0);
    }

    #[tokio::test]
    async fn board_show_working_true_restores_the_old_any_session_behavior() {
        let app = tauri::test::mock_app();
        let publisher = publisher_with(&app, true);
        let base = Instant::now();
        publisher
            .registry
            .apply_event(
                working_event(key(AgentRuntime::ClaudeCode, "s1"), "e1"),
                base,
            )
            .await;

        assert!(publisher.publish_if_changed(base).await);
        assert_eq!(published_states(&publisher), vec!["working"]);
        assert_eq!(publisher.last_session_count(), 1);
    }

    #[tokio::test]
    async fn a_gated_off_board_does_not_re_publish_on_every_working_session_change() {
        let app = tauri::test::mock_app();
        let publisher = publisher_with(&app, false);
        let count = listen_count(&app);
        let base = Instant::now();
        publisher
            .registry
            .apply_event(working_event(key(AgentRuntime::Codex, "s1"), "e1"), base)
            .await;
        // First publish seeds `last` with the gated (empty) slice.
        assert!(publisher.publish_if_changed(base).await);
        count.store(0, Ordering::SeqCst);

        // A second working session appearing is a real registry change,
        // but not a PRESENCE change — the gate runs before dedup, so
        // both slices are empty and nothing goes on the wire.
        publisher
            .registry
            .apply_event(working_event(key(AgentRuntime::Kimi, "s2"), "e2"), base)
            .await;
        assert!(!publisher.publish_if_changed(base).await);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}
