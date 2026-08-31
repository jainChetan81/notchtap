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
    /// Always server-assigned, never accepted from the `/notify` wire (same rule as
    /// `rotation`/`topic`).
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

/// Unknown types are rejected at deserialization — never silently coerced to
/// [`EventType::Generic`]:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Generic,
    ScoreUpdate,
    MatchState,
    NewsItem,
    AgentEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Medium,
    High,
}

/// A closed set, same rigor as [`EventType`]/[`EventSignal`] — unknown values are rejected at
/// deserialization, never silently coerced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Football,
    News,
    Manual,
    /// `#[serde(alias = "cmux")]` accepts the legacy literal `"cmux"` as an alternate spelling on
    /// deserialization only — serialization always writes `"agent"`.
    #[serde(alias = "cmux")]
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RotationSpec {
    OneShot { ttl_secs: u64 },
    Recurring { display_secs: u64 },
}

/// Which icon/animation the frontend plays — orthogonal to [`EventType`] and [`Priority`]: this
/// never touches queue/rotation/priority semantics, it's presentation-only.
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
    Foul,
    Offside,
    VarCheck,
    Substitution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    pub title: String,
    pub body: String,
}

/// Display-only, like the rest of [`EventMeta`] — never consulted by queue/rotation/priority logic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailItem {
    pub label: String,
    pub value: String,
}

/// `session_key` is deliberately NOT the raw `AgentSessionKey`: raw provider identity must never be
/// persisted, and this struct rides on `Event`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSignal {
    pub runtime: String,
    pub kind: String,
    pub session_hash: String,
    /// Already sanitized/capped upstream by `agents::adapter::parse_wire_event` — this struct never
    /// re-derives or further truncates it.
    pub summary: Option<String>,
}

/// Presentation-only — never consulted by queue/rotation/priority logic.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EventMeta {
    pub source: Option<String>,
    pub category: Option<String>,
    pub published_at_ms: Option<i64>,
    pub link: Option<String>,
    pub subtitle: Option<String>,
    pub details: Vec<DetailItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub espn: Option<EspnMeta>,
    /// `skip_serializing_if` for the same reason as `espn` above — every non-agent payload must
    /// keep an identical wire shape, no literal `"agent": null` key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentSignal>,
}

/// Display-only, like the rest of `EventMeta` — never consulted by queue/rotation/priority logic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EspnMeta {
    pub league: String,
    pub home_abbrev: String,
    pub away_abbrev: String,
    pub home_score: u32,
    pub away_score: u32,
    pub clock: String,
    pub home_cards: (u32, u32),
    pub away_cards: (u32, u32),
    pub home_crest: Option<String>,
    pub away_crest: Option<String>,
}

/// The rust-authoritative slot state pushed to the frontend whenever it changes (promotion,
/// rotation-to-empty, expand toggle).
// `Showing` is inherently large (it mirrors the whole wire payload) while
// `Empty` is a trivial sentinel — the asymmetry clippy's large_enum_variant
// flags is by design. Boxing wouldn't help honestly here: this enum is
// short-lived (built per emit, serialized, dropped), never stored in bulk,
// and boxing a field would only muddy the serde wire shape.
#[allow(clippy::large_enum_variant)]
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
        /// Time-invariant: it's assigned once when the event is accepted and never changes for that
        /// item's lifetime.
        origin: SourceKind,
        expanded: bool,
        source: Option<String>,
        category: Option<String>,
        published_at_ms: Option<i64>,
        link: Option<String>,
        subtitle: Option<String>,
        details: Vec<DetailItem>,
        #[serde(skip_serializing_if = "Option::is_none")]
        espn: Option<EspnMeta>,
        /// Queue-slider position within the current batch: `queue_total` is the batch size (never
        /// below 1 while Showing).
        queue_total: u32,
        queue_done: u32,
        ttl_ms: u64,
        /// Deliberately EXCLUDED from the dedup comparison (`dedup_eq`) — it's a pure function of
        /// wall-clock time and so never matches between two calls even microseconds apart.
        remaining_ms: u64,
        agent_runtime: Option<String>,
    },
}

impl SlotState {
    /// Dedup-only equality for `queue.rs::slot_state_if_changed`. Identical to the derived
    /// `PartialEq` above EXCEPT it ignores `remaining_ms`.
    pub(crate) fn dedup_eq(&self, other: &SlotState) -> bool {
        fn normalized(s: &SlotState) -> SlotState {
            let mut s = s.clone();
            match &mut s {
                SlotState::Empty => {}
                SlotState::Showing {
                    id: _,
                    title: _,
                    body: _,
                    event_type: _,
                    priority: _,
                    signal: _,
                    origin: _,
                    expanded: _,
                    source: _,
                    category: _,
                    published_at_ms: _,
                    link: _,
                    subtitle: _,
                    details: _,
                    espn: _,
                    queue_total: _,
                    queue_done: _,
                    ttl_ms: _,
                    remaining_ms,
                    agent_runtime: _,
                } => {
                    *remaining_ms = 0;
                }
            }
            s
        }
        normalized(self) == normalized(other)
    }
}

/// The one event channel into the overlay — the frontend listens for exactly this string
/// (`src/useSlotState.ts`). Change both together.
pub const SLOT_STATE_EVENT: &str = "slot-state";

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
        assert_eq!(SLOT_STATE_EVENT, "slot-state");
    }

    #[test]
    fn unknown_type_string_is_rejected_at_deserialization() {
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
            (SourceKind::Agent, "agent"),
        ] {
            assert_eq!(serde_json::to_value(kind).unwrap(), wire);
            let parsed: SourceKind = serde_json::from_str(&format!("\"{wire}\"")).unwrap();
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn unknown_source_kind_is_rejected_at_deserialization() {
        assert!(serde_json::from_str::<SourceKind>(r#""pigeon""#).is_err());
    }

    #[test]
    fn legacy_cmux_string_deserializes_as_agent_but_never_serializes_back() {
        let parsed: SourceKind = serde_json::from_str(r#""cmux""#).unwrap();
        assert_eq!(parsed, SourceKind::Agent);
        assert_eq!(serde_json::to_value(parsed).unwrap(), "agent");
        assert_ne!(serde_json::to_value(parsed).unwrap(), "cmux");
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
            (EventSignal::Foul, "foul"),
            (EventSignal::Offside, "offside"),
            (EventSignal::VarCheck, "var_check"),
            (EventSignal::Substitution, "substitution"),
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
            origin: SourceKind::Manual,
            expanded: false,
            source: Some("NDTV".to_string()),
            category: Some("politics".to_string()),
            published_at_ms: Some(1_789_600_000_000),
            link: Some("https://example.com/story".to_string()),
            subtitle: Some("Permission request".to_string()),
            details: vec![
                DetailItem {
                    label: "Tool".to_string(),
                    value: "Bash".to_string(),
                },
                DetailItem {
                    label: "Command".to_string(),
                    value: "git push".to_string(),
                },
            ],
            queue_total: 5,
            queue_done: 2,
            ttl_ms: 8000,
            remaining_ms: 6000,
            espn: None,
            agent_runtime: Some("claude-code".to_string()),
        };
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["state"], "showing");
        assert_eq!(json["id"], serde_json::to_value(id).unwrap());
        assert_eq!(json["title"], "GOAL");
        assert_eq!(json["body"], "1-0");
        assert_eq!(json["eventType"], "score_update");
        assert_eq!(json["priority"], "high");
        assert_eq!(json["signal"], "goal");
        assert_eq!(json["origin"], "manual");
        assert_eq!(json["expanded"], false);
        assert_eq!(json["source"], "NDTV");
        assert_eq!(json["category"], "politics");
        assert_eq!(json["publishedAtMs"], 1_789_600_000_000_i64);
        assert_eq!(json["link"], "https://example.com/story");
        assert_eq!(json["subtitle"], "Permission request");
        assert_eq!(json["details"][0]["label"], "Tool");
        assert_eq!(json["details"][0]["value"], "Bash");
        assert_eq!(json["details"][1]["label"], "Command");
        assert_eq!(json["details"][1]["value"], "git push");
        assert_eq!(json["queueTotal"], 5);
        assert_eq!(json["queueDone"], 2);
        assert_eq!(json["ttlMs"], 8000);
        assert_eq!(json["remainingMs"], 6000);
        assert_eq!(json["agentRuntime"], "claude-code");
        assert!(json.get("event_type").is_none());
        assert!(json.get("published_at_ms").is_none());
        assert!(json.get("queue_total").is_none());
        assert!(json.get("ttl_ms").is_none());
        assert!(json.get("remaining_ms").is_none());
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
            origin: SourceKind::Manual,
            expanded: false,
            source: None,
            category: None,
            published_at_ms: None,
            link: None,
            subtitle: None,
            details: Vec::new(),
            queue_total: 1,
            queue_done: 0,
            ttl_ms: 4000,
            remaining_ms: 4000,
            espn: None,
            agent_runtime: None,
        };

        let json = serde_json::to_value(state).unwrap();
        assert!(json["source"].is_null());
        assert!(json["category"].is_null());
        assert!(json["publishedAtMs"].is_null());
        assert!(json["link"].is_null());
        assert!(json["agentRuntime"].is_null());
        assert_eq!(json["ttlMs"], 4000);
        assert_eq!(json["remainingMs"], 4000);
        assert!(json["subtitle"].is_null());
        assert_eq!(json["details"], serde_json::json!([]));
        assert_eq!(json["queueTotal"], 1);
        assert_eq!(json["queueDone"], 0);
        assert!(
            !json.as_object().unwrap().contains_key("espn"),
            "absent espn must be an omitted key, not a null value"
        );
    }

    #[test]
    fn espn_meta_serializes_camel_case_when_present() {
        let state = SlotState::Showing {
            id: Uuid::new_v4(),
            title: "UCL: ARS 1–1 PSG".to_string(),
            body: "kickoff".to_string(),
            event_type: EventType::MatchState,
            priority: Priority::High,
            signal: EventSignal::Kickoff,
            origin: SourceKind::Football,
            expanded: false,
            source: None,
            category: None,
            published_at_ms: None,
            link: None,
            subtitle: None,
            details: Vec::new(),
            queue_total: 1,
            queue_done: 0,
            ttl_ms: 8000,
            remaining_ms: 8000,
            espn: Some(EspnMeta {
                league: "UCL".to_string(),
                home_abbrev: "PSG".to_string(),
                away_abbrev: "ARS".to_string(),
                home_score: 1,
                away_score: 1,
                clock: "45'".to_string(),
                home_cards: (2, 0),
                away_cards: (4, 0),
                home_crest: Some("/home/u/.config/notchtap/crests/160.png".to_string()),
                away_crest: None,
            }),
            agent_runtime: None,
        };
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["espn"]["league"], "UCL");
        assert_eq!(json["espn"]["homeAbbrev"], "PSG");
        assert_eq!(json["espn"]["awayAbbrev"], "ARS");
        assert_eq!(json["espn"]["homeScore"], 1);
        assert_eq!(json["espn"]["awayScore"], 1);
        assert_eq!(json["espn"]["clock"], "45'");
        assert_eq!(json["espn"]["homeCards"], serde_json::json!([2, 0]));
        assert_eq!(json["espn"]["awayCards"], serde_json::json!([4, 0]));
        assert_eq!(
            json["espn"]["homeCrest"],
            "/home/u/.config/notchtap/crests/160.png"
        );
        assert!(json["espn"]["awayCrest"].is_null());
        assert!(json.get("home_abbrev").is_none());
    }

    #[test]
    fn detail_item_round_trips_with_own_field_names() {
        let item = DetailItem {
            label: "Command".to_string(),
            value: "git push origin master".to_string(),
        };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"label": "Command", "value": "git push origin master"})
        );
        let parsed: DetailItem = serde_json::from_value(json).unwrap();
        assert_eq!(parsed, item);
    }

    #[test]
    fn event_meta_default_has_no_subtitle_and_empty_details() {
        let meta = EventMeta::default();
        assert_eq!(meta.subtitle, None);
        assert!(meta.details.is_empty());
        assert_eq!(meta.espn, None);
    }

    #[test]
    fn event_meta_espn_field_is_omitted_from_wire_when_absent() {
        let json = serde_json::to_value(EventMeta::default()).unwrap();
        assert!(
            !json.as_object().unwrap().contains_key("espn"),
            "absent espn must be an omitted key, not a null value"
        );
        assert!(json["subtitle"].is_null());
    }

    #[test]
    fn dedup_eq_treats_a_changed_espn_block_as_a_real_change() {
        fn showing_with_score(home_score: u32) -> SlotState {
            SlotState::Showing {
                id: Uuid::nil(),
                title: "t".to_string(),
                body: "b".to_string(),
                event_type: EventType::ScoreUpdate,
                priority: Priority::High,
                signal: EventSignal::Goal,
                origin: SourceKind::Football,
                expanded: false,
                source: None,
                category: None,
                published_at_ms: None,
                link: None,
                subtitle: None,
                details: Vec::new(),
                queue_total: 1,
                queue_done: 0,
                ttl_ms: 8000,
                remaining_ms: 8000,
                espn: Some(EspnMeta {
                    league: "UCL".to_string(),
                    home_abbrev: "PSG".to_string(),
                    away_abbrev: "ARS".to_string(),
                    home_score,
                    away_score: 1,
                    clock: "45'".to_string(),
                    home_cards: (0, 0),
                    away_cards: (0, 0),
                    home_crest: None,
                    away_crest: None,
                }),
                agent_runtime: None,
            }
        }

        let before = showing_with_score(1);
        let after_same_score = showing_with_score(1);
        let after_new_goal = showing_with_score(2);

        assert!(
            before.dedup_eq(&after_same_score),
            "identical espn blocks must still dedup"
        );
        assert!(
            !before.dedup_eq(&after_new_goal),
            "a changed espn block (new goal) must NOT be deduped away"
        );
    }

    #[test]
    fn dedup_eq_treats_a_changed_origin_as_a_real_change() {
        fn showing_with_origin(origin: SourceKind) -> SlotState {
            SlotState::Showing {
                id: Uuid::nil(),
                title: "t".to_string(),
                body: "b".to_string(),
                event_type: EventType::Generic,
                priority: Priority::Medium,
                signal: EventSignal::Generic,
                origin,
                expanded: false,
                source: None,
                category: None,
                published_at_ms: None,
                link: None,
                subtitle: None,
                details: Vec::new(),
                queue_total: 1,
                queue_done: 0,
                ttl_ms: 8000,
                remaining_ms: 8000,
                espn: None,
                agent_runtime: None,
            }
        }

        let before = showing_with_origin(SourceKind::Manual);
        let after_same_origin = showing_with_origin(SourceKind::Manual);
        let after_new_origin = showing_with_origin(SourceKind::News);

        assert!(
            before.dedup_eq(&after_same_origin),
            "identical origin must still dedup"
        );
        assert!(
            !before.dedup_eq(&after_new_origin),
            "a changed origin must NOT be deduped away"
        );
    }

    #[test]
    fn dedup_eq_treats_a_changed_agent_runtime_as_a_real_change() {
        fn showing_with_agent_runtime(agent_runtime: Option<&str>) -> SlotState {
            SlotState::Showing {
                id: Uuid::nil(),
                title: "t".to_string(),
                body: "b".to_string(),
                event_type: EventType::AgentEvent,
                priority: Priority::Medium,
                signal: EventSignal::Generic,
                origin: SourceKind::Agent,
                expanded: false,
                source: None,
                category: None,
                published_at_ms: None,
                link: None,
                subtitle: None,
                details: Vec::new(),
                queue_total: 1,
                queue_done: 0,
                ttl_ms: 8000,
                remaining_ms: 8000,
                espn: None,
                agent_runtime: agent_runtime.map(|s| s.to_string()),
            }
        }

        let before = showing_with_agent_runtime(Some("claude-code"));
        let after_same = showing_with_agent_runtime(Some("claude-code"));
        let after_new = showing_with_agent_runtime(Some("codex"));

        assert!(
            before.dedup_eq(&after_same),
            "identical agent_runtime must still dedup"
        );
        assert!(
            !before.dedup_eq(&after_new),
            "a changed agent_runtime must NOT be deduped away"
        );
    }

    #[test]
    fn event_meta_deserializes_subtitle_and_details_from_wire() {
        let meta: EventMeta = serde_json::from_value(serde_json::json!({
            "subtitle": "Permission request",
            "details": [{"label": "Tool", "value": "Bash"}]
        }))
        .unwrap();
        assert_eq!(meta.subtitle.as_deref(), Some("Permission request"));
        assert_eq!(meta.details.len(), 1);
        assert_eq!(meta.details[0].label, "Tool");
        assert_eq!(meta.details[0].value, "Bash");
    }

    #[test]
    fn slot_state_empty_serializes_to_tag_only() {
        let json = serde_json::to_value(&SlotState::Empty).unwrap();
        assert_eq!(json, serde_json::json!({"state": "empty"}));
    }

    #[test]
    fn rotation_window_doubles_when_expanded() {
        let event = test_fixtures::with_rotation(
            test_fixtures::event("t"),
            RotationSpec::OneShot { ttl_secs: 4 },
        );
        assert_eq!(event.rotation_window(false), 4);
        assert_eq!(event.rotation_window(true), 12);
    }

    #[test]
    fn event_error_messages_name_the_field() {
        let err = EventError::MissingField("title");
        assert_eq!(err.to_string(), "missing required field: title");
    }
}

#[cfg(test)]
pub(crate) mod test_fixtures {
    use super::*;

    pub(crate) fn event(title: &str) -> Event {
        Event {
            id: Uuid::new_v4(),
            event_type: EventType::Generic,
            priority: Priority::Medium,
            rotation: RotationSpec::OneShot { ttl_secs: 8 },
            topic: None,
            payload: EventPayload {
                title: title.to_string(),
                body: "body".to_string(),
            },
            meta: EventMeta::default(),
            signal: EventSignal::Generic,
            origin: SourceKind::Manual,
        }
    }

    pub(crate) fn with_priority(mut e: Event, priority: Priority) -> Event {
        e.priority = priority;
        e
    }

    pub(crate) fn with_rotation(mut e: Event, rotation: RotationSpec) -> Event {
        e.rotation = rotation;
        e
    }

    pub(crate) fn with_topic(mut e: Event, topic: &str) -> Event {
        e.topic = Some(topic.to_string());
        e
    }

    pub(crate) fn with_origin(mut e: Event, origin: SourceKind) -> Event {
        e.origin = origin;
        e
    }

    pub(crate) fn with_event_type(mut e: Event, event_type: EventType) -> Event {
        e.event_type = event_type;
        e
    }

    pub(crate) fn with_signal(mut e: Event, signal: EventSignal) -> Event {
        e.signal = signal;
        e
    }

    pub(crate) fn with_body(mut e: Event, body: &str) -> Event {
        e.payload.body = body.to_string();
        e
    }
}
