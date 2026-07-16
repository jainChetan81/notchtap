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
use crate::event::{dispatch, emit_promoted, Event, EventPayload, EventType, Priority};
use crate::queue::NotificationQueue;

// generic over the tauri runtime so tests can use tauri::test::mock_app()
// (MockRuntime) while the app runs on the default Wry runtime
pub struct AppState<R: tauri::Runtime = tauri::Wry> {
    pub queue: Arc<Mutex<NotificationQueue>>,
    pub default_ttl: u64,
    pub app_handle: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> Clone for AppState<R> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            default_ttl: self.default_ttl,
            app_handle: self.app_handle.clone(),
        }
    }
}

#[derive(Deserialize)]
struct NotifyRequest {
    title: Option<String>,
    body: Option<String>,
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
        return Err(HttpError::BadRequest("content-type must be application/json"));
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
        priority: Priority::Normal,
        ttl_secs: state.default_ttl,
        payload: EventPayload { title, body },
    };

    dispatch(event.clone()).map_err(HttpError::Event)?;

    let (promoted, paused, waiting_count) = {
        let mut queue = state.queue.lock().await;
        queue.enqueue(event).map_err(HttpError::Queue)?;
        (queue.take_promoted(), queue.is_paused(), queue.waiting().len())
    };

    emit_promoted(&state.app_handle, promoted);

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

    fn test_state(queue: NotificationQueue) -> AppState<tauri::test::MockRuntime> {
        let app = tauri::test::mock_app();
        AppState {
            queue: Arc::new(Mutex::new(queue)),
            default_ttl: 8,
            app_handle: app.handle().clone(),
        }
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
        let app = router(test_state(NotificationQueue::new(3, 50)));
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_json(response).await, json!({"status": "accepted"}));
    }

    #[tokio::test]
    async fn paused_post_returns_202_with_queued_count() {
        let mut queue = NotificationQueue::new(3, 50);
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
        let app = router(test_state(NotificationQueue::new(3, 50)));
        let response = app.oneshot(json_request("{not json")).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn wrong_content_type_returns_400() {
        let app = router(test_state(NotificationQueue::new(3, 50)));
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
        let app = router(test_state(NotificationQueue::new(3, 50)));
        let response = app
            .oneshot(json_request(r#"{"body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn missing_body_returns_400() {
        let app = router(test_state(NotificationQueue::new(3, 50)));
        let response = app
            .oneshot(json_request(r#"{"title":"t"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn full_queue_returns_429() {
        // max_concurrent 0 and max_queued 0: nothing can be promoted and
        // nothing can wait, so the very first enqueue is rejected.
        let app = router(test_state(NotificationQueue::new(0, 0)));
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn full_queue_returns_429_while_paused() {
        // TESTING_STRATEGY.md §4.3: "still 429 when full while paused" —
        // pause buffers, it never lifts the max_queued cap.
        let mut queue = NotificationQueue::new(0, 0);
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
        let app = router(test_state(NotificationQueue::new(3, 50)));
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
}
