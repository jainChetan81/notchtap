//! Idle source-status rail (plan 034): one combined `status-state` event
//! answering the idle card's "what's happening / what's next" — the boot
//! source gates, the queue depth behind the empty slot, and the one live
//! watched football match. Delivery duplicates the slot-state pattern
//! exactly: the rust core emits on change and plants
//! `window.__NOTCHTAP_STATUS_STATE__` on page load (lib.rs). The overlay
//! stays receive-only — this is a listen-only channel, no invoke.

use serde::Serialize;

use crate::queue::SingleSlotQueue;

/// The status channel into the overlay — the frontend listens for exactly
/// this string (`src/useStatusState.ts`). Change both together.
pub const STATUS_STATE_EVENT: &str = "status-state";

/// camelCase on the wire so the TS `StatusState` type mirrors this shape
/// exactly (same convention as `SlotState`, event.rs).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusState {
    pub paused: bool,
    pub waiting: usize,
    pub football: FootballStatus,
    pub news: NewsStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FootballStatus {
    pub enabled: bool,
    /// `None` when no watched match is in-play (serializes as `null`).
    pub live: Option<LiveMatchSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveMatchSummary {
    /// "Home X–Y Away" (poller.rs builds it from the tracked snapshot).
    pub label: String,
    /// espn's own clock text ("45'"), carried verbatim.
    pub minute: String,
}

/// "News paused" in the idle rail means `enabled == false`: the polling
/// gates are boot-config since v6, so there is no runtime poll pause to
/// report beyond the gate itself.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsStatus {
    pub enabled: bool,
}

impl StatusState {
    /// Recomputed from the live handles on every heartbeat pass; cheap
    /// (two queue reads + a clone) so the change-guard below is what keeps
    /// the channel silent at steady state, not any caching here.
    pub fn snapshot(
        queue: &SingleSlotQueue,
        live: Option<LiveMatchSummary>,
        espn_enabled: bool,
        rss_enabled: bool,
    ) -> Self {
        Self {
            paused: queue.is_paused(),
            waiting: queue.total_waiting(),
            football: FootballStatus {
                enabled: espn_enabled,
                live,
            },
            news: NewsStatus {
                enabled: rss_enabled,
            },
        }
    }
}

/// The change-guard. Unlike `slot_state_if_changed` (queue-owned), the
/// previous state is a `last_status` local in the heartbeat task — the
/// heartbeat is the sole emitter, so there is exactly one guard and no
/// second writer can desync it (plan 034 step 3).
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

/// The single emit path, mirroring `emit_slot_state`: emit failure is
/// logged, never propagated — by this point the state has already changed,
/// so failing the caller would misreport the underlying mutation.
pub fn emit_status_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>, state: StatusState) {
    use tauri::Emitter;
    if let Err(e) = app.emit(STATUS_STATE_EVENT, &state) {
        tracing::error!("failed to emit status-state: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{
        Event, EventMeta, EventPayload, EventSignal, EventType, Priority, RotationSpec, SourceKind,
    };

    fn live_summary() -> LiveMatchSummary {
        LiveMatchSummary {
            label: "Arsenal 2–0 Chelsea".to_string(),
            minute: "45'".to_string(),
        }
    }

    fn status(live: Option<LiveMatchSummary>) -> StatusState {
        StatusState {
            paused: false,
            waiting: 3,
            football: FootballStatus {
                enabled: true,
                live,
            },
            news: NewsStatus { enabled: true },
        }
    }

    #[test]
    fn status_state_event_name_is_pinned() {
        // The frontend listens for exactly this literal
        // (src/useStatusState.ts). A rename on either side compiles clean
        // and passes every other test, shipping a rail that never updates
        // — same reasoning as SLOT_STATE_EVENT's pin in event.rs.
        assert_eq!(STATUS_STATE_EVENT, "status-state");
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
    fn serializes_live_as_null_when_nothing_in_play() {
        let json = serde_json::to_value(status(None)).unwrap();
        assert!(json["football"]["live"].is_null());
    }

    #[test]
    fn change_guard_emits_once_then_stays_silent_until_a_real_change() {
        let mut last = None;
        // first sighting always emits (the page-load seed's dual-path
        // shield relies on the heartbeat's first pass emitting too)
        assert_eq!(
            status_state_if_changed(&mut last, status(None)),
            Some(status(None))
        );
        // identical recompute: silent
        assert_eq!(status_state_if_changed(&mut last, status(None)), None);
        // any field change (here: a match goes live) emits again
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
        Event {
            id: uuid::Uuid::new_v4(),
            event_type: EventType::Generic,
            priority: Priority::Medium,
            rotation: RotationSpec::OneShot { ttl_secs: 8 },
            topic: None,
            payload: EventPayload {
                title: "t".to_string(),
                body: "b".to_string(),
            },
            meta: EventMeta::default(),
            signal: EventSignal::Generic,
            origin: SourceKind::Manual,
        }
    }

    #[test]
    fn snapshot_reads_pause_and_waiting_from_the_queue() {
        let mut queue = SingleSlotQueue::new(50);
        queue.enqueue(generic_event()).unwrap(); // unpaused: promotes
        queue.pause();

        // one item visible, nothing waiting, paused, no live match
        let snap = StatusState::snapshot(&queue, None, true, false);
        assert!(snap.paused);
        assert_eq!(snap.waiting, 0);
        assert!(snap.football.enabled);
        assert_eq!(snap.football.live, None);
        assert!(!snap.news.enabled);

        // paused pushes buffer instead of promoting (v5 semantics)
        queue.enqueue(generic_event()).unwrap();
        assert_eq!(StatusState::snapshot(&queue, None, true, false).waiting, 1);
    }
}
