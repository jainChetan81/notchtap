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
use uuid::Uuid;

use crate::engine::Engine;
use crate::error::{EventError, QueueError};
use crate::event::{
    DetailItem, Event, EventMeta, EventPayload, EventSignal, EventType, Priority, RotationSpec,
    SourceKind,
};

// generic over the tauri runtime so tests can use tauri::test::mock_app()
// (MockRuntime) while the app runs on the default Wry runtime
pub struct AppState<R: tauri::Runtime = tauri::Wry> {
    /// plan 037: the one propagation module — ingest goes through
    /// `Engine::accept`, the paused/waiting response reads through
    /// `Engine::read`.
    pub engine: Engine<R>,
    pub default_ttl: u64,
    /// v6: the `/notify` fallback when a request omits its own `priority`
    /// (`Config.manual_default_priority`, default `Medium`) — a request
    /// that sets `priority` explicitly still overrides this.
    pub manual_default_priority: Priority,
    /// v6.1: same fallback role as `manual_default_priority`, but for a
    /// request that self-identifies as `source: "cmux"`.
    pub cmux_priority: Priority,
    /// v6.1: rotation window for a cmux-originated request, mirroring
    /// `default_ttl`'s role for a plain manual one.
    pub cmux_ttl_secs: u64,
}

impl<R: tauri::Runtime> Clone for AppState<R> {
    fn clone(&self) -> Self {
        Self {
            engine: self.engine.clone(),
            default_ttl: self.default_ttl,
            manual_default_priority: self.manual_default_priority,
            cmux_priority: self.cmux_priority,
            cmux_ttl_secs: self.cmux_ttl_secs,
        }
    }
}

/// The one origin a `/notify` caller may self-declare (v6.1) — a closed,
/// single-variant set, deliberately not the full `SourceKind`:
/// `Football`/`News` must never be wire-claimable, since only the
/// ESPN/RSS pollers may legitimately produce those. Set by the `notchtap`
/// CLI's `--source cmux` (explicit or auto-detected from
/// `CMUX_NOTIFICATION_BODY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RequestSource {
    Cmux,
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
    source: Option<RequestSource>,
    // plan 035: a first-class optional subtitle (no longer folded into the
    // body CLI-side) and optional label/value detail pairs. Both are
    // `Option` — a missing field deserializes to `None` (serde special-cases
    // `Option`), so old payloads that set neither stay byte-identical. Both
    // are capped/sanitized (see `sanitize_subtitle`/`sanitize_details`)
    // before they reach `EventMeta`, since `details` is untrusted hook input.
    subtitle: Option<String>,
    details: Option<Vec<DetailItem>>,
}

/// Display-safety caps for the plan-035 rich-relay fields (decision 4):
/// the manifest lives in a fixed 500×300 window, so subtitle/detail text
/// is bounded here — the server is the trust boundary. The hooks truncate
/// earlier as a courtesy, never as the guarantee; if the window ever
/// grows, revisit these numbers, not the mechanism.
const SUBTITLE_MAX_CHARS: usize = 120;
const DETAILS_MAX_PAIRS: usize = 8;
const DETAIL_LABEL_MAX_CHARS: usize = 40;
const DETAIL_VALUE_MAX_CHARS: usize = 200;
// title/body are the two required fields on every request — the same
// display-safety rationale as the subtitle/detail caps above applies
// (fixed 500×300 window), just sized a little larger since title/body
// are the primary content rather than supplementary meta. Only the
// overall 64 KiB body limit bounded these before; an unbounded single
// field could still blow the layout even under that cap.
const TITLE_MAX_CHARS: usize = 200;
const BODY_MAX_CHARS: usize = 500;

/// Truncates to at most `max_chars` characters (not bytes — never splits a
/// UTF-8 codepoint), appending an ellipsis only when truncation happened.
fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars).collect();
        out.push('…');
        out
    }
}

/// An empty subtitle collapses to `None`; anything longer than the cap is
/// truncated with an ellipsis.
fn sanitize_subtitle(subtitle: Option<String>) -> Option<String> {
    subtitle
        .filter(|s| !s.is_empty())
        .map(|s| truncate_with_ellipsis(&s, SUBTITLE_MAX_CHARS))
}

/// Drops pairs with an empty label, keeps at most `DETAILS_MAX_PAIRS`
/// (dropping happens first, so the cap counts only non-empty-label pairs),
/// and truncates each label/value to its cap.
fn sanitize_details(details: Option<Vec<DetailItem>>) -> Vec<DetailItem> {
    details
        .unwrap_or_default()
        .into_iter()
        .filter(|d| !d.label.is_empty())
        .take(DETAILS_MAX_PAIRS)
        .map(|d| DetailItem {
            label: truncate_with_ellipsis(&d.label, DETAIL_LABEL_MAX_CHARS),
            value: truncate_with_ellipsis(&d.value, DETAIL_VALUE_MAX_CHARS),
        })
        .collect()
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
        tracing::warn!(%content_type, "notify: rejected — content-type must be application/json");
        return Err(HttpError::BadRequest(
            "content-type must be application/json",
        ));
    }

    let req: NotifyRequest = serde_json::from_slice(&body).map_err(|e| {
        tracing::warn!(error = %e, "notify: rejected — malformed json");
        HttpError::BadRequest("malformed json")
    })?;

    let title = req.title.ok_or_else(|| {
        tracing::warn!(field = "title", "notify: rejected — missing field");
        HttpError::Event(EventError::MissingField("title"))
    })?;
    let title = truncate_with_ellipsis(&title, TITLE_MAX_CHARS);
    let body = req.body.ok_or_else(|| {
        tracing::warn!(field = "body", "notify: rejected — missing field");
        HttpError::Event(EventError::MissingField("body"))
    })?;
    let body = truncate_with_ellipsis(&body, BODY_MAX_CHARS);

    let (origin, default_priority, ttl_secs) = match req.source {
        Some(RequestSource::Cmux) => (SourceKind::Cmux, state.cmux_priority, state.cmux_ttl_secs),
        None => (
            SourceKind::Manual,
            state.manual_default_priority,
            state.default_ttl,
        ),
    };

    // plan 035: subtitle/details are the only meta a `/notify` caller may
    // set (source/category/published/link stay poller-only); both are
    // sanitized/capped here — this is the trust boundary for hook input.
    let meta = EventMeta {
        subtitle: sanitize_subtitle(req.subtitle),
        details: sanitize_details(req.details),
        ..EventMeta::default()
    };

    let event = Event {
        id: Uuid::new_v4(),
        event_type: EventType::Generic,
        priority: req.priority.unwrap_or(default_priority),
        rotation: RotationSpec::OneShot { ttl_secs },
        topic: None,
        payload: EventPayload { title, body },
        meta,
        signal: req.signal,
        origin,
    };

    state
        .engine
        .accept(event, false)
        .await
        .map_err(HttpError::Queue)?;

    let (paused, waiting_count) = state
        .engine
        .read(|q| (q.is_paused(), q.total_waiting()))
        .await;

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
    use crate::event::test_fixtures;
    use crate::notifier::{ConnectorHandle, ConnectorHealth};
    use crate::queue::SingleSlotQueue;
    use axum::http::Request;
    use std::sync::Arc;
    use std::time::Instant;
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
            engine: Engine::new(
                queue,
                app.handle().clone(),
                Arc::new(connectors),
                Arc::new(std::sync::Mutex::new(ConnectorHealth::default())),
                true,
                true,
                false,
                false,
                None,
            ),
            default_ttl: 8,
            manual_default_priority: Priority::Medium,
            cmux_priority: Priority::High,
            cmux_ttl_secs: 8,
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
            .enqueue(
                test_fixtures::with_signal(
                    test_fixtures::with_priority(
                        test_fixtures::event("t"),
                        req.priority.unwrap_or(Priority::Medium),
                    ),
                    req.signal,
                ),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(queue.current_priority(), Some(Priority::Medium));
    }

    #[tokio::test]
    async fn manual_default_priority_drives_the_absent_field_fallback() {
        // v6: the fallback used to be the hardcoded Priority::Medium; now
        // it's state.manual_default_priority (Config.manual_default_priority).
        let mut state = test_state(SingleSlotQueue::new(50));
        state.manual_default_priority = Priority::Low;
        let app = router(state.clone());
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            state.engine.read(|q| q.current_priority()).await,
            Some(Priority::Low)
        );
    }

    #[tokio::test]
    async fn explicit_priority_field_overrides_manual_default_priority() {
        let mut state = test_state(SingleSlotQueue::new(50));
        state.manual_default_priority = Priority::Low;
        let app = router(state.clone());
        let response = app
            .oneshot(json_request(
                r#"{"title":"t","body":"b","priority":"high"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            state.engine.read(|q| q.current_priority()).await,
            Some(Priority::High)
        );
    }

    // --- v6.1 cmux source field ---

    #[tokio::test]
    async fn cmux_source_uses_cmux_default_priority_not_manual() {
        let mut state = test_state(SingleSlotQueue::new(50));
        state.manual_default_priority = Priority::Low;
        state.cmux_priority = Priority::High;
        let app = router(state.clone());
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b","source":"cmux"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            state.engine.read(|q| q.current_priority()).await,
            Some(Priority::High)
        );
    }

    #[tokio::test]
    async fn absent_source_still_uses_manual_default_priority() {
        let mut state = test_state(SingleSlotQueue::new(50));
        state.manual_default_priority = Priority::Low;
        state.cmux_priority = Priority::High;
        let app = router(state.clone());
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            state.engine.read(|q| q.current_priority()).await,
            Some(Priority::Low)
        );
    }

    #[tokio::test]
    async fn explicit_priority_field_overrides_cmux_priority() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let response = app
            .oneshot(json_request(
                r#"{"title":"t","body":"b","source":"cmux","priority":"low"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            state.engine.read(|q| q.current_priority()).await,
            Some(Priority::Low)
        );
    }

    #[tokio::test]
    async fn unknown_source_string_returns_400() {
        // proves rejection, not silent coercion to a known source — same
        // rigor as the signal/priority fields' own unknown-string handling.
        // Football/News specifically must never be wire-claimable even
        // though they're valid SourceKind values elsewhere in the app.
        let app = router(test_state(SingleSlotQueue::new(50)));
        for source in ["football", "news", "manual", "telegram"] {
            let response = app
                .clone()
                .oneshot(json_request(&format!(
                    r#"{{"title":"t","body":"b","source":"{source}"}}"#
                )))
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::BAD_REQUEST,
                "{source:?} must be rejected"
            );
        }
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
            .enqueue(
                test_fixtures::with_signal(
                    test_fixtures::with_priority(
                        test_fixtures::event("t"),
                        req.priority.unwrap_or(Priority::Medium),
                    ),
                    req.signal,
                ),
                Instant::now(),
            )
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

    // --- §9.2 (docs/TESTING_STRATEGY.md) — burst and boundary cases ---
    //
    // Retargeted from the pre-v3.6 max_concurrent/max_queued framing to
    // today's single-slot-plus-per-tier-cap model: only one item is ever
    // visible, so "burst" here means bursting one priority tier's
    // `waiting` up to and past its `max_queued_per_tier` cap.

    #[tokio::test]
    async fn burst_to_tier_cap_boundary_accepts_exactly_cap_plus_one() {
        // cap 5: the first push fast-path-promotes to visible (nothing
        // waiting yet), the next 5 land in waiting up to the cap, and the
        // remaining 2 have nowhere to go. 8 posts total: 6x 200, 2x 429.
        let app = router(test_state(SingleSlotQueue::new(5)));
        let mut accepted = 0;
        let mut rejected = 0;
        for i in 0..8 {
            let response = app
                .clone()
                .oneshot(json_request(&format!(
                    r#"{{"title":"t{i}","body":"b{i}"}}"#
                )))
                .await
                .unwrap();
            match response.status() {
                StatusCode::OK => accepted += 1,
                StatusCode::TOO_MANY_REQUESTS => rejected += 1,
                other => panic!("unexpected status {other}"),
            }
        }
        assert_eq!(accepted, 6, "1 visible + 5 waiting = 6 accepted");
        assert_eq!(rejected, 2);
    }

    #[tokio::test]
    async fn paused_burst_to_tier_cap_boundary_accepts_exactly_cap() {
        // paused from the start: no fast path, every push goes straight to
        // waiting. cap 5, 8 posts: 5x 202 then 3x 429, nothing visible.
        let mut queue = SingleSlotQueue::new(5);
        queue.pause();
        let app = router(test_state(queue));
        let mut accepted = 0;
        let mut rejected = 0;
        for i in 0..8 {
            let response = app
                .clone()
                .oneshot(json_request(&format!(
                    r#"{{"title":"t{i}","body":"b{i}"}}"#
                )))
                .await
                .unwrap();
            match response.status() {
                StatusCode::ACCEPTED => accepted += 1,
                StatusCode::TOO_MANY_REQUESTS => rejected += 1,
                other => panic!("unexpected status {other}"),
            }
        }
        assert_eq!(
            accepted, 5,
            "exactly the per-tier cap accepted while paused"
        );
        assert_eq!(rejected, 3);
    }

    #[tokio::test]
    async fn boundary_body_size_exactly_at_limit_is_accepted() {
        // pin the exact 64 KiB DefaultBodyLimit boundary, not just a
        // grossly oversized body (oversized_body_returns_413 above).
        let limit = 64 * 1024;
        let overhead = r#"{"title":"t","body":""}"#.len();
        let pad = limit - overhead;
        let body = format!(r#"{{"title":"t","body":"{}"}}"#, "x".repeat(pad));
        assert_eq!(
            body.len(),
            limit,
            "test body must land exactly at the limit"
        );

        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app.oneshot(json_request(&body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn boundary_body_size_one_byte_over_limit_returns_413() {
        let limit = 64 * 1024;
        let overhead = r#"{"title":"t","body":""}"#.len();
        let pad = limit - overhead + 1;
        let body = format!(r#"{{"title":"t","body":"{}"}}"#, "x".repeat(pad));
        assert_eq!(
            body.len(),
            limit + 1,
            "test body must land exactly one byte past the limit"
        );

        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app.oneshot(json_request(&body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn ttl_field_on_wire_is_ignored_uses_configured_default() {
        // v1 spec §3: `/notify` never accepts a client-supplied ttl at
        // all — `NotifyRequest` has no `ttlSecs` field. An extra,
        // unrecognized field is silently ignored (no
        // `#[serde(deny_unknown_fields)]`), and the server's configured
        // `default_ttl` still applies. Verified via `next_deadline()`:
        // plan 033 arms the auto-retract at promotion, so the earliest
        // deadline is the retract at half the base window — ~now +
        // default_ttl/2, not anywhere near the attempted wire value.
        let state = test_state(SingleSlotQueue::new(50)); // default_ttl: 8
        let before = std::time::Instant::now();
        let app = router(state.clone());
        let response = app
            .oneshot(json_request(
                r#"{"title":"t","body":"b","ttlSecs":99999999}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let deadline = state
            .engine
            .read(|q| q.next_deadline())
            .await
            .expect("a freshly-promoted item has a deadline");
        let elapsed_to_deadline = deadline.duration_since(before).as_secs();
        assert!(
            (3..=5).contains(&elapsed_to_deadline),
            "expected ~default_ttl/2 (4s, the armed auto-retract), got {elapsed_to_deadline}s — the wire ttlSecs value leaked through"
        );
    }

    // --- plan 035: rich-relay subtitle/details wire fields + caps ---

    #[test]
    fn sanitize_subtitle_empties_and_caps() {
        assert_eq!(sanitize_subtitle(None), None);
        assert_eq!(sanitize_subtitle(Some(String::new())), None); // empty -> None
        assert_eq!(
            sanitize_subtitle(Some("short".to_string())),
            Some("short".to_string())
        );
        // 121 chars -> 120 kept + an ellipsis (121 total)
        let capped = sanitize_subtitle(Some("x".repeat(121))).unwrap();
        assert_eq!(capped.chars().count(), 121);
        assert!(capped.ends_with('…'));
        assert_eq!(capped.chars().filter(|c| *c == 'x').count(), 120);
    }

    #[test]
    fn sanitize_details_enforces_caps() {
        // 9 non-empty-label pairs -> capped to 8
        let nine: Vec<DetailItem> = (0..9)
            .map(|i| DetailItem {
                label: format!("L{i}"),
                value: format!("v{i}"),
            })
            .collect();
        assert_eq!(sanitize_details(Some(nine)).len(), 8);

        // empty-label pairs dropped before the count cap applies
        let with_empty = vec![
            DetailItem {
                label: String::new(),
                value: "dropped".to_string(),
            },
            DetailItem {
                label: "Kept".to_string(),
                value: "v".to_string(),
            },
        ];
        let kept = sanitize_details(Some(with_empty));
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].label, "Kept");

        // label > 40 and value > 200 chars each truncated with an ellipsis
        let big = sanitize_details(Some(vec![DetailItem {
            label: "L".repeat(50),
            value: "v".repeat(500),
        }]));
        assert_eq!(big[0].label.chars().count(), 41); // 40 + '…'
        assert!(big[0].label.ends_with('…'));
        assert_eq!(big[0].value.chars().count(), 201); // 200 + '…'
        assert!(big[0].value.ends_with('…'));

        assert!(sanitize_details(None).is_empty()); // absent -> empty
    }

    #[tokio::test]
    async fn notify_round_trips_subtitle_and_details_into_slot_state() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let response = app
            .oneshot(json_request(
                r#"{"title":"t","body":"b","subtitle":"Permission request","details":[{"label":"Tool","value":"Bash"},{"label":"Command","value":"git push"}]}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // bind first so the MutexGuard drops at the semicolon, not at the
        // end of the match (which would outlive `state`).
        let slot = state.engine.read(|q| q.current_slot_state()).await;
        match slot {
            crate::event::SlotState::Showing {
                subtitle, details, ..
            } => {
                assert_eq!(subtitle.as_deref(), Some("Permission request"));
                assert_eq!(details.len(), 2);
                assert_eq!(details[0].label, "Tool");
                assert_eq!(details[0].value, "Bash");
                assert_eq!(details[1].label, "Command");
                assert_eq!(details[1].value, "git push");
            }
            other => panic!("expected Showing, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn notify_caps_details_server_side_to_eight() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let pairs = (0..9)
            .map(|i| format!(r#"{{"label":"L{i}","value":"v{i}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let body = format!(r#"{{"title":"t","body":"b","details":[{pairs}]}}"#);
        let response = app.oneshot(json_request(&body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let slot = state.engine.read(|q| q.current_slot_state()).await;
        match slot {
            crate::event::SlotState::Showing { details, .. } => {
                assert_eq!(details.len(), 8, "9 pairs on the wire must cap to 8");
            }
            other => panic!("expected Showing, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn notify_caps_title_and_body_server_side() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let long_title = "t".repeat(TITLE_MAX_CHARS + 50);
        let long_body = "b".repeat(BODY_MAX_CHARS + 50);
        let body = serde_json::json!({ "title": long_title, "body": long_body }).to_string();
        let response = app.oneshot(json_request(&body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let slot = state.engine.read(|q| q.current_slot_state()).await;
        match slot {
            crate::event::SlotState::Showing { title, body, .. } => {
                assert_eq!(title.chars().count(), TITLE_MAX_CHARS + 1); // cap + ellipsis
                assert!(title.ends_with('…'));
                assert_eq!(body.chars().count(), BODY_MAX_CHARS + 1);
                assert!(body.ends_with('…'));
            }
            other => panic!("expected Showing, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn notify_leaves_short_title_and_body_untouched() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let response = app
            .oneshot(json_request(r#"{"title":"short title","body":"short body"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let slot = state.engine.read(|q| q.current_slot_state()).await;
        match slot {
            crate::event::SlotState::Showing { title, body, .. } => {
                assert_eq!(title, "short title");
                assert_eq!(body, "short body");
            }
            other => panic!("expected Showing, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn notify_without_subtitle_or_details_leaves_them_empty() {
        // back-compat: an old payload (neither field) yields None/empty,
        // byte-identical to pre-plan-035 behavior.
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let slot = state.engine.read(|q| q.current_slot_state()).await;
        match slot {
            crate::event::SlotState::Showing {
                subtitle, details, ..
            } => {
                assert_eq!(subtitle, None);
                assert!(details.is_empty());
            }
            other => panic!("expected Showing, got {other:?}"),
        }
    }
}
