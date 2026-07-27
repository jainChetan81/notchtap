//! Plan 138 (v7 ticket 6 of 13, `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md`
//! §4.2): the real, pure Claude Code hook-payload parser.
//!
//! [`normalize`] takes the raw JSON bytes Claude Code writes to a hook
//! command's stdin (one native payload per invocation — see
//! <https://code.claude.com/docs/en/hooks>) and returns a
//! [`super::wire::NormalizedEvent`] ready for
//! [`super::wire::build_wire_body`]. It is pure: no I/O, no clock read,
//! no randomness — every native event name this function recognizes is
//! read from the payload's own `hook_event_name` field, so a single
//! `notchtap-agent hook claude-code` command entry (registered against
//! every hook event Claude Code fires) is enough; there is no need for
//! the CLI invocation itself to know in advance which event is coming.
//!
//! ## Sanitization (spec §3.2)
//!
//! This parser is the one place that decides what Claude Code payload
//! content is safe to forward at all — the server-side caps in
//! `agents::adapter` bound length/count, but they can't undo a decision
//! made here to forward something that should never leave this process.
//! Concretely, this parser:
//!
//! - forwards `tool_name` (a short provider-defined identifier, e.g.
//!   `"Bash"`, `"Edit"`) — never `tool_input`/`tool_result` wholesale;
//! - extracts a `Path` detail ONLY by picking a known path-shaped key
//!   (`file_path`/`path`/`notebook_path`) out of `tool_input` and
//!   keeping just [`std::path::Path::file_name`] — never the full path,
//!   and never `tool_input.command` (the one place a full shell command
//!   would live);
//! - builds every `summary` from a fixed, bounded template plus a small
//!   number of already-sanitized fields (tool name, `source`/
//!   `end_reason`/`error_type` — all closed, provider-defined enum-ish
//!   strings) — it never forwards `message`, `last_assistant_message`,
//!   `error_message`, `task_description`, or any other free-text/
//!   model-authored field from the native payload;
//! - never inspects `tool_result` at all (success or failure) — the
//!   `tool_use_succeeded` boolean plus the tool name is the whole
//!   PostToolUse(Failure) story this parser tells.
//!
//! ## Notification (spec §4.2)
//!
//! `Notification`'s `notification_type` field is a closed enum
//! (`permission_prompt`, `idle_prompt`, `agent_needs_input`,
//! `auth_success`, `elicitation_dialog`, `elicitation_complete`,
//! `elicitation_response`, `agent_completed`) — [`classify_notification`]
//! switches on that field, never on `message` text, matching spec's
//! "wording is never parsed to infer state".

use thiserror::Error;

use super::wire::NormalizedEvent;

/// Spec §1's Claude Code capability row, restricted to the seven
/// capability strings this ticket's instructions enumerate (not
/// `open_or_focus`, which is Host-dependent and not part of what an
/// event's own `capabilities` array declares). Sent unchanged on every
/// event this parser produces — see [`NormalizedEvent::capabilities`]'s
/// doc for why a fixed per-provider set, not a per-event computation.
pub const CAPABILITIES: [&str; 7] = [
    "session_lifecycle",
    "permission_requests",
    "input_required",
    "completion",
    "failure",
    "tool_details",
    "subagents",
];

/// Typed parse errors (repo rule, CLAUDE.md: `thiserror` + matchable
/// variants for library/internal modules).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClaudeCodeParseError {
    #[error("malformed json: {0}")]
    MalformedJson(String),
    #[error("missing or empty session_id")]
    MissingSessionId,
    #[error("missing hook_event_name")]
    MissingHookEventName,
    #[error("unsupported hook_event_name: {0}")]
    UnsupportedHookEvent(String),
}

/// The raw wire shape Claude Code hooks send (spec §4.2, doc-verified
/// against <https://code.claude.com/docs/en/hooks>) — every field
/// is `Option` so a payload missing a field this parser doesn't use for
/// a given event still deserializes cleanly.
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
/// `"a tool"` rather than an empty detail value.
fn safe_tool_name(tool_name: Option<&str>) -> String {
    tool_name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("a tool")
        .to_string()
}

/// Pulls a basename-only path detail out of `tool_input` (spec §3.2:
/// "basename only for paths in summaries") — only from a small, known
/// set of path-shaped keys, NEVER from `command` (where a full shell
/// command line would live) or any other key. Returns `None` when
/// `tool_input` is absent, not an object, or has none of the known
/// keys as a string.
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

/// Spec §4.2: "`Notification` is accepted only for documented
/// permission/idle input notifications. generic notifications become
/// `Informational`; wording is not parsed to infer state." — this
/// switches on the closed `notification_type` enum only, never
/// `message`.
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

fn map_event(
    hook_event_name: &str,
    payload: &RawHookPayload,
) -> Result<Mapped, ClaudeCodeParseError> {
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
        // Operator decision 2026-07-26 (spec §2.1): `Stop` fires once per
        // turn, not once per session — the session stays live and the
        // registry resolves this non-terminal `completed` into
        // `WaitingForInput`, not a terminal state. `state` here is kept as
        // the wire label for the un-terminal-ized "completed" kind; the
        // registry (not this parser) is what actually lands the session in
        // `WaitingForInput` (`agents::registry::next_state`).
        "Stop" => Mapped {
            kind: "completed",
            state: "completed",
            terminal: false,
            summary: Some("Turn completed".to_string()),
            details: Vec::new(),
            subagent: None,
        },
        // `StopFailure` is `Stop`'s failure counterpart — it fires when the
        // SAME per-turn lifecycle point is reached abnormally, not when the
        // session itself ends. Treating it as terminal would fragment one
        // multi-turn session into a suffixed reuse key on every turn that
        // happens to fail (`AgentRegistry::apply_event`'s terminal-reuse
        // redirect), which is exactly the bug class the operator's Stop
        // decision was meant to close. So this is non-terminal too: the
        // session remains live (registry resolves non-terminal `Failed` to
        // `Working` — `agents::registry::next_state`) and only an explicit
        // `SessionEnd` closes the session for good.
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
                // Non-terminal tool failure: `terminal: false` keeps the
                // session `Working` in the registry (spec §2.1's
                // "AgentEventKind::Failed if terminal else Working" —
                // `agents::registry::next_state`).
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
            return Err(ClaudeCodeParseError::UnsupportedHookEvent(
                other.to_string(),
            ))
        }
    };
    Ok(mapped)
}

/// Parses one Claude Code hook stdin payload into a [`NormalizedEvent`].
/// Pure — see this module's top doc.
pub fn normalize(stdin: &[u8]) -> Result<NormalizedEvent, ClaudeCodeParseError> {
    let payload: RawHookPayload = serde_json::from_slice(stdin)
        .map_err(|e| ClaudeCodeParseError::MalformedJson(e.to_string()))?;

    let session_id = payload
        .session_id
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or(ClaudeCodeParseError::MissingSessionId)?;

    let hook_event_name = payload
        .hook_event_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or(ClaudeCodeParseError::MissingHookEventName)?;

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
            "session-start" => {
                include_str!("../../../tests/fixtures/claude-code-session-start.json")
            }
            "session-end" => include_str!("../../../tests/fixtures/claude-code-session-end.json"),
            "permission-request" => {
                include_str!("../../../tests/fixtures/claude-code-permission-request.json")
            }
            "notification-permission" => {
                include_str!("../../../tests/fixtures/claude-code-notification-permission.json")
            }
            "notification-idle" => {
                include_str!("../../../tests/fixtures/claude-code-notification-idle.json")
            }
            "notification-generic" => {
                include_str!("../../../tests/fixtures/claude-code-notification-generic.json")
            }
            "stop" => include_str!("../../../tests/fixtures/claude-code-stop.json"),
            "stop-failure" => include_str!("../../../tests/fixtures/claude-code-stop-failure.json"),
            "post-tool-use" => {
                include_str!("../../../tests/fixtures/claude-code-post-tool-use.json")
            }
            "post-tool-use-failure" => {
                include_str!("../../../tests/fixtures/claude-code-post-tool-use-failure.json")
            }
            "subagent-start" => {
                include_str!("../../../tests/fixtures/claude-code-subagent-start.json")
            }
            "subagent-stop" => {
                include_str!("../../../tests/fixtures/claude-code-subagent-stop.json")
            }
            "post-tool-use-with-secret" => {
                include_str!("../../../tests/fixtures/claude-code-post-tool-use-with-secret.json")
            }
            other => panic!("unknown fixture {other}"),
        }
    }

    // --- fixture-per-hook-event tests: normalized kind/state/capabilities/
    // sanitized fields -------------------------------------------------

    #[test]
    fn session_start_maps_to_informational_starting() {
        let event = normalize(fixture("session-start").as_bytes()).unwrap();
        assert_eq!(event.session_id, "sess-redacted-0001");
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
        let event = normalize(fixture("notification-generic").as_bytes()).unwrap();
        assert_eq!(event.kind, "informational");
        assert_eq!(event.state, "working");
    }

    #[test]
    fn stop_maps_to_completed_non_terminal() {
        // Operator decision 2026-07-26: per-turn Stop must not be terminal
        // — the session stays live (registry resolves this into
        // `WaitingForInput`, not the terminal `Completed` state).
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
        // `StopFailure` is `Stop`'s failure counterpart, firing at the same
        // per-turn point — same non-terminal treatment, for the same
        // reason (see this file's `map_event` doc comment on the
        // `"StopFailure"` arm).
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
        assert_eq!(id, "agent-redacted-01");
        assert_eq!(label.as_deref(), Some("general-purpose"));
        assert_eq!(state.as_deref(), Some("working"));
    }

    #[test]
    fn subagent_stop_carries_subagent_field() {
        let event = normalize(fixture("subagent-stop").as_bytes()).unwrap();
        assert_eq!(event.kind, "informational");
        let (id, label, state) = event.subagent.unwrap();
        assert_eq!(id, "agent-redacted-01");
        assert_eq!(label.as_deref(), Some("general-purpose"));
        assert_eq!(state.as_deref(), Some("completed"));
    }

    // --- sanitization: a fixture with a fake secret/full command line
    // never emits it (spec §3.2) --------------------------------------

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
        // The only detail this event should carry is the safe tool name.
        assert_eq!(
            event.details,
            vec![("Tool".to_string(), "Bash".to_string())]
        );
    }

    // --- malformed input ------------------------------------------------

    #[test]
    fn garbage_json_is_rejected() {
        assert!(matches!(
            normalize(b"{not json"),
            Err(ClaudeCodeParseError::MalformedJson(_))
        ));
    }

    #[test]
    fn missing_session_id_is_rejected() {
        let body = r#"{"hook_event_name": "Stop"}"#;
        assert_eq!(
            normalize(body.as_bytes()).unwrap_err(),
            ClaudeCodeParseError::MissingSessionId
        );
    }

    #[test]
    fn missing_hook_event_name_is_rejected() {
        let body = r#"{"session_id": "s1"}"#;
        assert_eq!(
            normalize(body.as_bytes()).unwrap_err(),
            ClaudeCodeParseError::MissingHookEventName
        );
    }

    #[test]
    fn unrecognized_hook_event_name_is_rejected() {
        let body = r#"{"session_id": "s1", "hook_event_name": "SomeFutureEvent"}"#;
        assert_eq!(
            normalize(body.as_bytes()).unwrap_err(),
            ClaudeCodeParseError::UnsupportedHookEvent("SomeFutureEvent".to_string())
        );
    }

    // --- round-trip: every normalized+wire-built payload is accepted by
    // `agents::adapter::parse_wire_event` (proves this parser emits
    // exactly what the endpoint accepts) --------------------------------

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
            let body = build_wire_body(
                "claude-code",
                &event,
                "test-event-id",
                1_785_067_200_000,
                None,
            );
            let bytes = serde_json::to_vec(&body).unwrap();
            parse_wire_event(&bytes)
                .unwrap_or_else(|e| panic!("fixture {name}'s wire body was rejected: {e}"));
        }
    }

    // --- declared capabilities vs. fixture suite must agree (spec §14) -

    #[test]
    fn declared_capabilities_match_the_spec_1_claude_code_row() {
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
        // Which capability each fixture is written to demonstrate, per
        // spec §1's Claude Code row. This is the "declaration and
        // fixture suite must agree" check (spec §14): every capability
        // this parser declares must have at least one fixture whose
        // native event is documented evidence of it, and nothing here
        // should need a capability the parser doesn't declare.
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

        // And every event that's supposed to demonstrate a capability
        // does in fact carry the full declared set on the wire (spec:
        // capabilities are sent on every event, not derived per-event).
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
