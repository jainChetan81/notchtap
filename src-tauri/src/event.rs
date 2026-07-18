use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub event_type: EventType,
    pub priority: Priority,
    pub rotation: RotationSpec,
    pub topic: Option<String>,
    pub payload: EventPayload,
    #[serde(default)]
    pub meta: EventMeta,
    pub signal: EventSignal,
    /// Which source produced this event (v6: rotation-order tie-break) —
    /// orthogonal to `Priority`, which still decides cross-tier order
    /// first. Always server-assigned, never accepted from the `/notify`
    /// wire (same rule as `rotation`/`topic`).
    pub origin: SourceKind,
}

impl Event {
    pub fn rotation_window(&self, expanded: bool) -> u64 {
        let base = match self.rotation {
            RotationSpec::OneShot { ttl_secs } => ttl_secs,
            RotationSpec::Recurring { display_secs } => display_secs,
        };
        if expanded {
            base * EXPANDED_MULTIPLIER
        } else {
            base
        }
    }
}

pub const EXPANDED_MULTIPLIER: u64 = 3;

/// Event type on the `/notify` wire, snake_case per the v1 spec §7.
/// Unknown types are rejected at deserialization — never silently
/// coerced to [`EventType::Generic`]:
///
/// ```
/// use notchtap_lib::event::EventType;
///
/// let t: EventType = serde_json::from_str(r#""score_update""#).unwrap();
/// assert!(matches!(t, EventType::ScoreUpdate));
///
/// assert!(serde_json::from_str::<EventType>(r#""posture_alert""#).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Generic,
    ScoreUpdate,
    MatchState,
    NewsItem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Medium,
    High,
}

/// The source that produced an [`Event`] (v6: `Config.rotation_order`
/// tie-break). A closed set, same rigor as [`EventType`]/[`EventSignal`] —
/// unknown values are rejected at deserialization, never silently coerced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Football,
    News,
    Manual,
    Cmux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RotationSpec {
    OneShot { ttl_secs: u64 },
    Recurring { display_secs: u64 },
}

/// Which icon/animation the frontend plays — orthogonal to [`EventType`]
/// and [`Priority`]: this never touches queue/rotation/priority
/// semantics, it's presentation-only. Unknown values are rejected at
/// deserialization, same rigor as `EventType`:
///
/// ```
/// use notchtap_lib::event::EventSignal;
///
/// let s: EventSignal = serde_json::from_str(r#""goal""#).unwrap();
/// assert!(matches!(s, EventSignal::Goal));
///
/// assert!(serde_json::from_str::<EventSignal>(r#""confetti""#).is_err());
/// ```
///
/// Sources that can't know a specific signal (the CLI, cmux) omit the
/// field on the wire and get `Generic` via `#[serde(default)]` on the
/// containing struct — see `http.rs`'s `NotifyRequest`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventSignal {
    #[default]
    Generic,
    Goal,
    RedCard,
    YellowCard,
    Kickoff,
    Halftime,
    Fulltime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    pub title: String,
    pub body: String,
}

/// News-source metadata (v5): populated only by the rss poller; every
/// other source leaves it default. Presentation-only — never consulted
/// by queue/rotation/priority logic.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EventMeta {
    pub source: Option<String>,
    pub category: Option<String>,
    pub published_at_ms: Option<i64>,
    pub link: Option<String>,
}

/// The rust-authoritative slot state pushed to the frontend whenever it
/// changes (promotion, rotation-to-empty, expand toggle). camelCase on
/// the wire so the TS `SlotState` type mirrors this shape exactly.
// `rename_all` alone only renames the variant tag ("Showing" -> "showing");
// struct-variant field names need `rename_all_fields` too, or `event_type`
// would serialize as-is instead of `eventType` (caught by
// slot_state_showing_serializes_camel_case_and_tag's own assertion).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[serde(tag = "state")]
pub enum SlotState {
    Empty,
    Showing {
        id: Uuid,
        title: String,
        body: String,
        event_type: EventType,
        priority: Priority,
        signal: EventSignal,
        expanded: bool,
        source: Option<String>,
        category: Option<String>,
        published_at_ms: Option<i64>,
        link: Option<String>,
    },
}

/// The one event channel into the overlay — the frontend listens for
/// exactly this string (`src/useSlotState.ts`). Change both together.
pub const SLOT_STATE_EVENT: &str = "slot-state";

/// The single emit path (spec §5.1): one `slot-state` event whenever the
/// displayed slot changes. Emit failure is logged, never propagated — by
/// this point the queue state has already changed, so failing the caller
/// would report a notification as lost when it may still display.
pub fn emit_slot_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>, state: SlotState) {
    use tauri::Emitter;
    if let Err(e) = app.emit(SLOT_STATE_EVENT, &state) {
        tracing::error!("failed to emit slot-state: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::EventError;

    #[test]
    fn slot_state_event_name_is_pinned() {
        // The frontend listens for exactly this literal (src/useSlotState.ts).
        // A rename on either side compiles clean and passes every other test,
        // shipping an overlay that never updates — this pins the seam so a
        // rename fails loudly and names the file that must change in lockstep.
        assert_eq!(SLOT_STATE_EVENT, "slot-state");
    }

    #[test]
    fn unknown_type_string_is_rejected_at_deserialization() {
        // score_update and match_state are real variants now; pick something
        // that is not a variant to exercise the unknown-type path.
        let result: Result<EventType, _> = serde_json::from_str(r#""posture_alert""#);
        assert!(result.is_err());
    }

    #[test]
    fn score_update_deserializes() {
        let event_type: EventType = serde_json::from_str(r#""score_update""#).unwrap();
        assert!(matches!(event_type, EventType::ScoreUpdate));
    }

    #[test]
    fn match_state_deserializes() {
        let event_type: EventType = serde_json::from_str(r#""match_state""#).unwrap();
        assert!(matches!(event_type, EventType::MatchState));
    }

    #[test]
    fn news_item_deserializes() {
        let event_type: EventType = serde_json::from_str(r#""news_item""#).unwrap();
        assert!(matches!(event_type, EventType::NewsItem));
    }

    #[test]
    fn news_item_serializes_snake_case() {
        let json = serde_json::to_value(EventType::NewsItem).unwrap();
        assert_eq!(json, "news_item");
    }

    #[test]
    fn priority_ord_is_low_lt_medium_lt_high() {
        assert!(Priority::Low < Priority::Medium && Priority::Medium < Priority::High);
    }

    #[test]
    fn source_kind_round_trips_every_variant() {
        for (kind, wire) in [
            (SourceKind::Football, "football"),
            (SourceKind::News, "news"),
            (SourceKind::Manual, "manual"),
            (SourceKind::Cmux, "cmux"),
        ] {
            assert_eq!(serde_json::to_value(kind).unwrap(), wire);
            let parsed: SourceKind = serde_json::from_str(&format!("\"{wire}\"")).unwrap();
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn unknown_source_kind_is_rejected_at_deserialization() {
        assert!(serde_json::from_str::<SourceKind>(r#""telegram""#).is_err());
    }

    #[test]
    fn event_signal_default_is_generic() {
        assert_eq!(EventSignal::default(), EventSignal::Generic);
    }

    #[test]
    fn event_signal_round_trips_every_variant() {
        for (signal, wire) in [
            (EventSignal::Generic, "generic"),
            (EventSignal::Goal, "goal"),
            (EventSignal::RedCard, "red_card"),
            (EventSignal::YellowCard, "yellow_card"),
            (EventSignal::Kickoff, "kickoff"),
            (EventSignal::Halftime, "halftime"),
            (EventSignal::Fulltime, "fulltime"),
        ] {
            assert_eq!(serde_json::to_value(signal).unwrap(), wire);
            let parsed: EventSignal = serde_json::from_str(&format!("\"{wire}\"")).unwrap();
            assert_eq!(parsed, signal);
        }
    }

    #[test]
    fn slot_state_showing_serializes_camel_case_and_tag() {
        let id = Uuid::new_v4();
        let state = SlotState::Showing {
            id,
            title: "GOAL".to_string(),
            body: "1-0".to_string(),
            event_type: EventType::ScoreUpdate,
            priority: Priority::High,
            signal: EventSignal::Goal,
            expanded: false,
            source: Some("NDTV".to_string()),
            category: Some("politics".to_string()),
            published_at_ms: Some(1_789_600_000_000),
            link: Some("https://example.com/story".to_string()),
        };
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["state"], "showing");
        assert_eq!(json["id"], serde_json::to_value(id).unwrap());
        assert_eq!(json["title"], "GOAL");
        assert_eq!(json["body"], "1-0");
        assert_eq!(json["eventType"], "score_update");
        assert_eq!(json["priority"], "high");
        assert_eq!(json["signal"], "goal");
        assert_eq!(json["expanded"], false);
        assert_eq!(json["source"], "NDTV");
        assert_eq!(json["category"], "politics");
        assert_eq!(json["publishedAtMs"], 1_789_600_000_000_i64);
        assert_eq!(json["link"], "https://example.com/story");
        assert!(json.get("event_type").is_none());
        assert!(json.get("published_at_ms").is_none());
        assert!(json.get("ttlSecs").is_none());
    }

    #[test]
    fn slot_state_showing_without_metadata_serializes_null_fields() {
        let state = SlotState::Showing {
            id: Uuid::new_v4(),
            title: "Status".to_string(),
            body: "No news metadata".to_string(),
            event_type: EventType::Generic,
            priority: Priority::Medium,
            signal: EventSignal::Generic,
            expanded: false,
            source: None,
            category: None,
            published_at_ms: None,
            link: None,
        };

        let json = serde_json::to_value(state).unwrap();
        assert!(json["source"].is_null());
        assert!(json["category"].is_null());
        assert!(json["publishedAtMs"].is_null());
        assert!(json["link"].is_null());
    }

    #[test]
    fn slot_state_empty_serializes_to_tag_only() {
        let json = serde_json::to_value(&SlotState::Empty).unwrap();
        assert_eq!(json, serde_json::json!({"state": "empty"}));
    }

    #[test]
    fn rotation_window_doubles_when_expanded() {
        let event = Event {
            id: Uuid::new_v4(),
            event_type: EventType::Generic,
            priority: Priority::Medium,
            rotation: RotationSpec::OneShot { ttl_secs: 4 },
            topic: None,
            payload: EventPayload {
                title: "t".to_string(),
                body: "b".to_string(),
            },
            meta: EventMeta::default(),
            signal: EventSignal::Generic,
            origin: SourceKind::Manual,
        };
        assert_eq!(event.rotation_window(false), 4);
        assert_eq!(event.rotation_window(true), 12);
    }

    #[test]
    fn event_error_messages_name_the_field() {
        let err = EventError::MissingField("title");
        assert_eq!(err.to_string(), "missing required field: title");
    }
}
