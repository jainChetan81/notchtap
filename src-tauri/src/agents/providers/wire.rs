//! Provider-neutral intermediate shape every provider parser produces,
//! plus the schema-v1 JSON body builder that turns one into the exact
//! `POST /agent/events` wire shape `agents::adapter::parse_wire_event`
//! accepts.
//!
//! Pure — no HTTP, no clock read, no randomness. `eventId`/
//! `occurredAtMs` are supplied by the caller
//! (`src/bin/notchtap_agent.rs`) because generating them is impure.
//!
//! Also holds what every hook parser shares: the `RawHookPayload`
//! stdin shape, the `Mapped` intermediate its match arms build, and
//! the sanitization helpers `basename`, `safe_tool_name`,
//! `safe_path_detail`, `classify_notification`. Those helpers encode a
//! forwarding decision the server-side caps in `agents::adapter`
//! cannot undo, so they live in exactly one place and no parser may
//! reimplement them.

use serde_json::{json, Value};

/// One normalized Agent Event, provider-agnostic. Field values are
/// ALREADY the wire strings (e.g. `kind: "informational"`, not an enum)
/// — deliberately no dependency on `agents::model`/`agents::adapter`
/// types, so any crate target serializes the same shape identically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedEvent {
    pub session_id: String,
    /// The provider's own event name (e.g. `"PostToolUse"`) —
    /// diagnostics only; never branch on it outside the parser that
    /// produced it.
    pub native_event: String,
    /// One of schema v1's five `kind` strings: `permission_requested` /
    /// `input_required` / `completed` / `failed` / `informational`.
    pub kind: &'static str,
    /// One of schema v1's seven `state` strings — validated by the
    /// endpoint but NOT authoritative: the registry alone derives
    /// session state from `kind` + `terminal`
    /// (`agents::registry::next_state`).
    pub state: &'static str,
    pub terminal: bool,
    pub summary: Option<String>,
    /// `(label, value)` pairs — already sanitized by the parser (safe
    /// tool name, basename-only paths; never raw command lines or
    /// secrets).
    pub details: Vec<(String, String)>,
    pub project_name: Option<String>,
    pub project_cwd: Option<String>,
    /// `(id, label, state)`.
    pub subagent: Option<(String, Option<String>, Option<String>)>,
    /// The provider's declared capability set — the same constant set on
    /// every event, never computed per-event.
    pub capabilities: Vec<&'static str>,
}

/// Builds the schema-v1 `POST /agent/events` JSON body from a
/// [`NormalizedEvent`]. `event_id`/`occurred_at_ms` are caller-supplied
/// (impure inputs stay out of this pure module). `sequence` is `None`
/// unless the provider payload offers a monotonic value.
pub fn build_wire_body(
    runtime: &str,
    event: &NormalizedEvent,
    event_id: &str,
    occurred_at_ms: i64,
    sequence: Option<u64>,
) -> Value {
    let mut body = json!({
        "schemaVersion": 1,
        "eventId": event_id,
        "runtime": runtime,
        "sessionId": event.session_id,
        "occurredAtMs": occurred_at_ms,
        "nativeEvent": event.native_event,
        "kind": event.kind,
        "state": event.state,
        "capabilities": event.capabilities,
        "terminal": event.terminal,
    });

    if let Some(seq) = sequence {
        body["sequence"] = json!(seq);
    }
    if let Some(summary) = &event.summary {
        body["summary"] = json!(summary);
    }
    if !event.details.is_empty() {
        let details: Vec<Value> = event
            .details
            .iter()
            .map(|(label, value)| json!({"label": label, "value": value}))
            .collect();
        body["details"] = json!(details);
    }
    if event.project_name.is_some() || event.project_cwd.is_some() {
        body["project"] = json!({
            "name": event.project_name,
            "cwd": event.project_cwd,
        });
    }
    if let Some((id, label, state)) = &event.subagent {
        body["subagent"] = json!({
            "id": id,
            "label": label,
            "state": state,
        });
    }

    body
}

/// The raw hook-payload shape a runtime writes to a hook command's
/// stdin. Every field is `Option` so a payload missing a field the
/// reading parser doesn't use for a given event still deserializes
/// cleanly; unknown keys are ignored.
///
/// SessionEnd names its reason differently per runtime — Claude Code
/// and Kimi send `end_reason`, Codex sends `reason` — so both are kept
/// as distinct fields and each parser reads only its own runtime's.
/// Both stay open strings rather than enums, so a future documented
/// value passes through unchanged.
#[derive(Debug, serde::Deserialize)]
pub(super) struct RawHookPayload {
    pub(super) session_id: Option<String>,
    pub(super) hook_event_name: Option<String>,
    pub(super) cwd: Option<String>,
    // SessionStart
    pub(super) source: Option<String>,
    // SessionEnd (Claude Code, Kimi)
    pub(super) end_reason: Option<String>,
    // SessionEnd (Codex)
    pub(super) reason: Option<String>,
    // PermissionRequest / PreToolUse / PostToolUse / PostToolUseFailure
    pub(super) tool_name: Option<String>,
    pub(super) tool_input: Option<serde_json::Value>,
    // Notification
    pub(super) notification_type: Option<String>,
    // StopFailure
    pub(super) error_type: Option<String>,
    // SubagentStart / SubagentStop
    pub(super) agent_id: Option<String>,
    pub(super) agent_type: Option<String>,
}

/// One `(label, value)`-shaped intermediate a parser's match arms build
/// before wrapping into a [`NormalizedEvent`].
pub(super) struct Mapped {
    pub(super) kind: &'static str,
    pub(super) state: &'static str,
    pub(super) terminal: bool,
    pub(super) summary: Option<String>,
    pub(super) details: Vec<(String, String)>,
    pub(super) subagent: Option<(String, Option<String>, Option<String>)>,
}

pub(super) fn basename(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
}

/// Never empty — a missing/blank `tool_name` becomes the generic
/// `"a tool"` rather than an empty detail value.
pub(super) fn safe_tool_name(tool_name: Option<&str>) -> String {
    tool_name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("a tool")
        .to_string()
}

/// Pulls a basename-only `Path` detail out of a payload's `tool_input`.
/// Reads ONLY the path-shaped keys `file_path`, `path`,
/// `notebook_path`, in that order, and keeps just the basename. Every
/// other key is ignored — `command` (where a full shell command line
/// lives) and `description` (free text) are deliberately excluded, so
/// raw tool input never reaches the wire. Returns `None` when
/// `tool_input` is absent, is not an object, or carries none of those
/// keys as a string.
pub(super) fn safe_path_detail(tool_input: Option<&serde_json::Value>) -> Option<(String, String)> {
    let obj = tool_input?.as_object()?;
    for key in ["file_path", "path", "notebook_path"] {
        if let Some(raw) = obj.get(key).and_then(|v| v.as_str()) {
            let value = basename(raw).unwrap_or_else(|| raw.to_string());
            return Some(("Path".to_string(), value));
        }
    }
    None
}

/// Maps a `Notification` payload by its closed `notification_type` enum
/// only, never by `message` — an unrecognized value becomes
/// `Informational`; wording is never parsed to infer state.
pub(super) fn classify_notification(notification_type: Option<&str>) -> Mapped {
    match notification_type {
        Some("permission_prompt") => Mapped {
            kind: "permission_requested",
            state: "waiting_for_permission",
            terminal: false,
            summary: Some("Approval needed".to_string()),
            details: Vec::new(),
            subagent: None,
        },
        Some("idle_prompt") | Some("agent_needs_input") => Mapped {
            kind: "input_required",
            state: "waiting_for_input",
            terminal: false,
            summary: Some("Waiting for input".to_string()),
            details: Vec::new(),
            subagent: None,
        },
        _ => Mapped {
            kind: "informational",
            state: "working",
            terminal: false,
            summary: Some("Notification".to_string()),
            details: Vec::new(),
            subagent: None,
        },
    }
}
