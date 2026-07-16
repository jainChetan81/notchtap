use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::EventError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub event_type: EventType,
    pub priority: Priority,
    pub ttl_secs: u64,
    pub payload: EventPayload,
}

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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Generic,
    ScoreUpdate,
    MatchState,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Normal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    pub title: String,
    pub body: String,
}

/// The `notification-promoted` payload the frontend receives — camelCase
/// on the wire (the frontend's `NotificationPayload` type mirrors this
/// shape, v2 spec §5):
///
/// ```
/// use notchtap_lib::event::{Event, EventPayload, EventType, NotificationPayload, Priority};
///
/// let event = Event {
///     id: uuid::Uuid::new_v4(),
///     event_type: EventType::ScoreUpdate,
///     priority: Priority::Normal,
///     ttl_secs: 8,
///     payload: EventPayload { title: "GOAL".into(), body: "1-0".into() },
/// };
///
/// let json = serde_json::to_value(NotificationPayload::from(&event)).unwrap();
/// assert_eq!(json["ttlSecs"], 8);              // camelCase, not ttl_secs
/// assert_eq!(json["eventType"], "score_update"); // value stays snake_case
/// ```
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPayload {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub ttl_secs: u64,
    pub event_type: EventType,
}

impl From<&Event> for NotificationPayload {
    fn from(event: &Event) -> Self {
        Self {
            id: event.id,
            title: event.payload.title.clone(),
            body: event.payload.body.clone(),
            ttl_secs: event.ttl_secs,
            event_type: event.event_type.clone(),
        }
    }
}

pub fn dispatch(event: Event) -> Result<(), EventError> {
    match event.event_type {
        EventType::Generic | EventType::ScoreUpdate | EventType::MatchState => Ok(()),
    }
}

/// The single emit path (spec §4's emit rule): one `notification-promoted`
/// event per promoted item, wherever the promotion happened (enqueue
/// fast-path, heartbeat tick, or resume). Emit failure is logged, never
/// propagated — by this point the item is already promoted, so failing the
/// caller would report a notification as lost when it may still display.
pub fn emit_promoted<R: tauri::Runtime>(app: &tauri::AppHandle<R>, promoted: Vec<Event>) {
    use tauri::Emitter;
    for event in promoted {
        let payload = NotificationPayload::from(&event);
        if let Err(e) = app.emit("notification-promoted", &payload) {
            tracing::error!("failed to emit notification-promoted: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generic_event() -> Event {
        Event {
            id: Uuid::new_v4(),
            event_type: EventType::Generic,
            priority: Priority::Normal,
            ttl_secs: 8,
            payload: EventPayload {
                title: "t".to_string(),
                body: "b".to_string(),
            },
        }
    }

    #[test]
    fn generic_event_dispatches_ok() {
        assert!(dispatch(generic_event()).is_ok());
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
    fn dispatch_accepts_all_three_variants() {
        for event_type in [
            EventType::Generic,
            EventType::ScoreUpdate,
            EventType::MatchState,
        ] {
            let mut event = generic_event();
            event.event_type = event_type;
            assert!(dispatch(event).is_ok());
        }
    }

    #[test]
    fn wire_payload_uses_camel_case_ttl_secs_and_event_type() {
        let event = generic_event();
        let payload = NotificationPayload::from(&event);
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["title"], "t");
        assert_eq!(json["body"], "b");
        assert_eq!(json["ttlSecs"], 8);
        assert_eq!(json["eventType"], "generic");
        assert!(json.get("ttl_secs").is_none());
        assert!(json.get("event_type").is_none());
    }

    #[test]
    fn event_error_messages_name_the_field() {
        let err = EventError::MissingField("title");
        assert_eq!(err.to_string(), "missing required field: title");
    }
}
