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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Generic,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPayload {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub ttl_secs: u64,
}

impl From<&Event> for NotificationPayload {
    fn from(event: &Event) -> Self {
        Self {
            id: event.id,
            title: event.payload.title.clone(),
            body: event.payload.body.clone(),
            ttl_secs: event.ttl_secs,
        }
    }
}

pub fn dispatch(event: Event) -> Result<(), EventError> {
    match event.event_type {
        EventType::Generic => Ok(()),
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
        // v1 has one variant; an unknown `type` can only arrive as data,
        // and serde rejects it before dispatch ever sees it
        let result: Result<EventType, _> = serde_json::from_str(r#""score_update""#);
        assert!(result.is_err());
    }

    #[test]
    fn wire_payload_uses_camel_case_ttl_secs() {
        let event = generic_event();
        let payload = NotificationPayload::from(&event);
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["title"], "t");
        assert_eq!(json["body"], "b");
        assert_eq!(json["ttlSecs"], 8);
        assert!(json.get("ttl_secs").is_none());
    }

    #[test]
    fn event_error_messages_name_the_field() {
        let err = EventError::MissingField("title");
        assert_eq!(err.to_string(), "missing required field: title");
    }
}
