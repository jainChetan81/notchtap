use thiserror::Error;

use super::model::{
    AgentCapability, AgentDetail, AgentEventKind, AgentHost, AgentProject, AgentRuntime,
    AgentSessionKey, AgentSessionState, AgentSubagentSummary, ModelError,
};
use super::registry::AgentEvent;

#[allow(unused_imports)]
pub use super::registry::{MAX_REMEMBERED_EVENT_IDS, MAX_TRANSITIONS_PER_SESSION};

/// Body cap.
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// event/session/native-event/Host IDs, bytes not scalars — these are opaque provider identifiers,
/// not display text.
pub const MAX_ID_BYTES: usize = 256;
/// `summary`, in Unicode scalar values.
pub const MAX_SUMMARY_SCALARS: usize = 500;
/// project name / Host name / detail+subagent labels, in Unicode scalar values.
pub const MAX_NAME_OR_LABEL_SCALARS: usize = 120;
/// cwd / detail values, in Unicode scalar values.
pub const MAX_VALUE_SCALARS: usize = 1024;
/// `details` array length.
pub const MAX_DETAILS: usize = 12;
/// `capabilities` array length.
pub const MAX_CAPABILITIES: usize = 16;
/// subagents represented per event — see this file's top doc comment for why schema v1 can never
/// actually reach this cap.
pub const MAX_SUBAGENTS_PER_EVENT: usize = 16;

const SUPPORTED_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdapterError {
    #[error("malformed json: {0}")]
    MalformedJson(String),
    #[error("unsupported schemaVersion {0}")]
    UnsupportedSchemaVersion(u64),
    #[error("missing identity: {0}")]
    MissingIdentity(&'static str),
    #[error("unsupported runtime: {0}")]
    UnsupportedRuntime(String),
    #[error("malformed {field}: {value:?}")]
    MalformedEnum { field: &'static str, value: String },
}

/// Every field is `Option` here — even ones the schema treats as conceptually required — so this
/// struct can never fail to deserialize on its own.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireEvent {
    schema_version: Option<u64>,
    event_id: Option<String>,
    runtime: Option<String>,
    session_id: Option<String>,
    #[allow(dead_code)] // timestamps never override receive order — accepted, not used
    occurred_at_ms: Option<i64>,
    sequence: Option<u64>,
    native_event: Option<String>,
    kind: Option<String>,
    state: Option<String>,
    summary: Option<String>,
    details: Option<Vec<WireDetail>>,
    capabilities: Option<Vec<String>>,
    project: Option<WireProject>,
    host: Option<WireHost>,
    subagent: Option<WireSubagent>,
    terminal: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
struct WireDetail {
    label: Option<String>,
    value: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct WireProject {
    name: Option<String>,
    cwd: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireHost {
    name: Option<String>,
    bundle_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct WireSubagent {
    id: Option<String>,
    label: Option<String>,
    state: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedAgentEvent {
    pub event: AgentEvent,
    pub native_event: String,
}

fn sanitize_trim(s: &str) -> String {
    s.trim().chars().filter(|c| !c.is_control()).collect()
}

/// Truncates to at most `max` Unicode scalar values (`char`s) — never splits a codepoint.
fn cap_scalars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

fn cap_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

fn sanitize_id(s: &str) -> String {
    cap_bytes(&sanitize_trim(s), MAX_ID_BYTES)
}

fn sanitize_name_or_label(s: &str) -> Option<String> {
    let s = cap_scalars(&sanitize_trim(s), MAX_NAME_OR_LABEL_SCALARS);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn sanitize_value(s: &str) -> Option<String> {
    let s = cap_scalars(&sanitize_trim(s), MAX_VALUE_SCALARS);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn sanitize_summary(s: &str) -> Option<String> {
    let s = cap_scalars(&sanitize_trim(s), MAX_SUMMARY_SCALARS);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn sanitize_detail_label(s: &str) -> String {
    cap_scalars(&sanitize_trim(s), MAX_NAME_OR_LABEL_SCALARS)
}

fn sanitize_detail_value(s: &str) -> String {
    cap_scalars(&sanitize_trim(s), MAX_VALUE_SCALARS)
}

fn parse_runtime(s: &str) -> Result<AgentRuntime, AdapterError> {
    match s {
        "claude-code" => Ok(AgentRuntime::ClaudeCode),
        "codex" => Ok(AgentRuntime::Codex),
        "kimi" => Ok(AgentRuntime::Kimi),
        "opencode" => Ok(AgentRuntime::OpenCode),
        other => Err(AdapterError::UnsupportedRuntime(other.to_string())),
    }
}

fn parse_kind(s: &str) -> Result<AgentEventKind, AdapterError> {
    match s {
        "permission_requested" => Ok(AgentEventKind::PermissionRequested),
        "input_required" => Ok(AgentEventKind::InputRequired),
        "completed" => Ok(AgentEventKind::Completed),
        "failed" => Ok(AgentEventKind::Failed),
        "informational" => Ok(AgentEventKind::Informational),
        other => Err(AdapterError::MalformedEnum {
            field: "kind",
            value: other.to_string(),
        }),
    }
}

fn parse_state(s: &str) -> Result<AgentSessionState, AdapterError> {
    match s {
        "starting" => Ok(AgentSessionState::Starting),
        "working" => Ok(AgentSessionState::Working),
        "waiting_for_permission" => Ok(AgentSessionState::WaitingForPermission),
        "waiting_for_input" => Ok(AgentSessionState::WaitingForInput),
        "completed" => Ok(AgentSessionState::Completed),
        "failed" => Ok(AgentSessionState::Failed),
        "stale" => Ok(AgentSessionState::Stale),
        other => Err(AdapterError::MalformedEnum {
            field: "state",
            value: other.to_string(),
        }),
    }
}

pub fn runtime_wire_label(runtime: AgentRuntime) -> &'static str {
    match runtime {
        AgentRuntime::ClaudeCode => "claude-code",
        AgentRuntime::Codex => "codex",
        AgentRuntime::Kimi => "kimi",
        AgentRuntime::OpenCode => "opencode",
    }
}

/// Inverse of [`parse_kind`] — see [`runtime_wire_label`]'s doc for why this lives here rather than
/// being re-derived in `notification.rs`.
pub fn kind_wire_label(kind: AgentEventKind) -> &'static str {
    match kind {
        AgentEventKind::PermissionRequested => "permission_requested",
        AgentEventKind::InputRequired => "input_required",
        AgentEventKind::Completed => "completed",
        AgentEventKind::Failed => "failed",
        AgentEventKind::Informational => "informational",
    }
}

/// Inverse of [`parse_state`] — the exact wire token an adapter would send for `state`, same "wire
/// token, not a display label" rule as [`runtime_wire_label`]/[`kind_wire_label`].
pub fn state_wire_label(state: AgentSessionState) -> &'static str {
    match state {
        AgentSessionState::Starting => "starting",
        AgentSessionState::Working => "working",
        AgentSessionState::WaitingForPermission => "waiting_for_permission",
        AgentSessionState::WaitingForInput => "waiting_for_input",
        AgentSessionState::Completed => "completed",
        AgentSessionState::Failed => "failed",
        AgentSessionState::Stale => "stale",
    }
}

pub fn capability_wire_label(capability: AgentCapability) -> &'static str {
    match capability {
        AgentCapability::SessionLifecycle => "session_lifecycle",
        AgentCapability::PermissionRequests => "permission_requests",
        AgentCapability::InputRequired => "input_required",
        AgentCapability::Completion => "completion",
        AgentCapability::Failure => "failure",
        AgentCapability::ToolDetails => "tool_details",
        AgentCapability::Subagents => "subagents",
        AgentCapability::OpenOrFocus => "open_or_focus",
    }
}

fn parse_capability(s: &str) -> Result<AgentCapability, AdapterError> {
    match s {
        "session_lifecycle" => Ok(AgentCapability::SessionLifecycle),
        "permission_requests" => Ok(AgentCapability::PermissionRequests),
        "input_required" => Ok(AgentCapability::InputRequired),
        "completion" => Ok(AgentCapability::Completion),
        "failure" => Ok(AgentCapability::Failure),
        "tool_details" => Ok(AgentCapability::ToolDetails),
        "subagents" => Ok(AgentCapability::Subagents),
        "open_or_focus" => Ok(AgentCapability::OpenOrFocus),
        other => Err(AdapterError::MalformedEnum {
            field: "capabilities[]",
            value: other.to_string(),
        }),
    }
}

/// Parses and validates one `/agent/events` POST body.
pub fn parse_wire_event(body: &[u8]) -> Result<ParsedAgentEvent, AdapterError> {
    let wire: WireEvent =
        serde_json::from_slice(body).map_err(|e| AdapterError::MalformedJson(e.to_string()))?;

    let schema_version = wire
        .schema_version
        .ok_or_else(|| AdapterError::MalformedJson("missing schemaVersion".to_string()))?;
    if schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(AdapterError::UnsupportedSchemaVersion(schema_version));
    }

    let event_id = wire
        .event_id
        .map(|s| sanitize_id(&s))
        .filter(|s| !s.is_empty())
        .ok_or(AdapterError::MissingIdentity("eventId"))?;

    let runtime_raw = wire
        .runtime
        .ok_or(AdapterError::MissingIdentity("runtime"))?;
    let runtime = parse_runtime(&runtime_raw)?;

    let session_id = wire
        .session_id
        .map(|s| sanitize_id(&s))
        .filter(|s| !s.is_empty())
        .ok_or(AdapterError::MissingIdentity("sessionId"))?;
    let session_key = AgentSessionKey::new(runtime, session_id)
        .map_err(|ModelError::EmptyNativeSessionId| AdapterError::MissingIdentity("sessionId"))?;

    let native_event = wire
        .native_event
        .map(|s| sanitize_id(&s))
        .ok_or_else(|| AdapterError::MalformedJson("missing nativeEvent".to_string()))?;

    let kind_raw = wire
        .kind
        .ok_or_else(|| AdapterError::MalformedJson("missing kind".to_string()))?;
    let kind = parse_kind(&kind_raw)?;

    let state_raw = wire
        .state
        .ok_or_else(|| AdapterError::MalformedJson("missing state".to_string()))?;
    let declared_state = parse_state(&state_raw)?;

    let terminal = wire.terminal.unwrap_or(false);
    let summary = wire.summary.and_then(|s| sanitize_summary(&s));

    let details: Vec<AgentDetail> = wire
        .details
        .unwrap_or_default()
        .into_iter()
        .map(|d| AgentDetail {
            label: sanitize_detail_label(&d.label.unwrap_or_default()),
            value: sanitize_detail_value(&d.value.unwrap_or_default()),
        })
        .filter(|d| !d.label.is_empty())
        .take(MAX_DETAILS)
        .collect();

    let mut capabilities = Vec::new();
    for raw in wire
        .capabilities
        .unwrap_or_default()
        .into_iter()
        .take(MAX_CAPABILITIES)
    {
        capabilities.push(parse_capability(&raw)?);
    }

    let project = wire.project.map(|p| AgentProject {
        name: p.name.and_then(|s| sanitize_name_or_label(&s)),
        cwd: p.cwd.and_then(|s| sanitize_value(&s)),
    });

    let host = wire.host.map(|h| AgentHost {
        name: h.name.and_then(|s| sanitize_name_or_label(&s)),
        bundle_id: h
            .bundle_id
            .map(|s| sanitize_id(&s))
            .filter(|s| !s.is_empty()),
    });

    let subagent = match wire.subagent {
        Some(s) => {
            let id = s
                .id
                .map(|v| sanitize_id(&v))
                .filter(|v| !v.is_empty())
                .ok_or_else(|| AdapterError::MalformedJson("subagent.id required".to_string()))?;
            Some(AgentSubagentSummary {
                id,
                label: s.label.and_then(|v| sanitize_name_or_label(&v)),
                state: s.state.and_then(|v| sanitize_name_or_label(&v)),
            })
        }
        None => None,
    };
    debug_assert!(subagent.iter().count() <= MAX_SUBAGENTS_PER_EVENT);

    let event = AgentEvent {
        declared_state,
        event_id,
        session_key,
        sequence: wire.sequence,
        kind,
        terminal,
        capabilities,
        summary,
        details,
        project,
        host,
        subagent,
    };

    Ok(ParsedAgentEvent {
        event,
        native_event,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_body() -> String {
        r#"{
            "schemaVersion": 1,
            "eventId": "e1",
            "runtime": "codex",
            "sessionId": "s1",
            "occurredAtMs": 1785067200000,
            "sequence": 12,
            "nativeEvent": "PermissionRequest",
            "kind": "permission_requested",
            "state": "waiting_for_permission",
            "summary": "Approval needed to run a command",
            "details": [{"label": "Tool", "value": "shell"}],
            "capabilities": ["session_lifecycle", "permission_requests"],
            "project": {"name": "notchtap", "cwd": "/Users/example/code/notchtap"},
            "host": {"name": "T3 Code", "bundleId": "validated.adapter-owned.value"},
            "subagent": {"id": "sub-1", "label": "test runner", "state": "working"},
            "terminal": false
        }"#
        .to_string()
    }

    #[test]
    fn valid_event_parses_and_maps_every_field() {
        let parsed = parse_wire_event(valid_body().as_bytes()).unwrap();
        assert_eq!(parsed.event.event_id, "e1");
        assert_eq!(parsed.event.session_key.runtime, AgentRuntime::Codex);
        assert_eq!(parsed.event.session_key.native_session_id, "s1");
        assert_eq!(parsed.event.sequence, Some(12));
        assert_eq!(parsed.native_event, "PermissionRequest");
        assert_eq!(parsed.event.kind, AgentEventKind::PermissionRequested);
        assert_eq!(
            parsed.event.summary.as_deref(),
            Some("Approval needed to run a command")
        );
        assert_eq!(parsed.event.details.len(), 1);
        assert_eq!(parsed.event.details[0].label, "Tool");
        assert_eq!(parsed.event.details[0].value, "shell");
        assert_eq!(
            parsed.event.capabilities,
            vec![
                AgentCapability::SessionLifecycle,
                AgentCapability::PermissionRequests
            ]
        );
        let project = parsed.event.project.unwrap();
        assert_eq!(project.name.as_deref(), Some("notchtap"));
        assert_eq!(project.cwd.as_deref(), Some("/Users/example/code/notchtap"));
        let host = parsed.event.host.unwrap();
        assert_eq!(host.name.as_deref(), Some("T3 Code"));
        assert_eq!(
            host.bundle_id.as_deref(),
            Some("validated.adapter-owned.value")
        );
        let subagent = parsed.event.subagent.unwrap();
        assert_eq!(subagent.id, "sub-1");
        assert_eq!(subagent.label.as_deref(), Some("test runner"));
        assert_eq!(subagent.state.as_deref(), Some("working"));
        assert!(!parsed.event.terminal);
    }

    #[test]
    fn minimal_event_omits_every_optional_field() {
        let body = r#"{
            "schemaVersion": 1,
            "eventId": "e1",
            "runtime": "codex",
            "sessionId": "s1",
            "nativeEvent": "Stop",
            "kind": "completed",
            "state": "completed",
            "terminal": true
        }"#;
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        assert_eq!(parsed.event.sequence, None);
        assert_eq!(parsed.event.summary, None);
        assert!(parsed.event.details.is_empty());
        assert!(parsed.event.capabilities.is_empty());
        assert_eq!(parsed.event.project, None);
        assert_eq!(parsed.event.host, None);
        assert_eq!(parsed.event.subagent, None);
        assert!(parsed.event.terminal);
    }

    #[test]
    fn unknown_schema_version_is_rejected() {
        let body = r#"{"schemaVersion": 2, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed"}"#;
        assert_eq!(
            parse_wire_event(body.as_bytes()).unwrap_err(),
            AdapterError::UnsupportedSchemaVersion(2)
        );
    }

    #[test]
    fn missing_schema_version_is_rejected() {
        let body = r#"{"eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed"}"#;
        assert!(matches!(
            parse_wire_event(body.as_bytes()),
            Err(AdapterError::MalformedJson(_))
        ));
    }

    #[test]
    fn garbage_json_is_rejected() {
        assert!(matches!(
            parse_wire_event(b"{not json"),
            Err(AdapterError::MalformedJson(_))
        ));
    }

    #[test]
    fn missing_event_id_is_missing_identity() {
        let body = r#"{"schemaVersion": 1, "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed"}"#;
        assert_eq!(
            parse_wire_event(body.as_bytes()).unwrap_err(),
            AdapterError::MissingIdentity("eventId")
        );
    }

    #[test]
    fn empty_event_id_is_missing_identity() {
        let body = r#"{"schemaVersion": 1, "eventId": "   ", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed"}"#;
        assert_eq!(
            parse_wire_event(body.as_bytes()).unwrap_err(),
            AdapterError::MissingIdentity("eventId")
        );
    }

    #[test]
    fn missing_session_id_is_missing_identity() {
        let body = r#"{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "nativeEvent": "x", "kind": "completed", "state": "completed"}"#;
        assert_eq!(
            parse_wire_event(body.as_bytes()).unwrap_err(),
            AdapterError::MissingIdentity("sessionId")
        );
    }

    #[test]
    fn missing_runtime_is_missing_identity() {
        let body = r#"{"schemaVersion": 1, "eventId": "e", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed"}"#;
        assert_eq!(
            parse_wire_event(body.as_bytes()).unwrap_err(),
            AdapterError::MissingIdentity("runtime")
        );
    }

    #[test]
    fn unsupported_runtime_is_rejected() {
        let body = r#"{"schemaVersion": 1, "eventId": "e", "runtime": "cursor", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed"}"#;
        assert_eq!(
            parse_wire_event(body.as_bytes()).unwrap_err(),
            AdapterError::UnsupportedRuntime("cursor".to_string())
        );
    }

    #[test]
    fn every_declared_runtime_is_accepted() {
        for (wire, expected) in [
            ("claude-code", AgentRuntime::ClaudeCode),
            ("codex", AgentRuntime::Codex),
            ("kimi", AgentRuntime::Kimi),
            ("opencode", AgentRuntime::OpenCode),
        ] {
            let body = format!(
                r#"{{"schemaVersion": 1, "eventId": "e", "runtime": "{wire}", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed"}}"#
            );
            let parsed = parse_wire_event(body.as_bytes()).unwrap();
            assert_eq!(parsed.event.session_key.runtime, expected);
        }
    }

    #[test]
    fn malformed_kind_enum_is_rejected() {
        let body = r#"{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "made_up", "state": "completed"}"#;
        assert_eq!(
            parse_wire_event(body.as_bytes()).unwrap_err(),
            AdapterError::MalformedEnum {
                field: "kind",
                value: "made_up".to_string()
            }
        );
    }

    #[test]
    fn malformed_state_enum_is_rejected() {
        let body = r#"{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "made_up"}"#;
        assert_eq!(
            parse_wire_event(body.as_bytes()).unwrap_err(),
            AdapterError::MalformedEnum {
                field: "state",
                value: "made_up".to_string()
            }
        );
    }

    #[test]
    fn malformed_capability_enum_is_rejected() {
        let body = r#"{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed", "capabilities": ["made_up"]}"#;
        assert_eq!(
            parse_wire_event(body.as_bytes()).unwrap_err(),
            AdapterError::MalformedEnum {
                field: "capabilities[]",
                value: "made_up".to_string()
            }
        );
    }

    #[test]
    fn subagent_missing_id_is_rejected() {
        let body = r#"{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed", "subagent": {"label": "x"}}"#;
        assert!(matches!(
            parse_wire_event(body.as_bytes()),
            Err(AdapterError::MalformedJson(_))
        ));
    }

    #[test]
    fn id_cap_exactly_256_bytes_is_kept_whole() {
        let id = "e".repeat(MAX_ID_BYTES);
        let body = format!(
            r#"{{"schemaVersion": 1, "eventId": "{id}", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed"}}"#
        );
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        assert_eq!(parsed.event.event_id.len(), MAX_ID_BYTES);
    }

    #[test]
    fn id_cap_above_256_bytes_is_truncated() {
        let id = "e".repeat(MAX_ID_BYTES + 10);
        let body = format!(
            r#"{{"schemaVersion": 1, "eventId": "{id}", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed"}}"#
        );
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        assert_eq!(parsed.event.event_id.len(), MAX_ID_BYTES);
    }

    #[test]
    fn id_cap_truncation_does_not_split_a_multibyte_codepoint() {
        let id = "é".repeat(MAX_ID_BYTES); // 2 * MAX_ID_BYTES bytes total
        let body = format!(
            r#"{{"schemaVersion": 1, "eventId": "{id}", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed"}}"#
        );
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        assert!(parsed.event.event_id.len() <= MAX_ID_BYTES);
        assert!(String::from_utf8(parsed.event.event_id.into_bytes()).is_ok());
    }

    #[test]
    fn summary_cap_exactly_500_scalars_is_kept_whole() {
        let summary = "s".repeat(MAX_SUMMARY_SCALARS);
        let body = wrap_summary(&summary);
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        assert_eq!(
            parsed.event.summary.unwrap().chars().count(),
            MAX_SUMMARY_SCALARS
        );
    }

    #[test]
    fn summary_cap_above_500_scalars_is_truncated() {
        let summary = "s".repeat(MAX_SUMMARY_SCALARS + 10);
        let body = wrap_summary(&summary);
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        assert_eq!(
            parsed.event.summary.unwrap().chars().count(),
            MAX_SUMMARY_SCALARS
        );
    }

    fn wrap_summary(summary: &str) -> String {
        format!(
            r#"{{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed", "summary": "{summary}"}}"#
        )
    }

    #[test]
    fn name_and_label_cap_at_120_scalars_exactly_kept_above_truncated() {
        let at_cap = "n".repeat(MAX_NAME_OR_LABEL_SCALARS);
        let over_cap = "n".repeat(MAX_NAME_OR_LABEL_SCALARS + 10);
        for (name, expected) in [
            (at_cap, MAX_NAME_OR_LABEL_SCALARS),
            (over_cap, MAX_NAME_OR_LABEL_SCALARS),
        ] {
            let body = format!(
                r#"{{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed", "project": {{"name": "{name}"}}}}"#
            );
            let parsed = parse_wire_event(body.as_bytes()).unwrap();
            assert_eq!(
                parsed.event.project.unwrap().name.unwrap().chars().count(),
                expected
            );
        }
    }

    #[test]
    fn cwd_and_detail_value_cap_at_1024_scalars_exactly_kept_above_truncated() {
        let at_cap = "c".repeat(MAX_VALUE_SCALARS);
        let over_cap = "c".repeat(MAX_VALUE_SCALARS + 10);
        for (cwd, expected) in [(at_cap, MAX_VALUE_SCALARS), (over_cap, MAX_VALUE_SCALARS)] {
            let body = format!(
                r#"{{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed", "project": {{"cwd": "{cwd}"}}}}"#
            );
            let parsed = parse_wire_event(body.as_bytes()).unwrap();
            assert_eq!(
                parsed.event.project.unwrap().cwd.unwrap().chars().count(),
                expected
            );
        }
    }

    #[test]
    fn details_cap_exactly_12_is_kept_whole() {
        let details: Vec<String> = (0..MAX_DETAILS)
            .map(|i| format!(r#"{{"label": "L{i}", "value": "v{i}"}}"#))
            .collect();
        let body = format!(
            r#"{{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed", "details": [{}]}}"#,
            details.join(",")
        );
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        assert_eq!(parsed.event.details.len(), MAX_DETAILS);
    }

    #[test]
    fn details_cap_above_12_is_truncated_to_12() {
        let details: Vec<String> = (0..(MAX_DETAILS + 5))
            .map(|i| format!(r#"{{"label": "L{i}", "value": "v{i}"}}"#))
            .collect();
        let body = format!(
            r#"{{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed", "details": [{}]}}"#,
            details.join(",")
        );
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        assert_eq!(parsed.event.details.len(), MAX_DETAILS);
    }

    #[test]
    fn detail_with_empty_label_is_dropped() {
        let body = r#"{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed", "details": [{"label": "", "value": "v"}, {"label": "Kept", "value": "v"}]}"#;
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        assert_eq!(parsed.event.details.len(), 1);
        assert_eq!(parsed.event.details[0].label, "Kept");
    }

    #[test]
    fn capabilities_cap_exactly_16_is_kept_whole() {
        let all = [
            "session_lifecycle",
            "permission_requests",
            "input_required",
            "completion",
            "failure",
            "tool_details",
            "subagents",
            "open_or_focus",
        ];
        let caps: Vec<&str> = all.iter().chain(all.iter()).copied().collect();
        assert_eq!(caps.len(), MAX_CAPABILITIES);
        let caps_json = caps
            .iter()
            .map(|c| format!(r#""{c}""#))
            .collect::<Vec<_>>()
            .join(",");
        let body = format!(
            r#"{{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed", "capabilities": [{caps_json}]}}"#
        );
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        assert_eq!(parsed.event.capabilities.len(), MAX_CAPABILITIES);
    }

    #[test]
    fn capabilities_cap_above_16_is_truncated_to_16() {
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
        let caps_json = (0..(MAX_CAPABILITIES + 1))
            .map(|i| format!(r#""{}""#, known[i % known.len()]))
            .collect::<Vec<_>>()
            .join(",");
        let body = format!(
            r#"{{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed", "capabilities": [{caps_json}]}}"#
        );
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        assert_eq!(parsed.event.capabilities.len(), MAX_CAPABILITIES);
    }

    #[test]
    fn max_subagents_per_event_cap_is_sixteen() {
        assert_eq!(MAX_SUBAGENTS_PER_EVENT, 16);
    }

    #[test]
    fn transitions_and_event_id_caps_are_reexported_from_registry() {
        assert_eq!(MAX_TRANSITIONS_PER_SESSION, 50);
        assert_eq!(MAX_REMEMBERED_EVENT_IDS, 2048);
    }

    #[test]
    fn control_characters_are_stripped_and_whitespace_trimmed() {
        let raw_summary = "  hascontrol\u{7}chars  ";
        let body = serde_json::json!({
            "schemaVersion": 1,
            "eventId": "e",
            "runtime": "codex",
            "sessionId": "s",
            "nativeEvent": "x",
            "kind": "completed",
            "state": "completed",
            "summary": raw_summary,
        })
        .to_string();
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        let summary = parsed.event.summary.unwrap();
        assert_eq!(summary, "hascontrolchars");
    }

    #[test]
    fn project_and_host_persist_when_omitted_vs_present() {
        let body = r#"{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed"}"#;
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        assert_eq!(parsed.event.project, None);
        assert_eq!(parsed.event.host, None);
    }

    #[test]
    fn sequence_is_passed_through_unchanged() {
        let body = r#"{"schemaVersion": 1, "eventId": "e", "runtime": "codex", "sessionId": "s", "nativeEvent": "x", "kind": "completed", "state": "completed", "sequence": 42}"#;
        let parsed = parse_wire_event(body.as_bytes()).unwrap();
        assert_eq!(parsed.event.sequence, Some(42));
    }

    #[test]
    fn runtime_wire_label_round_trips_every_variant_through_parse_runtime() {
        for runtime in [
            AgentRuntime::ClaudeCode,
            AgentRuntime::Codex,
            AgentRuntime::Kimi,
            AgentRuntime::OpenCode,
        ] {
            let label = runtime_wire_label(runtime);
            assert_eq!(parse_runtime(label).unwrap(), runtime);
        }
    }

    #[test]
    fn kind_wire_label_round_trips_every_variant_through_parse_kind() {
        for kind in [
            AgentEventKind::PermissionRequested,
            AgentEventKind::InputRequired,
            AgentEventKind::Completed,
            AgentEventKind::Failed,
            AgentEventKind::Informational,
        ] {
            let label = kind_wire_label(kind);
            assert_eq!(parse_kind(label).unwrap(), kind);
        }
    }

    #[test]
    fn state_wire_label_round_trips_every_variant_through_parse_state() {
        for state in [
            AgentSessionState::Starting,
            AgentSessionState::Working,
            AgentSessionState::WaitingForPermission,
            AgentSessionState::WaitingForInput,
            AgentSessionState::Completed,
            AgentSessionState::Failed,
            AgentSessionState::Stale,
        ] {
            let label = state_wire_label(state);
            assert_eq!(parse_state(label).unwrap(), state);
        }
    }

    #[test]
    fn capability_wire_label_round_trips_every_variant_through_parse_capability() {
        for capability in [
            AgentCapability::SessionLifecycle,
            AgentCapability::PermissionRequests,
            AgentCapability::InputRequired,
            AgentCapability::Completion,
            AgentCapability::Failure,
            AgentCapability::ToolDetails,
            AgentCapability::Subagents,
            AgentCapability::OpenOrFocus,
        ] {
            let label = capability_wire_label(capability);
            assert_eq!(parse_capability(label).unwrap(), capability);
        }
    }
}
