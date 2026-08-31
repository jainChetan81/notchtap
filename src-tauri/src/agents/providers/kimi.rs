//! Pure Kimi Code hook-payload parser; hook support gating lives in [`super::kimi_version`].

use thiserror::Error;

use super::wire::{
    basename, classify_notification, safe_path_detail, safe_tool_name, Mapped, NormalizedEvent,
    RawHookPayload,
};

/// Kimi's declared capability set — the full Claude-Code-equivalent set.
pub const CAPABILITIES: [&str; 7] = [
    "session_lifecycle",
    "permission_requests",
    "input_required",
    "completion",
    "failure",
    "tool_details",
    "subagents",
];

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
        "Stop" => Mapped {
            kind: "completed",
            state: "completed",
            terminal: false,
            summary: Some("Turn completed".to_string()),
            details: Vec::new(),
            subagent: None,
        },
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

/// Parses one payload after the caller has enforced the Kimi hook-version gate.
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
    fn notification_generic_maps_to_informational() {
        let event = normalize(fixture("notification-generic").as_bytes()).unwrap();
        assert_eq!(event.kind, "informational");
        assert_eq!(event.state, "working");
    }

    #[test]
    fn stop_maps_to_completed_non_terminal() {
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

    #[test]
    fn declared_capabilities_are_the_kimi_capability_set() {
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
