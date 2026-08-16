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
    /// Plan 171 (tab-notch): live Agent Session count, sourced from the
    /// Agent Board publisher's own recompute (an `AtomicUsize` mirror —
    /// see `AgentBoardPublisher::publish_if_changed`), NOT a second
    /// registry read. Drives the agent icon's present/live tiers.
    pub agent: AgentStatus,
    pub football: FootballStatus,
    pub news: NewsStatus,
}

/// Plan 171: the agent icon's presence source. One field for now —
/// present iff `active_sessions > 0` (an agent icon has no separate
/// "present but idle" tier: a registered live session IS liveness).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub active_sessions: usize,
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
    /// Plan 171 (tab-notch, spec §8): the news-charge cycle, sourced
    /// from `news_charge.rs`'s state machine (owned by `lib.rs`, fed by
    /// `rss_poller.rs`). `charge_fraction` is `fill()` (0..=1),
    /// `charge_count` is items waiting, `is_charged` is the edge-held
    /// "cycle ended with a full batch" flag cleared on visit.
    pub charge_fraction: f32,
    pub charge_count: usize,
    pub is_charged: bool,
}

/// Named-field inputs for [`StatusState::snapshot`] — replaces five
/// positional bool/Option arguments (three same-typed `bool`s, two
/// same-shaped `Option`s) that a future call-site edit could transpose
/// without a compile error. Construct with field names, not
/// positionally, at every call site.
pub struct StatusInputs {
    pub live: Option<LiveMatchSummary>,
    pub espn_enabled: bool,
    pub rss_enabled: bool,
    /// Plan 171: live Agent Session count (Agent Board's atomic mirror).
    pub agent_sessions: usize,
    /// Plan 171: the news-charge snapshot `(fill, count, is_charged)`,
    /// read from `news_charge.rs` under its own lock by the caller.
    pub news_charge: (f32, usize, bool),
}

impl StatusState {
    /// Recomputed from the live handles on every heartbeat pass; cheap
    /// (two queue reads + a clone) so the change-guard below is what keeps
    /// the channel silent at steady state, not any caching here.
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
/// Plan 171 §0: `tab-selection-changed`, `{ selected: "agent" | … |
/// "news" | null }`, emitted on actual transitions only (`last` is the
/// last value actually put on the wire, not the last computed). Called
/// from every path that can move the selection: the click monitor, the
/// prefix keymap, and the engine loop's liveness clearing.
///
/// **The emit happens INSIDE the `last` guard, and that differs
/// deliberately from `emit_hover_changed_if_transitioned`, which this
/// otherwise mirrors.** Hover has exactly ONE writer (the AppKit
/// tracking-area handlers, all on the main thread), so unlocking before
/// emitting cannot reorder anything there. Tab selection has TWO: the
/// click/prefix path on the AppKit main thread, and the engine's status
/// loop on a tokio worker. With two writers, unlock-then-emit can put the
/// OLDER payload on the wire LAST (both threads swap the guard, then race
/// to `emit`), and the mismatch STICKS — the next call compares against
/// the newer guard value and returns early, so nothing ever corrects the
/// frontend. Holding the lock across the emit makes wire order a total
/// order under this mutex. It adds no blocking edge: `app.emit` is a
/// synchronous, non-blocking post (see `engine.rs`'s `apply`, which emits
/// under its own queue lock for exactly this reason), this mutex is the
/// only lock this function takes, and no rust-side listener subscribes to
/// `tab-selection-changed` — do not add an `.await` inside this block.
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
        test_fixtures::event("t")
    }

    #[test]
    fn snapshot_reads_pause_and_waiting_from_the_queue() {
        let mut queue = SingleSlotQueue::new(50);
        queue
            .enqueue(generic_event(), std::time::Instant::now())
            .unwrap(); // unpaused: promotes
        queue.pause();

        // one item visible, nothing waiting, paused, no live match
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

        // paused pushes buffer instead of promoting (v5 semantics)
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
