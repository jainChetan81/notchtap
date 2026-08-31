use serde::Serialize;

use crate::queue::SingleSlotQueue;

/// The status channel into the overlay — the frontend listens for exactly this string
/// (`src/useStatusState.ts`). Change both together.
pub const STATUS_STATE_EVENT: &str = "status-state";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusState {
    pub paused: bool,
    pub waiting: usize,
    pub agent: AgentStatus,
    pub football: FootballStatus,
    pub news: NewsStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub active_sessions: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FootballStatus {
    pub enabled: bool,
    pub live: Option<LiveMatchSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveMatchSummary {
    pub label: String,
    pub minute: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsStatus {
    pub enabled: bool,
    pub charge_fraction: f32,
    pub charge_count: usize,
    pub is_charged: bool,
}

/// Named-field inputs for [`StatusState::snapshot`].
pub struct StatusInputs {
    pub live: Option<LiveMatchSummary>,
    pub espn_enabled: bool,
    pub rss_enabled: bool,
    pub agent_sessions: usize,
    pub news_charge: (f32, usize, bool),
}

impl StatusState {
    /// Recomputed from the live handles on every heartbeat pass; cheap (two queue reads + a clone)
    /// so the change-guard below is what keeps the channel silent at steady state.
    pub fn snapshot(queue: &SingleSlotQueue, inputs: StatusInputs) -> Self {
        Self {
            paused: queue.is_paused(),
            waiting: queue.total_waiting(),
            agent: AgentStatus {
                active_sessions: inputs.agent_sessions,
            },
            football: FootballStatus {
                enabled: inputs.espn_enabled,
                live: inputs.live,
            },
            news: NewsStatus {
                enabled: inputs.rss_enabled,
                charge_fraction: inputs.news_charge.0,
                charge_count: inputs.news_charge.1,
                is_charged: inputs.news_charge.2,
            },
        }
    }
}

pub fn status_state_if_changed(
    last: &mut Option<StatusState>,
    next: StatusState,
) -> Option<StatusState> {
    if last.as_ref() == Some(&next) {
        None
    } else {
        *last = Some(next.clone());
        Some(next)
    }
}

pub fn emit_tab_selection_if_transitioned<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    last: &std::sync::Mutex<Option<crate::tabs::Tab>>,
    selected: Option<crate::tabs::Tab>,
) {
    use tauri::Emitter;
    let mut guard = last.lock().unwrap_or_else(|e| e.into_inner());
    if *guard == selected {
        return;
    }
    *guard = selected;
    let payload = serde_json::json!({
        "selected": selected.map(crate::tabs::Tab::wire_label),
    });
    if let Err(e) = app.emit("tab-selection-changed", payload) {
        tracing::error!("failed to emit tab-selection-changed: {e}");
    }
}

pub fn emit_status_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>, state: StatusState) {
    use tauri::Emitter;
    if let Err(e) = app.emit(STATUS_STATE_EVENT, &state) {
        tracing::error!("failed to emit status-state: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{test_fixtures, Event};

    fn live_summary() -> LiveMatchSummary {
        LiveMatchSummary {
            label: "Arsenal 2–0 Chelsea".to_string(),
            minute: "45'".to_string(),
        }
    }

    fn status(live: Option<LiveMatchSummary>) -> StatusState {
        StatusState {
            agent: AgentStatus { active_sessions: 0 },
            paused: false,
            waiting: 3,
            football: FootballStatus {
                enabled: true,
                live,
            },
            news: NewsStatus {
                enabled: true,
                charge_fraction: 0.0,
                charge_count: 0,
                is_charged: false,
            },
        }
    }

    #[test]
    fn serializes_camel_case_with_live_match() {
        let json = serde_json::to_value(status(Some(live_summary()))).unwrap();
        assert_eq!(json["paused"], false);
        assert_eq!(json["waiting"], 3);
        assert_eq!(json["football"]["enabled"], true);
        assert_eq!(json["football"]["live"]["label"], "Arsenal 2–0 Chelsea");
        assert_eq!(json["football"]["live"]["minute"], "45'");
        assert_eq!(json["news"]["enabled"], true);
    }

    #[test]
    fn change_guard_emits_once_then_stays_silent_until_a_real_change() {
        let mut last = None;
        assert_eq!(
            status_state_if_changed(&mut last, status(None)),
            Some(status(None))
        );
        assert_eq!(status_state_if_changed(&mut last, status(None)), None);
        assert_eq!(
            status_state_if_changed(&mut last, status(Some(live_summary()))),
            Some(status(Some(live_summary())))
        );
        assert_eq!(
            status_state_if_changed(&mut last, status(Some(live_summary()))),
            None
        );
    }

    fn generic_event() -> Event {
        test_fixtures::event("t")
    }

    #[test]
    fn snapshot_reads_pause_and_waiting_from_the_queue() {
        let mut queue = SingleSlotQueue::new(50);
        queue
            .enqueue(generic_event(), std::time::Instant::now())
            .unwrap(); // unpaused: promotes
        queue.pause();

        let snap = StatusState::snapshot(
            &queue,
            StatusInputs {
                live: None,
                espn_enabled: true,
                rss_enabled: false,
                agent_sessions: 0,
                news_charge: (0.0, 0, false),
            },
        );
        assert!(snap.paused);
        assert_eq!(snap.waiting, 0);
        assert!(snap.football.enabled);
        assert_eq!(snap.football.live, None);
        assert!(!snap.news.enabled);

        queue
            .enqueue(generic_event(), std::time::Instant::now())
            .unwrap();
        assert_eq!(
            StatusState::snapshot(
                &queue,
                StatusInputs {
                    live: None,
                    espn_enabled: true,
                    rss_enabled: false,
                    agent_sessions: 0,
                    news_charge: (0.0, 0, false),
                },
            )
            .waiting,
            1
        );
    }
}
