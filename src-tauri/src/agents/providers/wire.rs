//! Plan 138: the provider-neutral intermediate shape every provider
//! parser (`claude_code.rs`, and future 139/140 Codex/Kimi parsers)
//! produces, plus the schema-v1 (spec §3.1) JSON body builder that turns
//! one into the exact `POST /agent/events` wire shape
//! `agents::adapter::parse_wire_event` accepts.
//!
//! This module is pure — no HTTP, no clock read, no randomness.
//! `eventId`/`occurredAtMs` are supplied by the caller (`delivery.rs`'s
//! caller, `src/bin/notchtap_agent.rs`) precisely because generating
//! them (uuid, `SystemTime::now()`) is impure and doesn't belong in a
//! "pure per-provider parser" (spec §4.1) or in this shared builder.

use serde_json::{json, Value};

/// One normalized Agent Event, provider-agnostic. A provider parser
/// (e.g. [`super::claude_code::normalize`]) builds this from a native
/// hook payload; [`build_wire_body`] turns it into the schema-v1 JSON
/// body. Field values here are ALREADY the wire strings (e.g. `kind:
/// "informational"`, not an enum) — this module has no dependency on
/// `agents::model`/`agents::adapter`'s Rust types on purpose, so the
/// same shape can serialize identically regardless of which crate
/// target (this lib, or a future standalone adapter binary) builds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedEvent {
    pub session_id: String,
    /// The provider's own event name (e.g. `"PostToolUse"`), recorded
    /// for the `nativeEvent` wire field and §10's `agent.native_event`
    /// structured log — diagnostics only, no mapping branches outside
    /// the parser that produced it.
    pub native_event: String,
    /// One of schema v1's five `kind` strings (spec §3.1):
    /// `permission_requested` / `input_required` / `completed` /
    /// `failed` / `informational`.
    pub kind: &'static str,
    /// One of schema v1's seven `state` strings — validated by the
    /// endpoint but NOT authoritative (the registry alone derives
    /// session state from `kind` + `terminal`, see
    /// `agents::registry::next_state`'s doc) — sent anyway so the wire
    /// event is self-describing for logs/debugging.
    pub state: &'static str,
    pub terminal: bool,
    pub summary: Option<String>,
    /// `(label, value)` pairs — already sanitized by the parser (safe
    /// tool name, basename-only paths; never raw command lines/secrets,
    /// spec §3.2).
    pub details: Vec<(String, String)>,
    pub project_name: Option<String>,
    pub project_cwd: Option<String>,
    /// `(id, label, state)`.
    pub subagent: Option<(String, Option<String>, Option<String>)>,
    /// The provider's declared capability set (spec §1's matrix row),
    /// sent on every event this provider emits — see
    /// `claude_code::CAPABILITIES`'s doc for why it's the same constant
    /// set every time rather than computed per-event.
    pub capabilities: Vec<&'static str>,
}

/// Builds the schema-v1 (spec §3.1) `POST /agent/events` JSON body from
/// a [`NormalizedEvent`]. `event_id`/`occurred_at_ms` are caller-supplied
/// (impure inputs, kept out of this pure module — see this file's top
/// doc). `sequence` is `None` for every current provider (Claude Code's
/// hook payloads carry no monotonic counter — spec §4.2/this ticket's
/// instructions: "sequence if the payload offers a monotonic value,
/// else omit").
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
