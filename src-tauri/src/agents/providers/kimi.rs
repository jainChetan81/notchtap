//! Pure Kimi Code hook-payload parser; the version gate deciding
//! whether a local Kimi even supports hooks lives in
//! [`super::kimi_version`]. [`normalize`] takes the raw JSON bytes Kimi
//! writes to a hook command's stdin and returns a
//! [`super::wire::NormalizedEvent`].
//!
//! ## Field-name assumption: NEEDS VERIFICATION
//!
//! Kimi's hooks page documents the event list and base payload shape
//! but no per-event field tables; its event names match Claude Code's
//! set verbatim, so this parser assumes the SAME field names Claude
//! Code's documented payloads use (`source`, `end_reason`, `tool_name`,
//! `tool_input`, `notification_type`, `error_type`, `agent_id`,
//! `agent_type`) rather than inventing new ones. A recorded assumption,
//! not a verified fact — confirm against a real Kimi hook payload
//! before treating it as load-bearing; fixtures are shaped so real
//! payloads differing is a one-file diff.
//!
//! `notification_type` values are a second, narrower assumption:
//! [`classify_notification`] recognizes
//! `permission_prompt`/`idle_prompt`/`agent_needs_input` as the closed
//! enum; everything else — including Kimi's documented `task.completed`
//! — falls through to `Informational`, never inferred from `message`
//! wording.
//!
//! ## Sanitization
//!
//! Identical discipline to `claude_code.rs`: `tool_name` forwarded as
//! a short identifier; `tool_input`/`tool_result` never forwarded
//! wholesale (`command` is never read); `Path` detail only from a known
//! path-shaped key, basename only; free-text fields never forwarded.

use thiserror::Error;

use super::wire::NormalizedEvent;

/// Kimi's declared capability set — the full Claude-Code-equivalent
/// set. Sent unchanged on every event this parser produces.
pub const CAPABILITIES: [&str; 7] = [
    "session_lifecycle",
    "permission_requests",
    "input_required",
    "completion",
    "failure",
    "tool_details",
    "subagents",
];

/// Typed parse errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum KimiParseError {
    #[error("malformed json: {0}")]
    MalformedJson(String),
    #[error("missing or empty session_id")]
    MissingSessionId,
    #[error("missing hook_event_name")]
    MissingHookEventName,
    #[error("unsupported hook_event_name: {0}")]
    UnsupportedHookEvent(String),
}

/// The raw wire shape ASSUMED for Kimi Code hooks — see the module doc.
/// Every field is `Option` so a payload missing a field this parser
/// doesn't use for a given event still deserializes cleanly.
#[derive(Debug, serde::Deserialize)]
struct RawHookPayload {
    session_id: Option<String>,
    hook_event_name: Option<String>,
    cwd: Option<String>,
    // SessionStart
    source: Option<String>,
    // SessionEnd
    end_reason: Option<String>,
    // PermissionRequest / PostToolUse / PostToolUseFailure
    tool_name: Option<String>,
    tool_input: Option<serde_json::Value>,
    // Notification
    notification_type: Option<String>,
    // StopFailure
    error_type: Option<String>,
    // SubagentStart / SubagentStop
    agent_id: Option<String>,
    agent_type: Option<String>,
}

/// One `(label, value)`-shaped intermediate the match arms below build
/// before wrapping into a [`NormalizedEvent`].
struct Mapped {
    kind: &'static str,
    state: &'static str,
    terminal: bool,
    summary: Option<String>,
    details: Vec<(String, String)>,
    subagent: Option<(String, Option<String>, Option<String>)>,
}

fn basename(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
}

/// Never empty — a missing/blank `tool_name` becomes the generic
/// `"a tool"` rather than an empty detail value. Mirrors
/// `claude_code::safe_tool_name`.
fn safe_tool_name(tool_name: Option<&str>) -> String {
    tool_name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("a tool")
        .to_string()
}

/// Pulls a basename-only path detail out of `tool_input` — only from a
/// small, known set of path-shaped keys, NEVER from `command`. Mirrors
/// `claude_code::safe_path_detail`.
fn safe_path_detail(tool_input: Option<&serde_json::Value>) -> Option<(String, String)> {
    let obj = tool_input?.as_object()?;
    for key in ["file_path", "path", "notebook_path"] {
        if let Some(raw) = obj.get(key).and_then(|v| v.as_str()) {
            let value = basename(raw).unwrap_or_else(|| raw.to_string());
            return Some(("Path".to_string(), value));
        }
    }
    None
}

/// Switches on the closed `notification_type` enum only, never `message`
/// — see this module's top doc for the assumed-values caveat.
fn classify_notification(notification_type: Option<&str>) -> Mapped {
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

fn map_event(hook_event_name: &str, payload: &RawHookPayload) -> Result<Mapped, KimiParseError> {
    let mapped = match hook_event_name {
        "SessionStart" => {
            let source = payload.source.as_deref().unwrap_or("startup");
            Mapped {
                kind: "informational",
                state: "starting",
                terminal: false,
                summary: Some(format!("Session started ({source})")),
                details: Vec::new(),
                subagent: None,
            }
        }
        "SessionEnd" => {
            let reason = payload.end_reason.as_deref().unwrap_or("other");
            Mapped {
                kind: "completed",
                state: "completed",
                terminal: true,
                summary: Some(format!("Session ended ({reason})")),
                details: Vec::new(),
                subagent: None,
            }
        }
        "PermissionRequest" => {
            let tool = safe_tool_name(payload.tool_name.as_deref());
            let mut details = vec![("Tool".to_string(), tool.clone())];
            if let Some(pair) = safe_path_detail(payload.tool_input.as_ref()) {
                details.push(pair);
            }
            Mapped {
                kind: "permission_requested",
                state: "waiting_for_permission",
                terminal: false,
                summary: Some(format!("Approval needed to run {tool}")),
                details,
                subagent: None,
            }
        }
        "Notification" => classify_notification(payload.notification_type.as_deref()),
        // `Stop` fires once per turn, not once per session —
        // non-terminal, mirrors `claude_code.rs`'s `"Stop"` arm.
        "Stop" => Mapped {
            kind: "completed",
            state: "completed",
            terminal: false,
            summary: Some("Turn completed".to_string()),
            details: Vec::new(),
            subagent: None,
        },
        // `StopFailure` fires at the same per-turn point, not session
        // end — non-terminal for the same reason as `claude_code.rs`'s
        // `"StopFailure"` arm.
        "StopFailure" => {
            let error_type = payload
                .error_type
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            Mapped {
                kind: "failed",
                state: "failed",
                terminal: false,
                summary: Some(format!("Turn ended due to a {error_type} error")),
                details: vec![("Error".to_string(), error_type)],
                subagent: None,
            }
        }
        "PostToolUse" => {
            let tool = safe_tool_name(payload.tool_name.as_deref());
            let mut details = vec![("Tool".to_string(), tool.clone())];
            if let Some(pair) = safe_path_detail(payload.tool_input.as_ref()) {
                details.push(pair);
            }
            Mapped {
                kind: "informational",
                state: "working",
                terminal: false,
                summary: Some(format!("Tool finished: {tool}")),
                details,
                subagent: None,
            }
        }
        "PostToolUseFailure" => {
            let tool = safe_tool_name(payload.tool_name.as_deref());
            Mapped {
                kind: "failed",
                // Non-terminal tool failure keeps the session `Working`
                // in the registry, same as Claude Code's equivalent.
                state: "working",
                terminal: false,
                summary: Some(format!("Tool failed: {tool}")),
                details: vec![("Tool".to_string(), tool)],
                subagent: None,
            }
        }
        "SubagentStart" => {
            let agent_type = payload
                .agent_type
                .clone()
                .unwrap_or_else(|| "subagent".to_string());
            let agent_id = payload
                .agent_id
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "unknown".to_string());
            Mapped {
                kind: "informational",
                state: "working",
                terminal: false,
                summary: Some(format!("Subagent started: {agent_type}")),
                details: Vec::new(),
                subagent: Some((agent_id, Some(agent_type), Some("working".to_string()))),
            }
        }
        "SubagentStop" => {
            let agent_type = payload
                .agent_type
                .clone()
                .unwrap_or_else(|| "subagent".to_string());
            let agent_id = payload
                .agent_id
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "unknown".to_string());
            Mapped {
                kind: "informational",
                state: "working",
                terminal: false,
                summary: Some(format!("Subagent finished: {agent_type}")),
                details: Vec::new(),
                subagent: Some((agent_id, Some(agent_type), Some("completed".to_string()))),
            }
        }
        other => {
            return Err(KimiParseError::UnsupportedHookEvent(other.to_string()));
        }
    };
    Ok(mapped)
}

/// Parses one Kimi Code hook stdin payload into a [`NormalizedEvent`].
/// Pure. Callers must run the version gate ([`super::kimi_version`])
/// BEFORE calling this — this function does not know or care what Kimi
/// version produced the payload.
pub fn normalize(stdin: &[u8]) -> Result<NormalizedEvent, KimiParseError> {
    let payload: RawHookPayload =
        serde_json::from_slice(stdin).map_err(|e| KimiParseError::MalformedJson(e.to_string()))?;

    let session_id = payload
        .session_id
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or(KimiParseError::MissingSessionId)?;

    let hook_event_name = payload
        .hook_event_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or(KimiParseError::MissingHookEventName)?;

    let mapped = map_event(&hook_event_name, &payload)?;

    let project_cwd = payload.cwd.clone().filter(|s| !s.trim().is_empty());
    let project_name = project_cwd.as_deref().and_then(basename);

    Ok(NormalizedEvent {
        session_id,
        native_event: hook_event_name,
        kind: mapped.kind,
        state: mapped.state,
        terminal: mapped.terminal,
        summary: mapped.summary,
        details: mapped.details,
        project_name,
        project_cwd,
        subagent: mapped.subagent,
        capabilities: CAPABILITIES.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::adapter::parse_wire_event;
    use crate::agents::providers::wire::build_wire_body;

    fn fixture(name: &str) -> &'static str {
        match name {
            "session-start" => include_str!("../../../tests/fixtures/kimi-session-start.json"),
            "session-end" => include_str!("../../../tests/fixtures/kimi-session-end.json"),
            "permission-request" => {
                include_str!("../../../tests/fixtures/kimi-permission-request.json")
            }
            "notification-permission" => {
                include_str!("../../../tests/fixtures/kimi-notification-permission.json")
            }
            "notification-idle" => {
                include_str!("../../../tests/fixtures/kimi-notification-idle.json")
            }
            "notification-generic" => {
                include_str!("../../../tests/fixtures/kimi-notification-generic.json")
            }
            "stop" => include_str!("../../../tests/fixtures/kimi-stop.json"),
            "stop-failure" => include_str!("../../../tests/fixtures/kimi-stop-failure.json"),
            "post-tool-use" => include_str!("../../../tests/fixtures/kimi-post-tool-use.json"),
            "post-tool-use-failure" => {
                include_str!("../../../tests/fixtures/kimi-post-tool-use-failure.json")
            }
            "subagent-start" => include_str!("../../../tests/fixtures/kimi-subagent-start.json"),
            "subagent-stop" => include_str!("../../../tests/fixtures/kimi-subagent-stop.json"),
            "post-tool-use-with-secret" => {
                include_str!("../../../tests/fixtures/kimi-post-tool-use-with-secret.json")
            }
            other => panic!("unknown fixture {other}"),
        }
    }

    // --- fixture-per-hook-event tests ------------------------------------

    #[test]
    fn session_start_maps_to_informational_starting() {
        let event = normalize(fixture("session-start").as_bytes()).unwrap();
        assert_eq!(event.session_id, "kimi-sess-redacted-0001");
        assert_eq!(event.native_event, "SessionStart");
        assert_eq!(event.kind, "informational");
        assert_eq!(event.state, "starting");
        assert!(!event.terminal);
        assert_eq!(event.summary.as_deref(), Some("Session started (startup)"));
        assert_eq!(
            event.project_cwd.as_deref(),
            Some("/Users/example/code/notchtap")
        );
        assert_eq!(event.project_name.as_deref(), Some("notchtap"));
        assert_eq!(event.capabilities, CAPABILITIES.to_vec());
    }

    #[test]
    fn session_end_maps_to_completed_terminal() {
        let event = normalize(fixture("session-end").as_bytes()).unwrap();
        assert_eq!(event.kind, "completed");
        assert_eq!(event.state, "completed");
        assert!(event.terminal);
        assert_eq!(event.summary.as_deref(), Some("Session ended (clear)"));
    }

    #[test]
    fn permission_request_maps_to_waiting_for_permission_with_safe_tool_detail() {
        let event = normalize(fixture("permission-request").as_bytes()).unwrap();
        assert_eq!(event.kind, "permission_requested");
        assert_eq!(event.state, "waiting_for_permission");
        assert!(!event.terminal);
        assert_eq!(
            event.summary.as_deref(),
            Some("Approval needed to run Bash")
        );
        assert_eq!(
            event.details,
            vec![("Tool".to_string(), "Bash".to_string())]
        );
    }

    #[test]
    fn notification_permission_prompt_maps_to_waiting_for_permission() {
        let event = normalize(fixture("notification-permission").as_bytes()).unwrap();
        assert_eq!(event.kind, "permission_requested");
        assert_eq!(event.state, "waiting_for_permission");
    }

    #[test]
    fn notification_idle_prompt_maps_to_waiting_for_input() {
        let event = normalize(fixture("notification-idle").as_bytes()).unwrap();
        assert_eq!(event.kind, "input_required");
        assert_eq!(event.state, "waiting_for_input");
    }

    #[test]
    fn notification_generic_maps_to_informational() {
        // notification_type "task.completed" is outside the recognized
        // closed enum — falls through to Informational, never inferred
        // from wording.
        let event = normalize(fixture("notification-generic").as_bytes()).unwrap();
        assert_eq!(event.kind, "informational");
        assert_eq!(event.state, "working");
    }

    #[test]
    fn stop_maps_to_completed_non_terminal() {
        // Per-turn Stop must not be terminal.
        let event = normalize(fixture("stop").as_bytes()).unwrap();
        assert_eq!(event.kind, "completed");
        assert!(
            !event.terminal,
            "per-turn Stop must not fragment the session into a terminal row"
        );
        assert_eq!(event.summary.as_deref(), Some("Turn completed"));
    }

    #[test]
    fn stop_failure_maps_to_failed_non_terminal_with_safe_error_type() {
        let event = normalize(fixture("stop-failure").as_bytes()).unwrap();
        assert_eq!(event.kind, "failed");
        assert!(
            !event.terminal,
            "a per-turn stop failure must not fragment the session into a terminal row"
        );
        assert_eq!(
            event.summary.as_deref(),
            Some("Turn ended due to a rate_limit error")
        );
        assert_eq!(
            event.details,
            vec![("Error".to_string(), "rate_limit".to_string())]
        );
    }

    #[test]
    fn post_tool_use_maps_to_informational_with_tool_and_path_details() {
        let event = normalize(fixture("post-tool-use").as_bytes()).unwrap();
        assert_eq!(event.kind, "informational");
        assert!(!event.terminal);
        assert_eq!(event.summary.as_deref(), Some("Tool finished: Edit"));
        assert_eq!(
            event.details,
            vec![
                ("Tool".to_string(), "Edit".to_string()),
                ("Path".to_string(), "mod.rs".to_string()),
            ]
        );
    }

    #[test]
    fn post_tool_use_failure_maps_to_failed_non_terminal() {
        let event = normalize(fixture("post-tool-use-failure").as_bytes()).unwrap();
        assert_eq!(event.kind, "failed");
        assert!(!event.terminal, "a tool failure alone must not be terminal");
        assert_eq!(event.summary.as_deref(), Some("Tool failed: Bash"));
    }

    #[test]
    fn subagent_start_carries_subagent_field() {
        let event = normalize(fixture("subagent-start").as_bytes()).unwrap();
        assert_eq!(event.kind, "informational");
        let (id, label, state) = event.subagent.unwrap();
        assert_eq!(id, "kimi-agent-redacted-01");
        assert_eq!(label.as_deref(), Some("general-purpose"));
        assert_eq!(state.as_deref(), Some("working"));
    }

    #[test]
    fn subagent_stop_carries_subagent_field() {
        let event = normalize(fixture("subagent-stop").as_bytes()).unwrap();
        assert_eq!(event.kind, "informational");
        let (id, label, state) = event.subagent.unwrap();
        assert_eq!(id, "kimi-agent-redacted-01");
        assert_eq!(label.as_deref(), Some("general-purpose"));
        assert_eq!(state.as_deref(), Some("completed"));
    }

    // --- sanitization: a fixture with a fake secret/full command line
    // never emits it ---------------------------------------------------

    #[test]
    fn secret_and_full_command_line_never_appear_in_normalized_output() {
        let event = normalize(fixture("post-tool-use-with-secret").as_bytes()).unwrap();
        let forbidden = [
            "sk-live-FAKESECRET1234567890",
            "Authorization",
            "Bearer",
            "curl",
        ];

        let mut haystack = String::new();
        if let Some(s) = &event.summary {
            haystack.push_str(s);
        }
        for (label, value) in &event.details {
            haystack.push_str(label);
            haystack.push_str(value);
        }

        for needle in forbidden {
            assert!(
                !haystack.contains(needle),
                "sanitized output must never contain {needle:?}, got {haystack:?}"
            );
        }
        assert_eq!(
            event.details,
            vec![("Tool".to_string(), "Bash".to_string())]
        );
    }

    // --- malformed input --------------------------------------------------

    #[test]
    fn garbage_json_is_rejected() {
        assert!(matches!(
            normalize(b"{not json"),
            Err(KimiParseError::MalformedJson(_))
        ));
    }

    #[test]
    fn missing_session_id_is_rejected() {
        let body = r#"{"hook_event_name": "Stop"}"#;
        assert_eq!(
            normalize(body.as_bytes()).unwrap_err(),
            KimiParseError::MissingSessionId
        );
    }

    #[test]
    fn missing_hook_event_name_is_rejected() {
        let body = r#"{"session_id": "s1"}"#;
        assert_eq!(
            normalize(body.as_bytes()).unwrap_err(),
            KimiParseError::MissingHookEventName
        );
    }

    #[test]
    fn unrecognized_hook_event_name_is_rejected() {
        let body = r#"{"session_id": "s1", "hook_event_name": "SomeFutureEvent"}"#;
        assert_eq!(
            normalize(body.as_bytes()).unwrap_err(),
            KimiParseError::UnsupportedHookEvent("SomeFutureEvent".to_string())
        );
    }

    // --- round-trip: every normalized+wire-built payload is accepted by
    // `agents::adapter::parse_wire_event` ----------------------------------

    #[test]
    fn every_fixture_round_trips_through_the_wire_adapter() {
        let names = [
            "session-start",
            "session-end",
            "permission-request",
            "notification-permission",
            "notification-idle",
            "notification-generic",
            "stop",
            "stop-failure",
            "post-tool-use",
            "post-tool-use-failure",
            "subagent-start",
            "subagent-stop",
            "post-tool-use-with-secret",
        ];
        for name in names {
            let event = normalize(fixture(name).as_bytes())
                .unwrap_or_else(|e| panic!("fixture {name} failed to normalize: {e}"));
            let body = build_wire_body("kimi", &event, "test-event-id", 1_785_067_200_000, None);
            let bytes = serde_json::to_vec(&body).unwrap();
            parse_wire_event(&bytes)
                .unwrap_or_else(|e| panic!("fixture {name}'s wire body was rejected: {e}"));
        }
    }

    // --- declared capabilities vs. fixture suite must agree ------------

    #[test]
    fn declared_capabilities_match_the_spec_1_kimi_row() {
        let expected: std::collections::BTreeSet<&str> = [
            "session_lifecycle",
            "permission_requests",
            "input_required",
            "completion",
            "failure",
            "tool_details",
            "subagents",
        ]
        .into_iter()
        .collect();
        let declared: std::collections::BTreeSet<&str> = CAPABILITIES.into_iter().collect();
        assert_eq!(declared, expected);
    }

    #[test]
    fn fixture_suite_exercises_every_declared_capability() {
        let exercised: std::collections::BTreeSet<&str> = [
            ("session-start", "session_lifecycle"),
            ("session-end", "session_lifecycle"),
            ("permission-request", "permission_requests"),
            ("notification-idle", "input_required"),
            ("stop", "completion"),
            ("stop-failure", "failure"),
            ("post-tool-use", "tool_details"),
            ("post-tool-use-failure", "failure"),
            ("subagent-start", "subagents"),
            ("subagent-stop", "subagents"),
        ]
        .into_iter()
        .map(|(_, capability)| capability)
        .collect();

        let declared: std::collections::BTreeSet<&str> = CAPABILITIES.into_iter().collect();
        assert_eq!(
            exercised, declared,
            "every declared capability must be exercised by the committed fixture suite, and vice versa"
        );

        for name in [
            "session-start",
            "session-end",
            "permission-request",
            "notification-idle",
            "stop",
            "stop-failure",
            "post-tool-use",
            "post-tool-use-failure",
            "subagent-start",
            "subagent-stop",
        ] {
            let event = normalize(fixture(name).as_bytes()).unwrap();
            assert_eq!(event.capabilities, CAPABILITIES.to_vec(), "fixture {name}");
        }
    }
}
