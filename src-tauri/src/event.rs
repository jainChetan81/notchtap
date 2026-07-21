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

/// Temperature display units for the weather source (plan 040 Part B).
/// Display-only: Open-Meteo does the conversion server-side via its
/// `temperature_unit` query param; alert thresholds are always stored
/// and compared in Celsius regardless of this value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Units {
    Celsius,
    Fahrenheit,
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
    Weather,
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

/// A single label/value pair rendered as one manifest cell (plan 035).
/// Display-only, like the rest of [`EventMeta`] — never consulted by
/// queue/rotation/priority logic. `details` come from untrusted hook
/// input, so the `/notify` handler caps label/value length (and the
/// pair count) before they land here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailItem {
    pub label: String,
    pub value: String,
}

/// News-source metadata (v5) plus the rich-relay fields (plan 035:
/// `subtitle`/`details`): the rss poller populates source/category/
/// published/link, and `/notify` callers populate subtitle/details;
/// every other source leaves them default. Presentation-only — never
/// consulted by queue/rotation/priority logic.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EventMeta {
    pub source: Option<String>,
    pub category: Option<String>,
    pub published_at_ms: Option<i64>,
    pub link: Option<String>,
    /// A first-class optional subtitle (plan 035): the CLI's `--subtitle`
    /// used to fold into the body CLI-side; it is now its own wire field.
    pub subtitle: Option<String>,
    /// Label/value detail pairs (plan 035), capped server-side.
    pub details: Vec<DetailItem>,
}

/// The rust-authoritative slot state pushed to the frontend whenever it
/// changes (promotion, rotation-to-empty, expand toggle). camelCase on
/// the wire so the TS `SlotState` type mirrors this shape exactly.
// `rename_all` alone only renames the variant tag ("Showing" -> "showing");
// struct-variant field names need `rename_all_fields` too, or `event_type`
// would serialize as-is instead of `eventType` (caught by
// slot_state_showing_serializes_camel_case_and_tag's own assertion).
// `Showing` is inherently large (it mirrors the whole wire payload) while
// `Empty` is a trivial sentinel — the asymmetry clippy's large_enum_variant
// flags is by design. Boxing wouldn't help honestly here: this enum is
// short-lived (built per emit, serialized, dropped), never stored in bulk,
// and boxing a field would only muddy the serde wire shape. Since plan 035
// added subtitle/details it crossed the 200-byte threshold, so allow it.
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
        expanded: bool,
        source: Option<String>,
        category: Option<String>,
        published_at_ms: Option<i64>,
        link: Option<String>,
        /// Rich-relay fields (plan 035), mirrored from `EventMeta` — the
        /// manifest renders `subtitle` as its own cell and one cell per
        /// `details` pair. Display-only, camelCase on the wire.
        subtitle: Option<String>,
        details: Vec<DetailItem>,
        /// Queue-slider position within the current batch (plan 033):
        /// `queue_total` is the batch size (never below 1 while Showing),
        /// `queue_done` how many items completed (capped at
        /// `queue_total - 1` so the current segment stays lit).
        queue_total: u32,
        queue_done: u32,
        /// Total rotation window for this showing, milliseconds — includes
        /// `extension_secs` and resolves OneShot ttl vs Recurring
        /// display_secs exactly as `rotation_window(self.window_expanded)`
        /// does (plan 081). Time-free: it only changes at discrete
        /// lifecycle moments (promotion, supersede top-up, manual expand),
        /// which is exactly what makes it safe to keep in the dedup
        /// comparison below — see `dedup_eq`.
        ttl_ms: u64,
        /// Milliseconds remaining until rotate-out AT EMISSION TIME,
        /// computed from the same `promoted_at` Instant math as
        /// `next_deadline` (saturating at 0). The frontend anchors its
        /// countdown at receipt — `Instant` isn't wall-clock and can't
        /// cross the wire, so remaining-at-emit + local elapsed is the
        /// honest shape. Deliberately EXCLUDED from the dedup comparison
        /// (`dedup_eq`) — it's a pure function of wall-clock time and so
        /// never matches between two calls even microseconds apart, which
        /// would defeat deduping entirely (plan 081 attempt 1 measured 2
        /// emissions per 1 `accept()` before this exclusion existed).
        remaining_ms: u64,
    },
}

impl SlotState {
    /// Dedup-only equality for `queue.rs::slot_state_if_changed`.
    ///
    /// Identical to the derived `PartialEq` above EXCEPT it ignores
    /// `remaining_ms`, which is a pure function of `Instant::now()` and can
    /// never be equal between two calls even milliseconds apart — see the
    /// rotation loop's post-wake recheck in `engine.rs::spawn_rotation`,
    /// which re-locks the queue and calls `slot_state_if_changed()` again
    /// after every mutation's own emit. Without this exclusion that second,
    /// structurally-unavoidable call always sees a "changed" state (because
    /// `remaining_ms` ticked down) and always re-emits, doubling emission
    /// volume system-wide (plan 081 attempt 1's finding).
    ///
    /// `ttl_ms` stays IN the comparison on purpose: it only changes at real
    /// lifecycle events (promotion, supersede extension, manual expand),
    /// exactly the moments a fresh emission is wanted, so keeping it means
    /// the gate still fires — with a fresh `remaining_ms` — right when the
    /// bar needs to re-anchor.
    ///
    /// This does NOT replace the derived `PartialEq`, which stays intact
    /// and honest — several tests assert full `SlotState` equality
    /// (`assert_eq!`), and a hand-rolled `PartialEq` that quietly ignored a
    /// field would make those assertions silently meaningless. Any future
    /// continuously-varying wire field (see the module doc's "maintenance
    /// notes" pointer) must extend this method, not `PartialEq`.
    pub(crate) fn dedup_eq(&self, other: &SlotState) -> bool {
        fn normalized(s: &SlotState) -> SlotState {
            let mut s = s.clone();
            if let SlotState::Showing { remaining_ms, .. } = &mut s {
                *remaining_ms = 0;
            }
            s
        }
        normalized(self) == normalized(other)
    }
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
            (SourceKind::Weather, "weather"),
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
        // plan 035: subtitle is a first-class string field; details is an
        // array of {label, value} pairs (own field names, not camelCased).
        assert_eq!(json["subtitle"], "Permission request");
        assert_eq!(json["details"][0]["label"], "Tool");
        assert_eq!(json["details"][0]["value"], "Bash");
        assert_eq!(json["details"][1]["label"], "Command");
        assert_eq!(json["details"][1]["value"], "git push");
        assert_eq!(json["queueTotal"], 5);
        assert_eq!(json["queueDone"], 2);
        // plan 081: timing fields, camelCase, milliseconds.
        assert_eq!(json["ttlMs"], 8000);
        assert_eq!(json["remainingMs"], 6000);
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
        };

        let json = serde_json::to_value(state).unwrap();
        assert!(json["source"].is_null());
        assert!(json["category"].is_null());
        assert!(json["publishedAtMs"].is_null());
        assert!(json["link"].is_null());
        // plan 081: timing fields are always present (never optional).
        assert_eq!(json["ttlMs"], 4000);
        assert_eq!(json["remainingMs"], 4000);
        // plan 035: absent subtitle serializes null; absent details is an
        // empty array (never null/absent), so the frontend can map over it.
        assert!(json["subtitle"].is_null());
        assert_eq!(json["details"], serde_json::json!([]));
        assert_eq!(json["queueTotal"], 1);
        assert_eq!(json["queueDone"], 0);
    }

    #[test]
    fn detail_item_round_trips_with_own_field_names() {
        // plan 035: DetailItem fields are `label`/`value` on the wire (no
        // camelCase rename — the frontend reads exactly these keys).
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
        // back-compat: every non-relay source leaves the plan-035 fields at
        // their default (None / empty), covered by EventMeta's derived Default.
        let meta = EventMeta::default();
        assert_eq!(meta.subtitle, None);
        assert!(meta.details.is_empty());
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

/// Shared test fixture builder (plan 028): the ONE place tests build
/// `Event`s, so a new field is a one-file test change. Production code
/// must never use this — `#[cfg(test)]` enforces it.
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

    // add more with_* combinators ONLY if the sweep actually needs one that
    // isn't here — no speculative API beyond what the sweep below uses.
}
