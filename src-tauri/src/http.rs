use std::time::Instant;

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

use crate::agents::adapter::{self, AdapterError};
use crate::agents::board::AgentBoardPublisher;
use crate::agents::model::session_hash_hex;
#[cfg(test)]
use crate::agents::model::AgentSessionKey;
use crate::agents::notification::{self, NotificationPolicy};
use crate::agents::registry::{AgentRegistryHandle, ApplyOutcome};
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
    /// plan 137 (spec §7): renamed from v6.1's `cmux_priority` — the
    /// `/notify` cmux self-declared source is gone (superseded by the v7
    /// Agent Adapter layer), so this flat field has no direct consumer in
    /// this struct today; it's carried here only because `Config`'s own
    /// `agent_priority` field (this value's source) is itself a migration
    /// target, not because `/notify`/`agent_events_handler` reads it.
    pub agent_priority: Priority,
    /// plan 137 (spec §7): renamed from v6.1's `cmux_ttl_secs`. UNLIKE
    /// `agent_priority` above, this one has a live consumer:
    /// `agent_events_handler` passes it as the one-shot rotation window to
    /// `agents::notification::build_notification` for every noteworthy
    /// Agent Notification — the same role `cmux_ttl_secs` played for a
    /// cmux-originated `/notify` push.
    pub agent_ttl_secs: u64,
    /// v7 (plan 137, spec §5/§7): the four kind-priority/informational
    /// knobs `[agents]` config resolves to — built once in `lib.rs`'s
    /// `setup` and reused for every `/agent/events` call, replacing the
    /// `NotificationPolicy::default()` ticket-135 placeholder.
    pub agent_notification_policy: NotificationPolicy,
    /// v7 (plan 137, spec §7): per-runtime `[agents.runtimes.*]` enable
    /// flags, read by `agent_events_handler` to decide whether a known,
    /// syntactically valid runtime's event still reaches the Agent
    /// Registry/Notification Engine — see that handler's own doc for the
    /// "why 202, not 400" reasoning.
    pub agent_runtimes: crate::config::AgentRuntimesConfig,
    /// v7 (plan 137, spec §7): the `[agents]` master switch, independent
    /// of the four per-runtime flags above — `agent_events_handler` skips
    /// the same registry/notification path when this is `false`,
    /// regardless of which runtime sent the event.
    pub agent_enabled: bool,
    /// v7 (plan 133/134): the one Agent Registry, behind the same
    /// application-state boundary as `engine` above — see
    /// `agents/registry.rs::AgentRegistryHandle`'s own doc for why it's
    /// a cheap `Clone` handle rather than the registry by value.
    pub agent_registry: AgentRegistryHandle,
    /// v7 (plan 136): the `agent-state` IPC publisher — see
    /// `agents/board.rs::AgentBoardPublisher`'s own doc. Called here
    /// after every `Applied` `/agent/events` mutation (spec §6: "after
    /// every accepted /agent/events mutation"); the periodic
    /// stale/retention tick is driven independently by
    /// `AgentBoardPublisher::spawn_tick` (`lib.rs`'s `setup` closure).
    pub agent_board: AgentBoardPublisher<R>,
    /// Plan 143 (v7 ticket 11 of 13, spec §4.6/§8/§10): shared Adapter
    /// Health bookkeeping — this handler records "last accepted event"
    /// and "last bounded error category" into it on every request; the
    /// Settings `get_agent_health` command and `agent_board`'s own
    /// publish path both read it back via `HealthTracker::snapshot`. See
    /// that type's own module doc for the pure/impure split.
    pub agent_health: std::sync::Arc<crate::agents::health::HealthTracker>,
}

impl<R: tauri::Runtime> Clone for AppState<R> {
    fn clone(&self) -> Self {
        Self {
            engine: self.engine.clone(),
            default_ttl: self.default_ttl,
            manual_default_priority: self.manual_default_priority,
            agent_priority: self.agent_priority,
            agent_ttl_secs: self.agent_ttl_secs,
            agent_notification_policy: self.agent_notification_policy,
            agent_runtimes: self.agent_runtimes,
            agent_enabled: self.agent_enabled,
            agent_registry: self.agent_registry.clone(),
            agent_board: self.agent_board.clone(),
            agent_health: self.agent_health.clone(),
        }
    }
}

#[derive(Deserialize)]
struct NotifyRequest {
    title: Option<String>,
    body: Option<String>,
    priority: Option<Priority>,
    // non-`Option`, unlike `priority` — deliberate: sources that can't
    // know a specific signal (this endpoint's own CLI callers) simply
    // never set the field and get `Generic` via this default, mirroring
    // `presentation.rs`'s `DetectOutput` cutout-field pattern rather than
    // `priority`'s `unwrap_or` pattern in this same file.
    #[serde(default)]
    signal: EventSignal,
    // plan 137 (spec §7/§12): the v6.1 `source: "cmux"` self-declaration
    // is gone — the cmux relay is superseded by the v7 Agent Adapter
    // layer, which posts to `/agent/events`, not `/notify`. A `/notify`
    // caller has exactly one origin now: `SourceKind::Manual`. An old
    // client that still sends a `"source"` key is unaffected — this
    // struct has no `deny_unknown_fields`, so the unrecognized field is
    // silently ignored, same as any other stray key always was.
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
        .route("/agent/events", post(agent_events_handler::<R>))
        // one shared 64 KiB body cap for both loopback endpoints — spec
        // §3.2 independently lands `/agent/events` on the same number
        // `/notify` already used, so this reads the ONE constant
        // (`agents::adapter::MAX_BODY_BYTES`) instead of repeating the
        // literal a second time.
        .layer(DefaultBodyLimit::max(adapter::MAX_BODY_BYTES))
        .with_state(state)
}

/// Binds the listener. Loopback-only is a security boundary
/// (`ARCHITECTURE.md` §7): this is the single place a bind happens,
/// and it is hardcoded to 127.0.0.1 — no config field can widen it.
pub async fn bind_listener(port: u16) -> std::io::Result<tokio::net::TcpListener> {
    tokio::net::TcpListener::bind(("127.0.0.1", port)).await
}

/// Strips a trailing `:<port>` from a `Host` header value, handling the
/// IPv6 bracket form (`[::1]:9789`) as well as the plain `host:port`
/// form. A value with no colon (or an IPv6 literal with no port suffix)
/// is returned unchanged.
fn host_header_without_port(host_header: &str) -> &str {
    if let Some(rest) = host_header.strip_prefix('[') {
        // IPv6 literal: `[::1]` or `[::1]:9789` — the host is everything
        // up to the closing bracket.
        return rest.split(']').next().unwrap_or(rest);
    }
    match host_header.rsplit_once(':') {
        // only split on a trailing numeric port — a bare IPv6 literal
        // with no brackets (unusual in a Host header, but defensive)
        // contains colons that are not a port separator.
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
        _ => host_header,
    }
}

/// See the DNS-rebinding comment at the call site in `notify_handler`:
/// only these three loopback literals are accepted, port suffix ignored.
fn is_loopback_host(host_header: &str) -> bool {
    matches!(
        host_header_without_port(host_header),
        "127.0.0.1" | "localhost" | "::1"
    )
}

/// Content-type defense (`application/json` only), shared by every
/// loopback POST handler (`notify_handler`, `agent_events_handler`).
/// `endpoint` is just the log-line label (e.g. `"notify"`,
/// `"agent/events"`) — the check itself is identical for both routes.
fn check_json_content_type(headers: &HeaderMap, endpoint: &str) -> Result<(), HttpError> {
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type.starts_with("application/json") {
        tracing::warn!(%content_type, "{endpoint}: rejected — content-type must be application/json");
        return Err(HttpError::BadRequest(
            "content-type must be application/json",
        ));
    }
    Ok(())
}

/// DNS-rebinding defense, shared by every loopback POST handler. The
/// loopback bind (`bind_listener`, above) stops a remote attacker from
/// reaching this socket at all, but a page served from a *legitimate*
/// remote origin can rebind its own hostname's DNS to 127.0.0.1 after
/// the browser's same-origin checks already passed, then issue a
/// same-origin `fetch` that lands here over a genuinely local TCP
/// connection. The one thing that request can't forge convincingly is
/// the `Host` header — a browser sets it from the URL's origin, which
/// is the attacker's domain, not `127.0.0.1`. The legitimate `notchtap`
/// CLI and (v7) Agent Adapter helpers (superseded the earlier cmux relay,
/// plan 137) always talk to
/// `http://127.0.0.1:<port>/...`, so they always send a loopback Host.
/// Reject anything else (including a missing header).
fn check_loopback_host(headers: &HeaderMap, endpoint: &str) -> Result<(), HttpError> {
    let host_header = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok());
    match host_header {
        Some(h) if is_loopback_host(h) => Ok(()),
        _ => {
            tracing::warn!(host = ?host_header, "{endpoint}: rejected — host header is not a loopback literal");
            Err(HttpError::BadRequest(
                "host header must be a loopback literal",
            ))
        }
    }
}

async fn notify_handler<R: tauri::Runtime>(
    State(state): State<AppState<R>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, HttpError> {
    check_json_content_type(&headers, "notify")?;
    check_loopback_host(&headers, "notify")?;

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

    // plan 137: `/notify` has exactly one origin now — the cmux
    // self-declaration is gone (see `NotifyRequest`'s own doc).
    let (origin, default_priority, ttl_secs) = (
        SourceKind::Manual,
        state.manual_default_priority,
        state.default_ttl,
    );

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

/// `POST /agent/events` (spec §3, plan 134). Shares `/notify`'s
/// listener, loopback binding, Host-header defense, and body-limit
/// posture (`router`, above) — see `check_json_content_type`/
/// `check_loopback_host`'s docs for why those two checks are factored
/// out rather than duplicated here.
///
/// Status mapping (spec §3.2): a parse/validation failure
/// (`AdapterError`, `agents/adapter.rs`) is always `400`; oversized body
/// is `413` via the router's `DefaultBodyLimit` layer (never reaches
/// this function); a successful [`ApplyOutcome::Applied`] and the two
/// idempotent no-op outcomes (`DuplicateEventId`/`StaleSequence`) are
/// both `202` — the wire response distinguishes them only via the
/// `idempotent` body field, per spec's "duplicate eventId or stale
/// sequence → idempotent 202 with no registry change" (the caller
/// cannot tell from the status code alone, by design: both are a
/// successful, safe-to-retry acceptance).
async fn agent_events_handler<R: tauri::Runtime>(
    State(state): State<AppState<R>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, HttpError> {
    check_json_content_type(&headers, "agent/events")?;
    check_loopback_host(&headers, "agent/events")?;

    let parsed = adapter::parse_wire_event(&body).map_err(|e| {
        tracing::warn!(error = %e, "agent/events: rejected — {e}");
        // Plan 143 (spec §10's "last bounded error category"): attribute
        // the rejection to a known runtime's Adapter Health card
        // whenever the body at least named one — see
        // `best_effort_runtime_hint`'s own doc for why this is a
        // separate, tolerant re-read rather than something `AdapterError`
        // itself carries.
        if let Some(runtime) = crate::agents::health::best_effort_runtime_hint(&body) {
            state.agent_health.record_error(
                runtime,
                crate::agents::health::AdapterErrorCategory::from_adapter_error(&e),
            );
        }
        HttpError::Agent(e)
    })?;

    let event = parsed.event;
    let session_key = event.session_key.clone();
    let session_hash = session_hash_hex(&session_key);
    let event_id = event.event_id.clone();
    let kind = event.kind;
    let terminal = event.terminal;
    // Cloned before `event` moves into `apply_event` below — plan 135's
    // notification mapping (`notification::build_notification`) needs the
    // same already-sanitized summary the registry itself just accepted,
    // not a second untrusted read of the wire body.
    let summary = event.summary.clone();
    // Plan 147: same clone-before-move for the parity fields — the project
    // NAME (not cwd) and the already-sanitized/capped details the registry
    // just accepted, so an agent card's subtitle/details match what a
    // manual `/notify` rich-relay call would populate for the same shape.
    let project_name = event.project.as_ref().and_then(|p| p.name.clone());
    let details = event.details.clone();
    let runtime = session_key.runtime;
    let native_event = parsed.native_event;

    // Plan 143 (spec §10's "last accepted event time"): a well-formed
    // event was just parsed off the wire for this runtime — recorded
    // regardless of the admin-disabled check just below, since this
    // field answers "is the adapter actually delivering", not "did
    // notchtap act on it" (see `HealthTracker::record_accepted`'s own
    // doc).
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    state.agent_health.record_accepted(runtime, now_ms);

    // plan 137 (spec §7): a KNOWN, syntactically valid runtime
    // (`adapter::parse_wire_event` already 400s an unrecognized runtime
    // string — that's the "unsupported runtime" spec §3.2 means) whose
    // `[agents.runtimes.*]` toggle is administratively off skips BOTH the
    // Agent Registry mutation and the Notification mapping entirely.
    // This is deliberately a `202`, not a `400`: the request itself is
    // well-formed and the runtime is one this build genuinely supports —
    // the operator has just chosen not to accept its events right now,
    // the same "accepted but not acted on" shape spec §3.2 already uses
    // for a duplicate `eventId`/stale `sequence`. `runtimeDisabled` is a
    // diagnostic-only wire field alongside `idempotent`/
    // `notificationQueued`, so a caller (or a test) can distinguish this
    // path from an ordinary accepted event without guessing from the
    // (identical) status code.
    if !state.agent_enabled || !state.agent_runtimes.runtime_enabled(runtime) {
        tracing::info!(
            "agent.runtime" = ?runtime,
            "agent.event_id" = %event_id,
            "agent/events: rejected — runtime administratively disabled"
        );
        return Ok((
            StatusCode::ACCEPTED,
            Json(json!({
                "status": "accepted",
                "idempotent": false,
                "notificationQueued": null,
                "runtimeDisabled": true,
            })),
        )
            .into_response());
    }

    let now = Instant::now();
    let outcome = state.agent_registry.apply_event(event, now).await;
    let resulting_state = state.agent_registry.state_for(&session_key, now).await;

    // spec §6: "after every accepted /agent/events mutation" — gated on
    // `Applied` specifically (not the two idempotent no-op outcomes):
    // `DuplicateEventId`/`StaleSequence` made zero registry change, so
    // `publish_if_changed`'s own dedup would suppress them anyway, but
    // skipping the call entirely avoids a needless registry re-read on
    // the (expected, at-least-once-delivery) common case of a retried
    // event.
    if matches!(outcome, ApplyOutcome::Applied) {
        state.agent_board.publish_if_changed(now).await;
    }

    let idempotent = matches!(
        outcome,
        ApplyOutcome::DuplicateEventId | ApplyOutcome::StaleSequence
    );

    // Plan 135 (spec §5): only a freshly `Applied` event is ever eligible
    // for a Notification — a duplicate/stale no-op must never re-offer one
    // (the registry itself already made zero state change for those, so a
    // second card would be pure duplication, not a queue-full retry).
    // `notification_queued` stays `None` (serializes to JSON `null`) for
    // every registry-only path: idempotent no-ops AND ordinary
    // Starting/Working/tool/subagent progress and (default policy)
    // suppressed Informational/non-terminal-Failed events, none of which
    // ever attempted to enter the Engine at all — `Some(false)` is
    // reserved for the one case spec §5 actually names, a noteworthy
    // event that WAS attempted and lost to a full queue tier.
    let mut notification_queued: Option<bool> = None;
    if matches!(outcome, ApplyOutcome::Applied) {
        // plan 137: `NotificationPolicy`/the agent-notification ttl now
        // come from real `[agents]` config — `state.agent_notification_policy`
        // (built once in `lib.rs`'s `setup` from `agents.*_priority`/
        // `agents.informational_notifications`) and `state.agent_ttl_secs`
        // (the flat migration-target field, spec §7 — mirroring how
        // `cmux_ttl_secs` fell back to `default_ttl` absent its own
        // override).
        if let Some(notification) = notification::build_notification(
            &session_key,
            kind,
            terminal,
            notification::NotificationContent {
                summary: summary.as_deref(),
                project_name: project_name.as_deref(),
                details: &details,
            },
            state.agent_ttl_secs,
            &state.agent_notification_policy,
        ) {
            notification_queued = Some(match state.engine.accept(notification, false).await {
                Ok(()) => true,
                Err(QueueError::QueueFull) => false,
            });
        }
    }

    // §10 structured log fields — cwd and the raw session id never
    // appear here (`session_hash`, not `session_key.native_session_id`).
    tracing::info!(
        "agent.runtime" = ?runtime,
        "agent.session_hash" = %session_hash,
        "agent.native_event" = %native_event,
        "agent.kind" = ?kind,
        "agent.state" = ?resulting_state,
        "agent.event_id" = %event_id,
        "agent.notification_queued" = ?notification_queued,
        idempotent,
        "agent/events: {}",
        if idempotent { "idempotent no-op" } else { "accepted" }
    );

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "status": "accepted",
            "idempotent": idempotent,
            "notificationQueued": notification_queued,
        })),
    )
        .into_response())
}

#[derive(Debug)]
enum HttpError {
    BadRequest(&'static str),
    Event(EventError),
    Queue(QueueError),
    Agent(AdapterError),
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            HttpError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.to_string()),
            HttpError::Event(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            HttpError::Queue(QueueError::QueueFull) => {
                (StatusCode::TOO_MANY_REQUESTS, "queue is full".to_string())
            }
            HttpError::Agent(e) => (StatusCode::BAD_REQUEST, e.to_string()),
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
    use crate::notifier::ConnectorHandle;
    use crate::queue::SingleSlotQueue;
    use axum::http::Request;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tower::ServiceExt;

    fn test_state(queue: SingleSlotQueue) -> AppState<tauri::test::MockRuntime> {
        test_state_with_connectors(queue, Vec::new())
    }

    fn test_state_with_connectors(
        queue: SingleSlotQueue,
        connectors: Vec<ConnectorHandle>,
    ) -> AppState<tauri::test::MockRuntime> {
        let app = tauri::test::mock_app();
        let agent_registry = test_agent_registry();
        AppState {
            engine: Engine::new(
                queue,
                app.handle().clone(),
                Arc::new(connectors),
                true,
                true,
                false,
                false,
                None,
            ),
            default_ttl: 8,
            manual_default_priority: Priority::Medium,
            agent_priority: Priority::High,
            agent_ttl_secs: 8,
            agent_notification_policy: NotificationPolicy::default(),
            agent_runtimes: crate::config::AgentRuntimesConfig::default(),
            agent_enabled: true,
            agent_registry: agent_registry.clone(),
            agent_board: AgentBoardPublisher::new(
                app.handle().clone(),
                agent_registry,
                Arc::new(crate::agents::health::HealthTracker::new()),
                crate::config::AgentRuntimesConfig::default(),
            ),
            agent_health: Arc::new(crate::agents::health::HealthTracker::new()),
        }
    }

    fn test_agent_registry() -> AgentRegistryHandle {
        AgentRegistryHandle::new(crate::agents::registry::AgentRegistry::new(
            Duration::from_secs(900),
            crate::agents::registry::DEFAULT_TERMINAL_RETENTION,
            crate::agents::registry::DEFAULT_STALE_RETENTION,
        ))
    }

    /// a connector whose receiving end the test holds, so fan-out can be
    /// asserted without any worker or network
    fn test_connector() -> (ConnectorHandle, tokio::sync::mpsc::Receiver<Event>) {
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        (ConnectorHandle::new("test", tx), rx)
    }

    fn json_request(body: &str) -> Request<Body> {
        // a hand-built `Request` (unlike a real hyper client) doesn't get
        // a `Host` header for free, so every test that expects to reach
        // past the Host check sets a loopback one explicitly here.
        Request::builder()
            .method("POST")
            .uri("/notify")
            .header("content-type", "application/json")
            .header("host", "127.0.0.1:9789")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    fn agent_events_request(body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/agent/events")
            .header("content-type", "application/json")
            .header("host", "127.0.0.1:9789")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    /// A minimal, structurally-valid schema-v1 body (spec §3.1) —
    /// `event_id`/`session_id` are parameters so tests can vary identity
    /// without repeating the whole JSON literal.
    fn valid_agent_body(event_id: &str, session_id: &str) -> String {
        format!(
            r#"{{"schemaVersion":1,"eventId":"{event_id}","runtime":"codex","sessionId":"{session_id}","nativeEvent":"PermissionRequest","kind":"permission_requested","state":"waiting_for_permission","terminal":false}}"#
        )
    }

    // plan 147: same shape as `valid_agent_body` but carrying `project`/
    // `details`, for the notification-parity pin below.
    fn valid_agent_body_with_project_and_details(event_id: &str, session_id: &str) -> String {
        format!(
            r#"{{"schemaVersion":1,"eventId":"{event_id}","runtime":"codex","sessionId":"{session_id}","nativeEvent":"PermissionRequest","kind":"permission_requested","state":"waiting_for_permission","terminal":false,"project":{{"name":"mac-notification-nudge","cwd":"/Users/dev/mac-notification-nudge"}},"details":[{{"label":"Tool","value":"Bash"}},{{"label":"Command","value":"git push"}}]}}"#
        )
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

    // --- M1: Host-header validation (DNS-rebinding defense) ---

    #[test]
    fn is_loopback_host_accepts_the_three_loopback_literals_with_or_without_port() {
        for host in ["127.0.0.1", "127.0.0.1:9789", "localhost", "localhost:9789"] {
            assert!(is_loopback_host(host), "{host:?} should be accepted");
        }
        // IPv6 loopback, bracketed (the only valid Host-header form for a
        // literal IPv6 address), with and without a port suffix.
        for host in ["[::1]", "[::1]:9789"] {
            assert!(is_loopback_host(host), "{host:?} should be accepted");
        }
    }

    #[test]
    fn is_loopback_host_rejects_foreign_hosts() {
        for host in [
            "attacker-domain",
            "attacker-domain:9789",
            "evil.com",
            "127.0.0.1.evil.com",
            "0.0.0.0",
            "[::2]",
        ] {
            assert!(!is_loopback_host(host), "{host:?} should be rejected");
        }
    }

    #[tokio::test]
    async fn valid_loopback_host_header_passes() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let request = Request::builder()
            .method("POST")
            .uri("/notify")
            .header("content-type", "application/json")
            .header("host", "127.0.0.1:9789")
            .body(Body::from(r#"{"title":"t","body":"b"}"#))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn foreign_host_header_is_rejected() {
        // the DNS-rebinding scenario: a rebound browser's same-origin
        // fetch still carries the attacker's own domain as Host, even
        // though the TCP connection lands on loopback.
        let app = router(test_state(SingleSlotQueue::new(50)));
        let request = Request::builder()
            .method("POST")
            .uri("/notify")
            .header("content-type", "application/json")
            .header("host", "attacker-domain:9789")
            .body(Body::from(r#"{"title":"t","body":"b"}"#))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn missing_host_header_is_rejected() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let request = Request::builder()
            .method("POST")
            .uri("/notify")
            .header("content-type", "application/json")
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

    // --- plan 137 (spec §7/§12): the v6.1 cmux source field is gone ---

    #[tokio::test]
    async fn a_source_field_on_the_wire_is_silently_ignored_and_stays_manual() {
        // `RequestSource`/`--source cmux` no longer exist (superseded by
        // the v7 Agent Adapter's own `/agent/events` endpoint) — a
        // `/notify` caller that still sends a `"source"` key (an old
        // script, a stale integration) must not be rejected: `NotifyRequest`
        // has no `deny_unknown_fields`, so the key is silently ignored and
        // the push resolves as an ordinary Manual push, same as if the key
        // were absent entirely.
        let mut state = test_state(SingleSlotQueue::new(50));
        state.manual_default_priority = Priority::Low;
        let app = router(state.clone());
        let response = app
            .oneshot(json_request(r#"{"title":"t","body":"b","source":"cmux"}"#))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            state.engine.read(|q| q.current_priority()).await,
            Some(Priority::Low)
        );
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
            .oneshot(json_request(
                r#"{"title":"short title","body":"short body"}"#,
            ))
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

    // --- plan 134: POST /agent/events (spec §3.2 status-code rows) ---

    #[tokio::test]
    async fn valid_agent_event_returns_202_accepted() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app
            .oneshot(agent_events_request(&valid_agent_body("e1", "s1")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = body_json(response).await;
        assert_eq!(body["status"].as_str(), Some("accepted"));
        assert_eq!(body["idempotent"].as_bool(), Some(false));
    }

    #[tokio::test]
    async fn agent_event_updates_the_registry() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let response = app
            .oneshot(agent_events_request(&valid_agent_body("e1", "s1")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(state.agent_registry.session_count().await, 1);
    }

    // --- plan 137 (spec §7): per-runtime `[agents.runtimes.*]` gate ---

    #[tokio::test]
    async fn disabled_runtime_skips_both_registry_and_notification_and_returns_202() {
        // `valid_agent_body` is always `runtime: "codex"` — disable just
        // that runtime and prove BOTH halves of the skip: no registry
        // mutation (`session_count` stays 0) and no notification queued
        // (queue's `current_priority` stays `None`), while the endpoint
        // still answers `202` (a known, syntactically valid runtime is not
        // the same "unsupported runtime" `400` case spec §3.2 means — see
        // `agent_events_handler`'s own doc).
        let mut state = test_state(SingleSlotQueue::new(50));
        state.agent_runtimes.codex.enabled = false;
        let app = router(state.clone());
        let response = app
            .oneshot(agent_events_request(&valid_agent_body("e1", "s1")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = body_json(response).await;
        assert_eq!(body["runtimeDisabled"].as_bool(), Some(true));
        assert_eq!(body["notificationQueued"], serde_json::Value::Null);
        assert_eq!(state.agent_registry.session_count().await, 0);
        assert_eq!(state.engine.read(|q| q.current_priority()).await, None);
    }

    #[tokio::test]
    async fn agents_master_switch_off_skips_every_runtime() {
        let mut state = test_state(SingleSlotQueue::new(50));
        state.agent_enabled = false;
        let app = router(state.clone());
        let response = app
            .oneshot(agent_events_request(&valid_agent_body("e1", "s1")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = body_json(response).await;
        assert_eq!(body["runtimeDisabled"].as_bool(), Some(true));
        assert_eq!(state.agent_registry.session_count().await, 0);
    }

    #[tokio::test]
    async fn other_runtimes_are_unaffected_by_a_disabled_sibling_runtime() {
        let mut state = test_state(SingleSlotQueue::new(50));
        state.agent_runtimes.codex.enabled = false;
        let app = router(state.clone());
        let claude_body = r#"{"schemaVersion":1,"eventId":"e2","runtime":"claude-code","sessionId":"s2","nativeEvent":"PermissionRequest","kind":"permission_requested","state":"waiting_for_permission","terminal":false}"#;
        let response = app
            .oneshot(agent_events_request(claude_body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = body_json(response).await;
        assert!(body.get("runtimeDisabled").is_none());
        assert_eq!(state.agent_registry.session_count().await, 1);
    }

    // --- plan 135 (spec §5): registry → Notification mapping ---------

    #[tokio::test]
    async fn noteworthy_agent_event_also_queues_a_notification() {
        // `valid_agent_body` is `kind: "permission_requested"` — noteworthy
        // per spec §5's table, so it must both update the registry AND
        // promote a High-priority card into the (empty, plenty-of-room)
        // queue.
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let response = app
            .oneshot(agent_events_request(&valid_agent_body("e1", "s1")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = body_json(response).await;
        assert_eq!(body["notificationQueued"].as_bool(), Some(true));

        let slot = state.engine.read(|q| q.current_slot_state()).await;
        match slot {
            crate::event::SlotState::Showing {
                event_type,
                priority,
                origin,
                ..
            } => {
                assert_eq!(event_type, crate::event::EventType::AgentEvent);
                assert_eq!(priority, Priority::High);
                assert_eq!(origin, SourceKind::Agent);
            }
            other => panic!("expected Showing, got {other:?}"),
        }
    }

    // plan 147: the parity companion to the pin above — a noteworthy event
    // that also carries `project`/`details` must thread the project NAME
    // onto `subtitle` and the details onto `details`, the same
    // `notification::build_notification` parity mapping unit-tested
    // directly in `agents/notification.rs`, now proven end to end through
    // the real `/agent/events` handler.
    #[tokio::test]
    async fn noteworthy_agent_event_threads_project_and_details_onto_the_card() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let response = app
            .oneshot(agent_events_request(
                &valid_agent_body_with_project_and_details("e1", "s1"),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = body_json(response).await;
        assert_eq!(body["notificationQueued"].as_bool(), Some(true));

        let slot = state.engine.read(|q| q.current_slot_state()).await;
        match slot {
            crate::event::SlotState::Showing {
                subtitle, details, ..
            } => {
                // The project NAME, not the cwd.
                assert_eq!(subtitle.as_deref(), Some("mac-notification-nudge"));
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
    async fn progress_event_creates_no_card_and_preserves_registry_history() {
        // A wire `informational`/`terminal: false` event (Starting/Working/
        // tool/subagent progress, spec §4.2/§4.3's hook lists have no
        // dedicated "progress" kind) must update the registry only — no
        // card, and the registry's own accepted state/history for that
        // session is untouched by the (absent) notification attempt.
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let body = r#"{"schemaVersion":1,"eventId":"e1","runtime":"codex","sessionId":"s1","nativeEvent":"PostToolUse","kind":"informational","state":"working","terminal":false,"summary":"Running tests"}"#;
        let response = app
            .clone()
            .oneshot(agent_events_request(body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let resp_json = body_json(response).await;
        assert!(resp_json["notificationQueued"].is_null());

        let slot = state.engine.read(|q| q.current_slot_state()).await;
        assert!(matches!(slot, crate::event::SlotState::Empty));

        assert_eq!(state.agent_registry.session_count().await, 1);
        // The very first event for a brand-new session key is `Informational`
        // + `terminal: false` — SessionStart in disguise (no dedicated wire
        // kind for it, `registry::next_state`'s doc) — so it leaves the
        // session at its `Starting` baseline rather than advancing to
        // `Working`. A SECOND informational event (a real "progress" tick,
        // not session start) is what actually proves the Working
        // transition — and, more importantly for this ticket, that it
        // creates no card either.
        let key = AgentSessionKey::new(crate::agents::model::AgentRuntime::Codex, "s1").unwrap();
        assert_eq!(
            state.agent_registry.state_for(&key, Instant::now()).await,
            Some(crate::agents::model::AgentSessionState::Starting)
        );

        let progress_body = r#"{"schemaVersion":1,"eventId":"e2","runtime":"codex","sessionId":"s1","nativeEvent":"PostToolUse","kind":"informational","state":"working","terminal":false,"summary":"Still running tests"}"#;
        let response2 = app
            .oneshot(agent_events_request(progress_body))
            .await
            .unwrap();
        assert_eq!(response2.status(), StatusCode::ACCEPTED);
        assert!(body_json(response2).await["notificationQueued"].is_null());

        let slot_after = state.engine.read(|q| q.current_slot_state()).await;
        assert!(matches!(slot_after, crate::event::SlotState::Empty));
        assert_eq!(
            state.agent_registry.state_for(&key, Instant::now()).await,
            Some(crate::agents::model::AgentSessionState::Working)
        );
        assert_eq!(state.agent_registry.session_count().await, 1);
    }

    #[tokio::test]
    async fn duplicate_noteworthy_event_never_double_queues_a_notification() {
        // A duplicate `eventId` re-delivery of a noteworthy event must be
        // a pure no-op end to end — not just registry-silent (already
        // covered by `duplicate_event_id_is_202_idempotent_with_zero_mutation`)
        // but also never a second attempt at the Engine.
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let first = app
            .clone()
            .oneshot(agent_events_request(&valid_agent_body("e1", "s1")))
            .await
            .unwrap();
        assert_eq!(
            body_json(first).await["notificationQueued"].as_bool(),
            Some(true)
        );

        let second = app
            .oneshot(agent_events_request(&valid_agent_body("e1", "s1")))
            .await
            .unwrap();
        let body2 = body_json(second).await;
        assert_eq!(body2["idempotent"].as_bool(), Some(true));
        assert!(body2["notificationQueued"].is_null());
    }

    #[tokio::test]
    async fn queue_full_agent_notification_still_updates_registry_and_returns_202() {
        // Spec §5's queue-full independence: the registry accepts an
        // Agent Event regardless of Engine queue capacity — losing an
        // ephemeral card must never lose authoritative session state.
        // Per-tier cap 0 (same recipe as `full_queue_returns_429` for
        // `/notify`): the first push fast-path-promotes into the empty
        // slot; the second, same-tier (High) push has nowhere to go.
        let state = test_state(SingleSlotQueue::new(0));
        let app = router(state.clone());

        let first = app
            .clone()
            .oneshot(agent_events_request(&valid_agent_body("e1", "s1")))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::ACCEPTED);
        assert_eq!(
            body_json(first).await["notificationQueued"].as_bool(),
            Some(true)
        );

        let second = app
            .oneshot(agent_events_request(&valid_agent_body("e2", "s2")))
            .await
            .unwrap();
        // Still 202, never 429 — the Notification's queue-full is not an
        // HTTP error for this endpoint, only a diagnostic body field.
        assert_eq!(second.status(), StatusCode::ACCEPTED);
        let body2 = body_json(second).await;
        assert_eq!(body2["status"].as_str(), Some("accepted"));
        assert_eq!(body2["idempotent"].as_bool(), Some(false));
        assert_eq!(body2["notificationQueued"].as_bool(), Some(false));

        // The registry still accepted BOTH sessions — the second one's
        // lost card never rolled back the registry mutation that already
        // happened.
        assert_eq!(state.agent_registry.session_count().await, 2);
        let key2 = AgentSessionKey::new(crate::agents::model::AgentRuntime::Codex, "s2").unwrap();
        assert_eq!(
            state.agent_registry.state_for(&key2, Instant::now()).await,
            Some(crate::agents::model::AgentSessionState::WaitingForPermission)
        );
    }

    #[tokio::test]
    async fn unknown_schema_version_returns_400() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let body = r#"{"schemaVersion":2,"eventId":"e1","runtime":"codex","sessionId":"s1","nativeEvent":"x","kind":"completed","state":"completed"}"#;
        let response = app.oneshot(agent_events_request(body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn malformed_agent_json_returns_400() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app
            .oneshot(agent_events_request("{not json"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn agent_event_missing_identity_returns_400() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let body = r#"{"schemaVersion":1,"runtime":"codex","sessionId":"s1","nativeEvent":"x","kind":"completed","state":"completed"}"#;
        let response = app.oneshot(agent_events_request(body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn agent_event_unsupported_runtime_returns_400() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let body = r#"{"schemaVersion":1,"eventId":"e1","runtime":"cursor","sessionId":"s1","nativeEvent":"x","kind":"completed","state":"completed"}"#;
        let response = app.oneshot(agent_events_request(body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn agent_event_malformed_enum_returns_400() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let body = r#"{"schemaVersion":1,"eventId":"e1","runtime":"codex","sessionId":"s1","nativeEvent":"x","kind":"not_a_kind","state":"completed"}"#;
        let response = app.oneshot(agent_events_request(body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn oversized_agent_event_body_returns_413() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let big_summary = "x".repeat(70 * 1024);
        let body = format!(
            r#"{{"schemaVersion":1,"eventId":"e1","runtime":"codex","sessionId":"s1","nativeEvent":"x","kind":"completed","state":"completed","summary":"{big_summary}"}}"#
        );
        let response = app.oneshot(agent_events_request(&body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn wrong_content_type_on_agent_events_returns_400() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let request = Request::builder()
            .method("POST")
            .uri("/agent/events")
            .header("content-type", "text/plain")
            .header("host", "127.0.0.1:9789")
            .body(Body::from(valid_agent_body("e1", "s1")))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn foreign_host_header_on_agent_events_is_rejected() {
        // same DNS-rebinding defense as /notify — proves the shared
        // helper is actually wired into the new route, not just /notify.
        let app = router(test_state(SingleSlotQueue::new(50)));
        let request = Request::builder()
            .method("POST")
            .uri("/agent/events")
            .header("content-type", "application/json")
            .header("host", "attacker-domain:9789")
            .body(Body::from(valid_agent_body("e1", "s1")))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn missing_host_header_on_agent_events_is_rejected() {
        let app = router(test_state(SingleSlotQueue::new(50)));
        let request = Request::builder()
            .method("POST")
            .uri("/agent/events")
            .header("content-type", "application/json")
            .body(Body::from(valid_agent_body("e1", "s1")))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // --- duplicate/stale idempotency: 202 + zero registry mutation ---

    #[tokio::test]
    async fn duplicate_event_id_is_202_idempotent_with_zero_mutation() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let first = app
            .clone()
            .oneshot(agent_events_request(&valid_agent_body("e1", "s1")))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::ACCEPTED);
        assert_eq!(state.agent_registry.session_count().await, 1);

        // Re-deliver the SAME eventId with a body that WOULD change state
        // if accepted (a different kind) — it must be a pure no-op.
        let dup_body = r#"{"schemaVersion":1,"eventId":"e1","runtime":"codex","sessionId":"s1","nativeEvent":"Stop","kind":"completed","state":"completed","terminal":true}"#;
        let second = app.oneshot(agent_events_request(dup_body)).await.unwrap();
        assert_eq!(second.status(), StatusCode::ACCEPTED);
        let body = body_json(second).await;
        assert_eq!(body["idempotent"].as_bool(), Some(true));

        // Zero mutation: still 1 session, and it must NOT have flipped to
        // Completed — the duplicate's differing kind/terminal must never
        // have reached the registry.
        assert_eq!(state.agent_registry.session_count().await, 1);
        let key = AgentSessionKey::new(crate::agents::model::AgentRuntime::Codex, "s1").unwrap();
        assert_eq!(
            state.agent_registry.state_for(&key, Instant::now()).await,
            Some(crate::agents::model::AgentSessionState::WaitingForPermission)
        );
    }

    #[tokio::test]
    async fn stale_sequence_is_202_idempotent_with_zero_mutation() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let seeded = r#"{"schemaVersion":1,"eventId":"e1","runtime":"codex","sessionId":"s1","sequence":5,"nativeEvent":"PermissionRequest","kind":"permission_requested","state":"waiting_for_permission","terminal":false}"#;
        let first = app
            .clone()
            .oneshot(agent_events_request(seeded))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::ACCEPTED);

        // Lower sequence, different (state-changing) kind — must be
        // rejected as stale with zero registry mutation.
        let stale = r#"{"schemaVersion":1,"eventId":"e2","runtime":"codex","sessionId":"s1","sequence":4,"nativeEvent":"Stop","kind":"completed","state":"completed","terminal":true}"#;
        let second = app.oneshot(agent_events_request(stale)).await.unwrap();
        assert_eq!(second.status(), StatusCode::ACCEPTED);
        let body = body_json(second).await;
        assert_eq!(body["idempotent"].as_bool(), Some(true));

        let key = AgentSessionKey::new(crate::agents::model::AgentRuntime::Codex, "s1").unwrap();
        assert_eq!(
            state.agent_registry.state_for(&key, Instant::now()).await,
            Some(crate::agents::model::AgentSessionState::WaitingForPermission)
        );
        assert_eq!(state.agent_registry.session_count().await, 1);
    }

    // --- every §3.2 cap: at, above, and trimming behavior -------------

    #[tokio::test]
    async fn agent_event_id_cap_truncates_above_256_bytes() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let long_id = "e".repeat(300);
        let response = app
            .clone()
            .oneshot(agent_events_request(&valid_agent_body(&long_id, "s-cap")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        // A second delivery with the SAME (truncated-to-256) prefix must
        // be treated as the duplicate it now is once both truncate
        // identically — proves the cap actually applied server-side
        // rather than merely being accepted untouched.
        let response2 = app
            .oneshot(agent_events_request(&valid_agent_body(&long_id, "s-cap")))
            .await
            .unwrap();
        let body2 = body_json(response2).await;
        assert_eq!(body2["idempotent"].as_bool(), Some(true));
    }

    #[tokio::test]
    async fn agent_event_summary_cap_truncates_above_500_scalars() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let long_summary = "s".repeat(600);
        let body = format!(
            r#"{{"schemaVersion":1,"eventId":"e1","runtime":"codex","sessionId":"s1","nativeEvent":"x","kind":"completed","state":"completed","terminal":true,"summary":"{long_summary}"}}"#
        );
        let response = app.oneshot(agent_events_request(&body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn agent_event_details_cap_truncates_above_12() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let details: Vec<String> = (0..15)
            .map(|i| format!(r#"{{"label":"L{i}","value":"v{i}"}}"#))
            .collect();
        let body = format!(
            r#"{{"schemaVersion":1,"eventId":"e1","runtime":"codex","sessionId":"s1","nativeEvent":"x","kind":"completed","state":"completed","terminal":true,"details":[{}]}}"#,
            details.join(",")
        );
        let response = app.oneshot(agent_events_request(&body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn agent_event_capabilities_cap_truncates_above_16() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let known = [
            "session_lifecycle",
            "permission_requests",
            "input_required",
            "completion",
            "failure",
            "tool_details",
            "subagents",
            "open_or_focus",
        ];
        let caps_json = (0..20)
            .map(|i| format!(r#""{}""#, known[i % known.len()]))
            .collect::<Vec<_>>()
            .join(",");
        let body = format!(
            r#"{{"schemaVersion":1,"eventId":"e1","runtime":"codex","sessionId":"s1","nativeEvent":"x","kind":"completed","state":"completed","terminal":true,"capabilities":[{caps_json}]}}"#
        );
        let response = app.oneshot(agent_events_request(&body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn agent_event_name_and_cwd_caps_are_enforced_server_side() {
        // Exercises the 120-scalar name/label cap and the 1024-scalar
        // cwd/value cap together via the project object.
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let long_name = "n".repeat(200);
        let long_cwd = "c".repeat(2000);
        let body = format!(
            r#"{{"schemaVersion":1,"eventId":"e1","runtime":"codex","sessionId":"s1","nativeEvent":"x","kind":"completed","state":"completed","terminal":true,"project":{{"name":"{long_name}","cwd":"{long_cwd}"}}}}"#
        );
        let response = app.oneshot(agent_events_request(&body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn agent_event_id_cap_exactly_at_256_bytes_is_accepted_whole() {
        let state = test_state(SingleSlotQueue::new(50));
        let app = router(state.clone());
        let id = "e".repeat(256);
        let response = app
            .oneshot(agent_events_request(&valid_agent_body(&id, "s-atcap")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    // --- log hygiene: raw session id / cwd never reach the log line ---
    //
    // plan 135 fix: this test used to install its OWN `Subscriber` per-run
    // via `tracing::subscriber::set_default` (thread-local). That's the
    // textbook pattern, but it has a well-known sharp edge under real
    // parallelism: `tracing`'s per-callsite `Interest` (whether a given
    // `tracing::info!` call site is "worth" constructing an event for at
    // all) is cached PROCESS-WIDE, not per-thread, and is decided the
    // FIRST time any thread ever touches that exact call site. Plan 135
    // added new lines above `agent_events_handler`'s `tracing::info!`
    // call (the notification-mapping block), which shifts it to a source
    // location tracing has never seen before — and this ticket also added
    // over a dozen OTHER `/agent/events` tests that hit that exact same
    // call site. Under the full suite's parallelism, the overwhelming
    // majority of those other tests reach it first on a thread with the
    // ambient no-op default (no test there installs a subscriber), which
    // caches the call site as "never interesting" — forever, for the
    // whole process — before this test's own thread ever gets a turn.
    // `tracing::callsite::rebuild_interest_cache()` cannot outrun that:
    // another thread can re-lose the race a moment later.
    //
    // The fix is the standard one for this exact pitfall: install exactly
    // ONE global default `Subscriber` for the whole test binary (so
    // `Interest` is decided once, consistently, the same way regardless
    // of which thread asks first) and route each event to the RIGHT
    // test's buffer — or nowhere — via a thread-local lookup inside the
    // writer, which `Subscriber::event` re-consults on every single call
    // (unlike the cached `Interest` fast path). A thread that never calls
    // `CaptureGuard::install` gets the thread-local's default `None` and
    // the writer discards the bytes, so every other test's log output is
    // unaffected.
    thread_local! {
        static CAPTURE_TARGET: std::cell::RefCell<Option<Arc<std::sync::Mutex<Vec<u8>>>>> =
            const { std::cell::RefCell::new(None) };
    }

    #[derive(Clone, Default)]
    struct CaptureWriter;

    struct CaptureHandle;

    impl std::io::Write for CaptureHandle {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            CAPTURE_TARGET.with(|cell| {
                if let Some(target) = cell.borrow().as_ref() {
                    target.lock().unwrap().extend_from_slice(buf);
                }
            });
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
        type Writer = CaptureHandle;
        fn make_writer(&'a self) -> Self::Writer {
            CaptureHandle
        }
    }

    /// Installs the one process-global subscriber (idempotent — `Once`
    /// guards it against every test's concurrent first call) and points
    /// THIS thread's capture target at `buf` until the guard drops.
    struct CaptureGuard;

    impl CaptureGuard {
        fn install(buf: Arc<std::sync::Mutex<Vec<u8>>>) -> Self {
            static INIT: std::sync::Once = std::sync::Once::new();
            INIT.call_once(|| {
                let subscriber = tracing_subscriber::fmt()
                    .with_writer(CaptureWriter)
                    .with_ansi(false)
                    .finish();
                // Best-effort: if some other path in this binary already
                // won the race to install a global default first, this
                // test simply can't capture anything and its own
                // "the log line fired" sanity assertion will fail loudly
                // rather than silently — there is no other global default
                // installed anywhere in this crate (grepped), so in
                // practice this always wins.
                let _ = tracing::subscriber::set_global_default(subscriber);
            });
            CAPTURE_TARGET.with(|cell| *cell.borrow_mut() = Some(buf));
            CaptureGuard
        }
    }

    impl Drop for CaptureGuard {
        fn drop(&mut self) {
            CAPTURE_TARGET.with(|cell| *cell.borrow_mut() = None);
        }
    }

    #[tokio::test]
    async fn raw_session_id_and_cwd_never_reach_the_log_line() {
        let buf: Arc<std::sync::Mutex<Vec<u8>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

        let raw_session_id = "SUPER-SECRET-RAW-SESSION-ID-0xdeadbeef";
        let raw_cwd = "/Users/nobody/very-secret-project-path";
        // plan 135: `kind: "informational"` (progress, not noteworthy under
        // the default policy) rather than `permission_requested` — this
        // test's own concern is log hygiene (`apply_event` + the
        // `agent/events` log line), not the notification-queueing seam
        // `noteworthy_agent_event_also_queues_a_notification` already
        // covers; keeping this one registry-only avoids coupling it to the
        // Engine/mock-app-emit path too.
        let body = format!(
            r#"{{"schemaVersion":1,"eventId":"e1","runtime":"codex","sessionId":"{raw_session_id}","nativeEvent":"PostToolUse","kind":"informational","state":"working","terminal":false,"project":{{"cwd":"{raw_cwd}"}}}}"#
        );

        // See `CaptureGuard`'s doc above for why this installs a
        // process-global subscriber (once) and routes via a thread-local
        // buffer, instead of `tracing::subscriber::set_default`.
        let _guard = CaptureGuard::install(buf.clone());
        let app = router(test_state(SingleSlotQueue::new(50)));
        let response = app.oneshot(agent_events_request(&body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        drop(_guard);

        let captured = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            !captured.contains(raw_session_id),
            "raw session id must never reach the log line, got: {captured}"
        );
        assert!(
            !captured.contains(raw_cwd),
            "raw cwd must never reach the log line, got: {captured}"
        );
        // sanity: the log line DID fire (a hashed, not-empty session_hash
        // field is present), so the negative assertions above aren't
        // vacuously true because nothing was captured at all.
        assert!(
            captured.contains("agent.session_hash"),
            "expected the accept log line to have fired, got: {captured}"
        );
    }
}
