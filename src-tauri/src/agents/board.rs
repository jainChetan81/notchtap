//! The `agent-state` IPC: a Rust-ordered `AgentSessionView[]` wire snapshot, published
//! independently of `slot-state`/`status-state` (`event.rs`/`status.rs`).

use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::Emitter;

use super::adapter::{capability_wire_label, runtime_wire_label, state_wire_label};
use super::model::{session_hash_hex, AgentState};
use super::registry::AgentRegistryHandle;

/// The overlay's own listener string (`src/useAgentState.ts`). Change both together.
pub const AGENT_STATE_EVENT: &str = "agent-state";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStateSnapshot {
    pub revision: u64,
    pub captured_at_ms: i64,
    pub sessions: Vec<AgentSessionView>,
    pub tab_sessions: Vec<AgentSessionView>,
    pub adapter_health: Vec<AdapterHealthView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionView {
    /// `agents::model::session_hash_hex` — never the raw native session id, the same privacy
    /// discipline `AgentSignal.session_hash` follows (`event.rs`).
    pub id: String,
    pub runtime: String,
    pub state: String,
    pub capabilities: Vec<String>,
    /// Already sanitized/capped by `agents::adapter::parse_wire_event` — this view never re-derives
    /// or further truncates it.
    pub summary: Option<String>,
    pub details: Vec<AgentDetailView>,
    pub project: Option<AgentProjectView>,
    pub host: Option<AgentHostView>,
    pub subagent: Option<AgentSubagentView>,
    pub elapsed_ms: u64,
    pub retention_remaining_ms: Option<u64>,
    pub history: Vec<AgentTransitionView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDetailView {
    pub label: String,
    pub value: String,
}

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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSubagentView {
    pub id: String,
    pub label: Option<String>,
    pub state: Option<String>,
}

/// The field is named `status` (not `availability`) because the overlay's `isValidAdapterHealth`
/// (`useAgentState.ts`) pins that name — change both together.
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

/// Builds one wire row from a domain [`super::health::AdapterHealth`] — the same "wire token, not a
/// display label" discipline [`to_view`] above follows for runtime/state/capabilities.
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

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn states_dedup_eq(a: &[AgentState], b: &[AgentState]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.dedup_eq(y))
}

struct PublishState {
    last: Option<Vec<AgentState>>,
    last_ungated: Option<Vec<AgentState>>,
    revision: u64,
}

pub struct AgentBoardPublisher<R: tauri::Runtime = tauri::Wry> {
    app: tauri::AppHandle<R>,
    registry: AgentRegistryHandle,
    state: Arc<StdMutex<PublishState>>,
    health: Arc<super::health::HealthTracker>,
    runtimes_cfg: crate::config::AgentRuntimesConfig,
    board_show_working: bool,
    /// Live-session count mirror for the agent icon — stored UNGATED (before `gate_presence`) in
    /// `publish_if_changed`.
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
                last_ungated: None,
                revision: 0,
            })),
            health,
            runtimes_cfg,
            board_show_working,
            tab_wire,
        }
    }

    /// Increments the revision only when clock-independent session content changes.
    pub async fn publish_if_changed(&self, now: Instant) -> bool {
        let ungated = self.registry.ordered_states(now).await;
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
        let states = self.gate_presence(ungated.clone());
        // poison-tolerant, matching this codebase's other `StdMutex` guards — a panic elsewhere
        // while holding this lock must not permanently wedge every later publish attempt.
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let changed = match &guard.last_ungated {
            None => true,
            Some(prev) => !states_dedup_eq(prev, &ungated),
        };
        if !changed {
            return false;
        }
        guard.revision += 1;
        let revision = guard.revision;
        guard.last = Some(states.clone());
        guard.last_ungated = Some(ungated.clone());
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
            tab_sessions: ungated.iter().map(|s| to_view(s, now)).collect(),
            adapter_health,
        };
        if let Err(e) = self.app.emit(AGENT_STATE_EVENT, &snapshot) {
            tracing::error!("failed to emit agent-state: {e}");
        }
        true
    }

    /// The Agent Board's PRESENCE gate: agents that are merely working must not summon the Board.
    fn gate_presence(&self, states: Vec<AgentState>) -> Vec<AgentState> {
        if self.board_show_working || states.iter().any(|s| s.state.summons_board()) {
            states
        } else {
            Vec::new()
        }
    }

    pub fn last_session_count(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last
            .as_ref()
            .map(Vec::len)
            .unwrap_or(0)
    }

    pub fn spawn_tick(&self, interval: Duration) {
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                let now = Instant::now();
                this.registry.tick(now).await;
                this.publish_if_changed(now).await;
            }
        });
    }
}

/// Default interval for [`AgentBoardPublisher::spawn_tick`] — fine enough granularity for a
/// resting-card elapsed-time/stale sweep without being a busy poll.
pub const DEFAULT_TICK_INTERVAL: Duration = Duration::from_secs(5);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::model::{
        AgentEventKind, AgentRuntime, AgentSessionKey, AgentSessionState,
    };
    use crate::agents::registry::{AgentEvent, AgentRegistry};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn key(runtime: AgentRuntime, id: &str) -> AgentSessionKey {
        AgentSessionKey::new(runtime, id).unwrap()
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

    fn emitted_snapshots(
        app: &tauri::App<tauri::test::MockRuntime>,
    ) -> Arc<StdMutex<Vec<serde_json::Value>>> {
        use tauri::Listener;
        let seen = Arc::new(StdMutex::new(Vec::<serde_json::Value>::new()));
        let sink = seen.clone();
        app.handle().listen(AGENT_STATE_EVENT, move |event| {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                sink.lock().unwrap().push(value);
            }
        });
        seen
    }

    fn wire_states(snapshot: &serde_json::Value, field: &str) -> Vec<String> {
        snapshot[field]
            .as_array()
            .map(|list| {
                list.iter()
                    .map(|s| s["state"].as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default()
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
            tab_sessions: states.iter().map(|s| to_view(s, now)).collect(),
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
        assert_ne!(session["id"], "s1");
    }

    #[tokio::test]
    async fn snapshot_wire_shape_carries_bounded_history_oldest_first() {
        let app = tauri::test::mock_app();
        let publisher = publisher(&app);
        let base = Instant::now();
        let k = key(AgentRuntime::Codex, "s1");
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
            tab_sessions: states.iter().map(|s| to_view(s, base)).collect(),
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

        publisher
            .registry
            .apply_event(event(k, "e2", AgentEventKind::Informational), base)
            .await;
        assert!(publisher.publish_if_changed(base).await);
        assert_eq!(count.load(Ordering::SeqCst), 2);

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
        assert_eq!(publisher.last_session_count(), 0);

        assert!(publisher.publish_if_changed(base).await);
        assert_eq!(publisher.last_session_count(), 1);
    }

    fn working_event(session_key: AgentSessionKey, event_id: &str) -> AgentEvent {
        let mut e = event(session_key, event_id, AgentEventKind::Informational);
        e.declared_state = AgentSessionState::Working;
        e
    }

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
        assert_eq!(publisher.last_session_count(), 0);
    }

    #[tokio::test]
    async fn a_starting_or_stale_session_alone_does_not_summon_the_board_either() {
        let app = tauri::test::mock_app();
        let publisher = publisher_with(&app, false);
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
        publisher.publish_if_changed(base).await;
        assert!(published_states(&publisher).is_empty(), "starting alone");

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

        publisher
            .registry
            .apply_event(event(k, "e2", AgentEventKind::Informational), base)
            .await;
        assert!(publisher.publish_if_changed(base).await);
        assert!(published_states(&publisher).is_empty());
        assert_eq!(publisher.last_session_count(), 0);
    }

    #[tokio::test]
    async fn board_show_working_true_publishes_a_working_only_session() {
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
    async fn a_gated_off_board_republishes_working_changes_for_the_pull_surface_only() {
        let app = tauri::test::mock_app();
        let publisher = publisher_with(&app, false);
        let count = listen_count(&app);
        let snapshots = emitted_snapshots(&app);
        let base = Instant::now();
        publisher
            .registry
            .apply_event(working_event(key(AgentRuntime::Codex, "s1"), "e1"), base)
            .await;
        assert!(publisher.publish_if_changed(base).await);
        count.store(0, Ordering::SeqCst);
        snapshots.lock().unwrap().clear();

        publisher
            .registry
            .apply_event(working_event(key(AgentRuntime::Kimi, "s2"), "e2"), base)
            .await;
        assert!(publisher.publish_if_changed(base).await);
        assert_eq!(count.load(Ordering::SeqCst), 1);

        let seen = snapshots.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert!(
            wire_states(&seen[0], "sessions").is_empty(),
            "the gate is untouched: a working-only registry still publishes zero Board sessions"
        );
        assert_eq!(
            wire_states(&seen[0], "tabSessions"),
            vec!["working", "working"],
            "the pull surface sees both working sessions"
        );
        assert_eq!(publisher.last_session_count(), 0);
    }

    #[tokio::test]
    async fn a_clock_only_tick_still_does_not_publish_with_the_board_gated_off() {
        let app = tauri::test::mock_app();
        let publisher = publisher_with(&app, false);
        let count = listen_count(&app);
        let base = Instant::now();
        publisher
            .registry
            .apply_event(working_event(key(AgentRuntime::Codex, "s1"), "e1"), base)
            .await;
        assert!(publisher.publish_if_changed(base).await);
        count.store(0, Ordering::SeqCst);

        let later = base + Duration::from_secs(30);
        assert!(
            !publisher.publish_if_changed(later).await,
            "a clock-only tick must not publish, gated or not"
        );
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn the_summoned_board_publishes_the_same_slice_on_both_lists() {
        let app = tauri::test::mock_app();
        let publisher = publisher_with(&app, false);
        let snapshots = emitted_snapshots(&app);
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
        let seen = snapshots.lock().unwrap();
        let last = seen.last().unwrap();
        assert_eq!(
            wire_states(last, "sessions"),
            vec!["waiting_for_permission", "working"]
        );
        assert_eq!(
            wire_states(last, "tabSessions"),
            wire_states(last, "sessions")
        );
    }

    #[tokio::test]
    async fn a_working_only_registry_lights_the_icon_and_fills_the_pull_surface() {
        let app = tauri::test::mock_app();
        let publisher = publisher_with(&app, false);
        let snapshots = emitted_snapshots(&app);
        let base = Instant::now();
        publisher
            .registry
            .apply_event(working_event(key(AgentRuntime::Codex, "s1"), "e1"), base)
            .await;
        assert!(publisher.publish_if_changed(base).await);

        let icon_count = publisher
            .tab_wire
            .agent_sessions
            .load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(icon_count, 1, "the agent icon is lit");
        let seen = snapshots.lock().unwrap();
        assert_eq!(
            wire_states(seen.last().unwrap(), "tabSessions"),
            vec!["working"],
            "and the pull surface has something to render"
        );
    }
}
