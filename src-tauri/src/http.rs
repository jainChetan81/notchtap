use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::{EventError, QueueError};
use crate::event::{
    dispatch, emit_slot_state, Event, EventPayload, EventSignal, EventType, Priority, RotationSpec,
};
use crate::notifier::ConnectorHandle;
use crate::queue::SingleSlotQueue;

// generic over the tauri runtime so tests can use tauri::test::mock_app()
// (MockRuntime) while the app runs on the default Wry runtime
pub struct AppState<R: tauri::Runtime = tauri::Wry> {
    pub queue: Arc<Mutex<SingleSlotQueue>>,
    pub default_ttl: u64,
    pub app_handle: tauri::AppHandle<R>,
    pub connectors: Arc<Vec<ConnectorHandle>>,
}

impl<R: tauri::Runtime> Clone for AppState<R> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            default_ttl: self.default_ttl,
            app_handle: self.app_handle.clone(),
            connectors: self.connectors.clone(),
        }
    }
}

#[derive(Deserialize)]
struct NotifyRequest {
    title: Option<String>,
    body: Option<String>,
    priority: Option<Priority>,
    // non-`Option`, unlike `priority` — deliberate: sources that can't
    // know a specific signal (this endpoint's own CLI/cmux callers)
    // simply never set the field and get `Generic` via this default,
    // mirroring `presentation.rs`'s `DetectOutput` cutout-field pattern
    // rather than `priority`'s `unwrap_or` pattern in this same file.
    #[serde(default)]
    signal: EventSignal,
}

pub fn router<R: tauri::Runtime>(state: AppState<R>) -> Router {
    Router::new()
        .route("/notify", post(notify_handler::<R>))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(state)
}

/// Binds the listener. Loopback-only is a security boundary
/// (`ARCHITECTURE.md` §7): this is the single place a bind happens,
/// and it is hardcoded to 127.0.0.1 — no config field can widen it.
pub async fn bind_listener(port: u16) -> std::io::Result<tokio::net::TcpListener> {
    tokio::net::TcpListener::bind(("127.0.0.1", port)).await
}

async fn notify_handler<R: tauri::Runtime>(
    State(state): State<AppState<R>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, HttpError> {
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type.starts_with("application/json") {
        return Err(HttpError::BadRequest(
            "content-type must be application/json",
        ));
    }

    let req: NotifyRequest =
        serde_json::from_slice(&body).map_err(|_| HttpError::BadRequest("malformed json"))?;

    let title = req
        .title
        .ok_or(HttpError::Event(EventError::MissingField("title")))?;
    let body = req
        .body
        .ok_or(HttpError::Event(EventError::MissingField("body")))?;

    let event = Event {
        id: Uuid::new_v4(),
        event_type: EventType::Generic,
        priority: req.priority.unwrap_or(Priority::Medium),
        rotation: RotationSpec::OneShot {
            ttl_secs: state.default_ttl,
        },
        topic: None,
        payload: EventPayload { title, body },
        signal: req.signal,
    };

    dispatch(event.clone()).map_err(HttpError::Event)?;

    // kept for the connector fan-out below — it must only happen after
    // enqueue succeeds (v3 spec §1: rejected pushes reach no connector)
    let accepted = event.clone();

    let (slot_change, paused, waiting_count) = {
        let mut queue = state.queue.lock().await;
        queue.enqueue(event).map_err(HttpError::Queue)?;
        (
            queue.slot_state_if_changed(),
            queue.is_paused(),
            queue.total_waiting(),
        )
    };

    // acceptance fan-out (v3 spec §1): every enabled connector gets every
    // accepted event — including paused-202 ones (a paused overlay is
    // exactly when outbound matters most). never blocks, never affects
    // the http status.
    for connector in state.connectors.iter() {
        connector.offer(&accepted);
    }

    if let Some(new_state) = slot_change {
        emit_slot_state(&state.app_handle, new_state);
    }

    let response = if paused {
        (
            StatusCode::ACCEPTED,
            Json(json!({"status": "paused", "queued": waiting_count})),
        )
    } else {
        (StatusCode::OK, Json(json!({"status": "accepted"})))
    };

    Ok(response.into_response())
}

#[derive(Debug)]
enum HttpError {
    BadRequest(&'static str),
    Event(EventError),
    Queue(QueueError),
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            HttpError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.to_string()),
            HttpError::Event(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            HttpError::Queue(QueueError::QueueFull) => {
                (StatusCode::TOO_MANY_REQUESTS, "queue is full".to_string())
            }
        };
        Response::builder()
            .status(status)
            .body(Body::from(message))
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_state(queue: SingleSlotQueue) -> AppState<tauri::test::MockRuntime> {
        test_state_with_connectors(queue, Vec::new())
    }

    fn test_state_with_connectors(
        queue: SingleSlotQueue,
        connectors: Vec<ConnectorHandle>,
    ) -> AppState<tauri::test::MockRuntime> {
        let app = tauri::test::mock_app();
        AppState {
            queue: Arc::new(Mutex::new(queue)),
            default_ttl: 8,
            app_handle: app.handle().clone(),
            connectors: Arc::new(connectors),
        }
    }

    /// a connector whose receiving end the test holds, so fan-out can be
    /// asserted without any worker or network
    fn test_connector() -> (ConnectorHandle, tokio::sync::mpsc::Receiver<Event>) {
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        (ConnectorHandle::new("test", tx), rx)
    }

    fn json_request(body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/notify")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    async fn body_json(response: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn valid_post_returns_200_accepted() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_json(response).await, json!({"status": "accepted"}));
    }

    #[tokio::test]
    async fn paused_post_returns_202_with_queued_count() {
        let mut queue = SingleSlotQueue::new(50);
        queue.pause();
        let app = router(test_state(queue));
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(
            body_json(response).await,
            json!({"status": "paused", "queued": 1})
        );
    }

    #[tokio::test]
    async fn malformed_json_returns_400() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app.oneshot(json_request("{not json")).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn wrong_content_type_returns_400() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let request = Request::builder()
            .method("POST")
            .uri("/notify")
            .header("content-type", "text/plain")
            .body(Body::from(r#"{"title":"t","body":"b"}"#))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn missing_title_returns_400() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app.oneshot(json_request(r#"{"body":"b"}"#)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn missing_body_returns_400() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app.oneshot(json_request(r#"{"title":"t"}"#)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn full_queue_returns_429() {
        // per-tier cap 0: the first push still fast-path-promotes (nothing
        // waiting yet, nothing visible); the second push at the same tier
        // has nowhere to go, since the fast path only checks "is anything
        // waiting", not the per-tier cap.
        let app = router(test_state(SingleSlotQueue::new(0)));
        let first = app
            .clone()
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);

        let second = app
            .oneshot(json_request(r#"{"title":"t2","body":"b2"}"#))
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn full_queue_returns_429_while_paused() {
        // TESTING_STRATEGY.md §4.3: "still 429 when full while paused" —
        // pause buffers, it never lifts the max_queued_per_tier cap. paused
        // forces every push onto the waiting path (no fast path), so a
        // 0-per-tier cap rejects the very first push here, unlike the
        // non-paused variant above which needs a second push to see it.
        let mut queue = SingleSlotQueue::new(0);
        queue.pause();
        let app = router(test_state(queue));
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn oversized_body_returns_413() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let big = format!(r#"{{"title":"t","body":"{}"}}"#, "x".repeat(70 * 1024));
        let response = app.oneshot(json_request(&big)).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn listener_binds_loopback_only() {
        // the security-boundary test from TESTING_STRATEGY.md §4.3: a real
        // bind (port 0 = ephemeral), asserting the bound address is loopback.
        let listener = bind_listener(0).await.unwrap();
        let addr = listener.local_addr().unwrap();
        assert!(addr.ip().is_loopback());
    }

    #[tokio::test]
    async fn ok_and_paused_response_bodies_match_documented_shape() {
        // deserialize rather than substring-match so the contract is pinned
        // field-by-field.
        let app = router(test_state(SingleSlotQueue::new(50)));
        let ok_response = app
            .clone()
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(ok_response.status(), StatusCode::OK);
        let ok_body = body_json(ok_response).await;
        assert_eq!(ok_body["status"].as_str(), Some("accepted"));
        assert!(ok_body["queued"].is_null());

        let mut queue = SingleSlotQueue::new(50);
        queue.pause();
        let paused_app = router(test_state(queue));
        let paused_response = paused_app
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(paused_response.status(), StatusCode::ACCEPTED);
        let paused_body = body_json(paused_response).await;
        assert_eq!(paused_body["status"].as_str(), Some("paused"));
        assert_eq!(paused_body["queued"].as_u64(), Some(1));
    }

    #[tokio::test]
    async fn get_method_on_notify_is_rejected() {
        // only POST /notify is routed; axum rejects other methods with 405.
        let app = router(test_state(SingleSlotQueue::new(50)));
        let request = Request::builder()
            .method("GET")
            .uri("/notify")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    // --- v3 acceptance fan-out (spec §1 / TESTING_STRATEGY.md §4.9) ---

    #[tokio::test]
    async fn accepted_push_fans_out_to_connectors() {
        let (connector, mut rx) = test_connector();
        let app = router(test_state_with_connectors(
            SingleSlotQueue::new(50),
            vec![connector],
        ));
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let event = rx
            .try_recv()
            .expect("accepted event must reach the connector");
        assert_eq!(event.payload.title, "t");
    }

    #[tokio::test]
    async fn rejected_push_reaches_no_connector() {
        let (connector, mut rx) = test_connector();
        let app = router(test_state_with_connectors(
            SingleSlotQueue::new(0),
            vec![connector],
        ));
        let first = app
            .clone()
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        rx.try_recv().expect("first accepted push must fan out");

        let second = app
            .oneshot(json_request(r#"{"title":"t2","body":"b2"}"#))
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(rx.try_recv().is_err(), "429 must not fan out");
    }

    // --- v3.6 priority field (spec §3.3) ---

    #[tokio::test]
    async fn priority_field_defaults_to_medium_when_absent() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let req: NotifyRequest = serde_json::from_str(r#"{"title":"t","body":"b"}"#).unwrap();
        assert_eq!(req.priority, None); // absent on the wire
        assert_eq!(req.signal, EventSignal::Generic); // absent -> default, not None

        let mut queue = SingleSlotQueue::new(50);
        queue
            .enqueue(Event {
                id: Uuid::new_v4(),
                event_type: EventType::Generic,
                priority: req.priority.unwrap_or(Priority::Medium),
                rotation: RotationSpec::OneShot { ttl_secs: 8 },
                topic: None,
                payload: EventPayload {
                    title: "t".into(),
                    body: "b".into(),
                },
                signal: req.signal,
            })
            .unwrap();
        assert_eq!(queue.current_priority(), Some(Priority::Medium));
    }

    #[tokio::test]
    async fn explicit_priority_field_is_honored() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app
            .oneshot(json_request(
                r#"{"title":"t","body":"b","priority":"high"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let req: NotifyRequest =
            serde_json::from_str(r#"{"title":"t","body":"b","priority":"high"}"#).unwrap();
        assert_eq!(req.priority, Some(Priority::High));
    }

    // --- signal field (v3.6 EventSignal work) ---

    #[tokio::test]
    async fn signal_field_defaults_to_generic_when_absent() {
        let mut queue = SingleSlotQueue::new(50);
        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let req: NotifyRequest = serde_json::from_str(r#"{"title":"t","body":"b"}"#).unwrap();
        queue
            .enqueue(Event {
                id: Uuid::new_v4(),
                event_type: EventType::Generic,
                priority: req.priority.unwrap_or(Priority::Medium),
                rotation: RotationSpec::OneShot { ttl_secs: 8 },
                topic: None,
                payload: EventPayload {
                    title: "t".into(),
                    body: "b".into(),
                },
                signal: req.signal,
            })
            .unwrap();
        match queue.current_slot_state() {
            crate::event::SlotState::Showing { signal, .. } => {
                assert_eq!(signal, EventSignal::Generic)
            }
            other => panic!("expected Showing, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn explicit_signal_field_is_honored() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b","signal":"goal"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let req: NotifyRequest =
            serde_json::from_str(r#"{"title":"t","body":"b","signal":"goal"}"#).unwrap();
        assert_eq!(req.signal, EventSignal::Goal);
    }

    #[tokio::test]
    async fn malformed_signal_string_returns_400() {
        // proves rejection, not silent coercion to Generic — same rigor
        // as EventType's own unknown-string handling.
        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app
            .oneshot(json_request(
                r#"{"title":"t","body":"b","signal":"confetti"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn paused_202_push_still_fans_out() {
        // v3 spec §1: a paused overlay is exactly when outbound matters
        // most — acceptance succeeded, so connectors hear about it.
        let (connector, mut rx) = test_connector();
        let mut queue = SingleSlotQueue::new(50);
        queue.pause();
        let app = router(test_state_with_connectors(queue, vec![connector]));
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let event = rx
            .try_recv()
            .expect("paused-202 event must reach the connector");
        assert_eq!(event.payload.title, "t");
    }
}
