//! Plan 139 (v7 ticket 7 of 13, `docs/V7_AGENT_INTEGRATIONS_TECHNICAL_SPEC.md`
//! §4.3): the real, pure Codex hook-payload parser.
//!
//! [`normalize`] takes the raw JSON bytes Codex writes to a hook command's
//! stdin (one native payload per invocation) and returns a
//! [`super::wire::NormalizedEvent`] ready for
//! [`super::wire::build_wire_body`]. It is pure: no I/O, no clock read, no
//! randomness.
//!
//! ## Doc verification (this ticket's instructions: "build parsers against
//! the DOCUMENTED payload shapes and hook-event names")
//!
//! Verified against <https://developers.openai.com/codex/hooks> (redirects
//! to <https://learn.chatgpt.com/docs/hooks>) on 2026-07-26. The
//! documented lifecycle event set is: `SessionStart`, `SessionEnd`,
//! `PreToolUse`, `PermissionRequest`, `PostToolUse`, `PreCompact`,
//! `PostCompact`, `UserPromptSubmit`, `SubagentStart`, `SubagentStop`,
//! `Stop`. This parser handles the eight of those that carry Agent Board
//! lifecycle meaning per spec §4.3/this ticket's instructions
//! (`SessionStart`, `SessionEnd`, `PermissionRequest`, `Stop`,
//! `SubagentStart`, `SubagentStop`, `PreToolUse`, `PostToolUse`);
//! `PreCompact`/`PostCompact`/`UserPromptSubmit` are not Agent Board
//! lifecycle events (no permission/completion/failure/tool meaning) and
//! are intentionally left unmapped, same as `UserPromptSubmit` is left out
//! of Claude Code's ticket 138 set.
//!
//! ## Doc-vs-spec discrepancy: no documented Codex failure signal at all
//!
//! Spec §1's capability matrix lists Codex `failed` as "tool failure;
//! terminal failure is partial", and this ticket's own instructions
//! enumerate `failure` as a Codex capability to declare. The documented
//! Codex hook surface does **not** support this claim:
//!
//! - there is no `PostToolUseFailure`/`StopFailure` hook event documented
//!   for Codex (unlike Claude Code and Kimi, which both document both);
//! - `PostToolUse`'s `tool_response` field is documented only as "tool
//!   specific output... MCP tools send the MCP call result. Other local
//!   function tools normally send their model-facing output" — no
//!   `tool_use_succeeded`/`is_error`/exit-code field is documented on it,
//!   unlike Claude Code's `PostToolUse.tool_use_succeeded` boolean;
//! - `Stop`'s documented fields (`turn_id`, `stop_hook_active`,
//!   `last_assistant_message`) carry no error/failure field either.
//!
//! Per this ticket's instruction to follow the docs over the spec's
//! assumption and record the discrepancy: this parser does **not**
//! declare `failure` as a capability, and never emits `kind: "failed"`.
//! Inferring failure from `tool_response`/`last_assistant_message` text
//! would mean parsing wording to infer state, which every parser in this
//! module is built to never do (mirrors `claude_code.rs`'s
//! `classify_notification` discipline). If a future documented Codex
//! hook surface adds a structural failure signal, this is the one place
//! to add it — see spec §1's "each adapter... may add a capability after
//! a documented provider event has been verified".
//!
//! ## `input_required`: never (declared gap, spec §1's matrix)
//!
//! Codex has no `Notification` hook, no idle/waiting-for-input event, and
//! no legacy top-level `notify` mechanism is used here (spec §4.3: "single
//! user-global slot, poor contract" — deliberately not integrated). No
//! branch below ever produces `kind: "input_required"` /
//! `state: "waiting_for_input"`; see this module's tests for a
//! structural proof (every mapped event, plus the explicit unsupported
//! `"Notification"` case).
//!
//! ## Sanitization (spec §3.2)
//!
//! Same discipline as `claude_code.rs`: `tool_name` is forwarded as a
//! short identifier; `tool_input`/`tool_response` are never forwarded
//! wholesale (`tool_input.command`, `tool_input.description`, and
//! `tool_response` are never read by this parser at all); a `Path` detail
//! is extracted only from a known path-shaped `tool_input` key, basename
//! only; `last_assistant_message`/`error_message`-shaped free text is
//! never forwarded.

use thiserror::Error;

use super::wire::NormalizedEvent;

/// Spec §1's Codex capability row, restricted to what the documented hook
/// surface actually supports — see this module's top doc for why
/// `failure` and `input_required` are both absent from this set despite
/// appearing in spec §1's matrix / this ticket's original instructions.
pub const CAPABILITIES: [&str; 5] = [
    "session_lifecycle",
    "permission_requests",
    "completion",
    "tool_details",
    "subagents",
];

/// Typed parse errors (repo rule, CLAUDE.md: `thiserror` + matchable
/// variants for library/internal modules).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodexParseError {
    #[error("malformed json: {0}")]
    MalformedJson(String),
    #[error("missing or empty session_id")]
    MissingSessionId,
    #[error("missing hook_event_name")]
    MissingHookEventName,
    #[error("unsupported hook_event_name: {0}")]
    UnsupportedHookEvent(String),
}

/// The raw wire shape Codex hooks send (spec §4.3, doc-verified against
/// <https://developers.openai.com/codex/hooks> — see this module's top
/// doc) — every field is `Option` so a payload missing a field this
/// parser doesn't use for a given event still deserializes cleanly.
#[derive(Debug, serde::Deserialize)]
struct RawHookPayload {
    session_id: Option<String>,
    hook_event_name: Option<String>,
    cwd: Option<String>,
    // SessionStart
    source: Option<String>,
    // SessionEnd — documented as "for now, reason is always other", kept
    // as an open string field anyway rather than hardcoding "other" so a
    // future documented value still passes through unchanged.
    reason: Option<String>,
    // PermissionRequest / PreToolUse / PostToolUse
    tool_name: Option<String>,
    tool_input: Option<serde_json::Value>,
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
/// small, known set of path-shaped keys, NEVER from `command` (where a
/// full shell command line would live) or `description` (free text).
/// Mirrors `claude_code::safe_path_detail`.
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

fn map_event(hook_event_name: &str, payload: &RawHookPayload) -> Result<Mapped, CodexParseError> {
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
            let reason = payload.reason.as_deref().unwrap_or("other");
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
        "PreToolUse" => {
            let tool = safe_tool_name(payload.tool_name.as_deref());
            let mut details = vec![("Tool".to_string(), tool.clone())];
            if let Some(pair) = safe_path_detail(payload.tool_input.as_ref()) {
                details.push(pair);
            }
            Mapped {
                kind: "informational",
                state: "working",
                terminal: false,
                summary: Some(format!("Tool starting: {tool}")),
                details,
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
        // Operator decision 2026-07-26 (spec §2.1): `Stop` fires once per
        // turn, not once per session — non-terminal, so the registry
        // resolves it into `WaitingForInput` rather than a terminal state.
        // Mirrors `claude_code.rs`'s `"Stop"` arm.
        "Stop" => Mapped {
            kind: "completed",
            state: "completed",
            terminal: false,
            summary: Some("Turn completed".to_string()),
            details: Vec::new(),
            subagent: None,
        },
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
            return Err(CodexParseError::UnsupportedHookEvent(other.to_string()));
        }
    };
    Ok(mapped)
}

/// Parses one Codex hook stdin payload into a [`NormalizedEvent`]. Pure —
/// see this module's top doc.
pub fn normalize(stdin: &[u8]) -> Result<NormalizedEvent, CodexParseError> {
    let payload: RawHookPayload =
        serde_json::from_slice(stdin).map_err(|e| CodexParseError::MalformedJson(e.to_string()))?;

    let session_id = payload
        .session_id
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or(CodexParseError::MissingSessionId)?;

    let hook_event_name = payload
        .hook_event_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or(CodexParseError::MissingHookEventName)?;

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
            "session-start" => include_str!("../../../tests/fixtures/codex-session-start.json"),
            "session-end" => include_str!("../../../tests/fixtures/codex-session-end.json"),
            "permission-request" => {
                include_str!("../../../tests/fixtures/codex-permission-request.json")
            }
            "pre-tool-use" => include_str!("../../../tests/fixtures/codex-pre-tool-use.json"),
            "post-tool-use" => include_str!("../../../tests/fixtures/codex-post-tool-use.json"),
            "post-tool-use-with-secret" => {
                include_str!("../../../tests/fixtures/codex-post-tool-use-with-secret.json")
            }
            "stop" => include_str!("../../../tests/fixtures/codex-stop.json"),
            "subagent-start" => include_str!("../../../tests/fixtures/codex-subagent-start.json"),
            "subagent-stop" => include_str!("../../../tests/fixtures/codex-subagent-stop.json"),
            other => panic!("unknown fixture {other}"),
        }
    }

    // --- fixture-per-hook-event tests -----------------------------------

    #[test]
    fn session_start_maps_to_informational_starting() {
        let event = normalize(fixture("session-start").as_bytes()).unwrap();
        assert_eq!(event.session_id, "codex-sess-redacted-0001");
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
        assert_eq!(event.summary.as_deref(), Some("Session ended (other)"));
    }

    #[test]
    fn permission_request_maps_to_waiting_for_permission_with_safe_tool_detail() {
        let event = normalize(fixture("permission-request").as_bytes()).unwrap();
        assert_eq!(event.kind, "permission_requested");
        assert_eq!(event.state, "waiting_for_permission");
        assert!(!event.terminal);
        assert_eq!(
            event.summary.as_deref(),
            Some("Approval needed to run shell")
        );
        assert_eq!(
            event.details,
            vec![("Tool".to_string(), "shell".to_string())]
        );
    }

    #[test]
    fn pre_tool_use_maps_to_informational_working_with_tool_and_path_details() {
        let event = normalize(fixture("pre-tool-use").as_bytes()).unwrap();
        assert_eq!(event.kind, "informational");
        assert_eq!(event.state, "working");
        assert!(!event.terminal);
        assert_eq!(event.summary.as_deref(), Some("Tool starting: apply_patch"));
        assert_eq!(
            event.details,
            vec![
                ("Tool".to_string(), "apply_patch".to_string()),
                ("Path".to_string(), "mod.rs".to_string()),
            ]
        );
    }

    #[test]
    fn post_tool_use_maps_to_informational_working_with_tool_and_path_details() {
        let event = normalize(fixture("post-tool-use").as_bytes()).unwrap();
        assert_eq!(event.kind, "informational");
        assert_eq!(event.state, "working");
        assert!(!event.terminal);
        assert_eq!(event.summary.as_deref(), Some("Tool finished: apply_patch"));
        assert_eq!(
            event.details,
            vec![
                ("Tool".to_string(), "apply_patch".to_string()),
                ("Path".to_string(), "mod.rs".to_string()),
            ]
        );
    }

    #[test]
    fn stop_maps_to_completed_non_terminal() {
        // Operator decision 2026-07-26: per-turn Stop must not be terminal.
        let event = normalize(fixture("stop").as_bytes()).unwrap();
        assert_eq!(event.kind, "completed");
        assert!(
            !event.terminal,
            "per-turn Stop must not fragment the session into a terminal row"
        );
        assert_eq!(event.summary.as_deref(), Some("Turn completed"));
    }

    #[test]
    fn subagent_start_carries_subagent_field() {
        let event = normalize(fixture("subagent-start").as_bytes()).unwrap();
        assert_eq!(event.kind, "informational");
        let (id, label, state) = event.subagent.unwrap();
        assert_eq!(id, "codex-agent-redacted-01");
        assert_eq!(label.as_deref(), Some("general-purpose"));
        assert_eq!(state.as_deref(), Some("working"));
    }

    #[test]
    fn subagent_stop_carries_subagent_field() {
        let event = normalize(fixture("subagent-stop").as_bytes()).unwrap();
        assert_eq!(event.kind, "informational");
        let (id, label, state) = event.subagent.unwrap();
        assert_eq!(id, "codex-agent-redacted-01");
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
        // The only detail this event should carry is the safe tool name —
        // no `tool_response`/`tool_input.command` content leaks through.
        assert_eq!(
            event.details,
            vec![("Tool".to_string(), "shell".to_string())]
        );
    }

    // --- malformed input -------------------------------------------------

    #[test]
    fn garbage_json_is_rejected() {
        assert!(matches!(
            normalize(b"{not json"),
            Err(CodexParseError::MalformedJson(_))
        ));
    }

    #[test]
    fn missing_session_id_is_rejected() {
        let body = r#"{"hook_event_name": "Stop"}"#;
        assert_eq!(
            normalize(body.as_bytes()).unwrap_err(),
            CodexParseError::MissingSessionId
        );
    }

    #[test]
    fn missing_hook_event_name_is_rejected() {
        let body = r#"{"session_id": "s1"}"#;
        assert_eq!(
            normalize(body.as_bytes()).unwrap_err(),
            CodexParseError::MissingHookEventName
        );
    }

    #[test]
    fn unrecognized_hook_event_name_is_rejected() {
        let body = r#"{"session_id": "s1", "hook_event_name": "SomeFutureEvent"}"#;
        assert_eq!(
            normalize(body.as_bytes()).unwrap_err(),
            CodexParseError::UnsupportedHookEvent("SomeFutureEvent".to_string())
        );
    }

    // --- declared gap: Codex never emits InputRequired --------------------
    //
    // Structural proof, not just "we didn't write a branch for it": every
    // supported native event's mapped `kind`/`state` is asserted to never
    // be `input_required`/`waiting_for_input`, AND the one event name a
    // Claude-Code-shaped payload would use for idle/input notifications
    // (`"Notification"`, undocumented for Codex) is asserted to be
    // rejected as unsupported rather than silently accepted.

    #[test]
    fn no_supported_codex_event_ever_maps_to_input_required() {
        let names = [
            "session-start",
            "session-end",
            "permission-request",
            "pre-tool-use",
            "post-tool-use",
            "stop",
            "subagent-start",
            "subagent-stop",
        ];
        for name in names {
            let event = normalize(fixture(name).as_bytes()).unwrap();
            assert_ne!(
                event.kind, "input_required",
                "fixture {name} must never map to input_required (declared Codex gap)"
            );
            assert_ne!(
                event.state, "waiting_for_input",
                "fixture {name} must never map to waiting_for_input (declared Codex gap)"
            );
        }
    }

    #[test]
    fn undocumented_notification_event_name_is_rejected_not_mapped_to_input() {
        // Codex has no documented `Notification` hook (unlike Claude Code
        // and Kimi) — this parser must not grow a speculative mapping for
        // it. A payload using that event name is rejected as unsupported.
        let body = r#"{"session_id": "s1", "hook_event_name": "Notification", "notification_type": "idle_prompt"}"#;
        assert_eq!(
            normalize(body.as_bytes()).unwrap_err(),
            CodexParseError::UnsupportedHookEvent("Notification".to_string())
        );
    }

    // --- round-trip: every normalized+wire-built payload is accepted by
    // `agents::adapter::parse_wire_event` ---------------------------------

    #[test]
    fn every_fixture_round_trips_through_the_wire_adapter() {
        let names = [
            "session-start",
            "session-end",
            "permission-request",
            "pre-tool-use",
            "post-tool-use",
            "post-tool-use-with-secret",
            "stop",
            "subagent-start",
            "subagent-stop",
        ];
        for name in names {
            let event = normalize(fixture(name).as_bytes())
                .unwrap_or_else(|e| panic!("fixture {name} failed to normalize: {e}"));
            let body = build_wire_body("codex", &event, "test-event-id", 1_785_067_200_000, None);
            let bytes = serde_json::to_vec(&body).unwrap();
            parse_wire_event(&bytes)
                .unwrap_or_else(|e| panic!("fixture {name}'s wire body was rejected: {e}"));
        }
    }

    // --- declared capabilities vs. fixture suite must agree (spec §14) -

    #[test]
    fn declared_capabilities_match_the_verified_codex_row() {
        let expected: std::collections::BTreeSet<&str> = [
            "session_lifecycle",
            "permission_requests",
            "completion",
            "tool_details",
            "subagents",
        ]
        .into_iter()
        .collect();
        let declared: std::collections::BTreeSet<&str> = CAPABILITIES.into_iter().collect();
        assert_eq!(declared, expected);
        // And the two capabilities spec §1's matrix / this ticket's
        // instructions named but the docs don't support are absent.
        assert!(!declared.contains("input_required"));
        assert!(!declared.contains("failure"));
    }

    #[test]
    fn fixture_suite_exercises_every_declared_capability() {
        let exercised: std::collections::BTreeSet<&str> = [
            ("session-start", "session_lifecycle"),
            ("session-end", "session_lifecycle"),
            ("permission-request", "permission_requests"),
            ("stop", "completion"),
            ("pre-tool-use", "tool_details"),
            ("post-tool-use", "tool_details"),
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
            "stop",
            "pre-tool-use",
            "post-tool-use",
            "subagent-start",
            "subagent-stop",
        ] {
            let event = normalize(fixture(name).as_bytes()).unwrap();
            assert_eq!(event.capabilities, CAPABILITIES.to_vec(), "fixture {name}");
        }
    }
}
