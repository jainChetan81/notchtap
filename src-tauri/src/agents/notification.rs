use uuid::Uuid;

use crate::event::{
    AgentSignal, DetailItem, Event, EventMeta, EventPayload, EventSignal as WireSignal, EventType,
    Priority, RotationSpec, SourceKind,
};

use super::adapter::{kind_wire_label, runtime_wire_label};
use super::model::{session_hash_hex, AgentDetail, AgentEventKind, AgentRuntime, AgentSessionKey};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotificationPolicy {
    pub informational_notifications: bool,
    pub completion_notifications: bool,
    pub permission_priority: Priority,
    pub input_priority: Priority,
    pub failure_priority: Priority,
    pub completion_priority: Priority,
}

impl Default for NotificationPolicy {
    /// `permission_priority = input_priority = failure_priority = "high"`, `completion_priority =
    /// "medium"`, `informational_notifications = false`, `completion_notifications = true`.
    fn default() -> Self {
        Self {
            informational_notifications: false,
            completion_notifications: true,
            permission_priority: Priority::High,
            input_priority: Priority::High,
            failure_priority: Priority::High,
            completion_priority: Priority::Medium,
        }
    }
}

/// Whether `kind` (+ `terminal`) is ever eligible to become a Notification, independent of queue
/// capacity.
pub fn is_noteworthy(kind: AgentEventKind, terminal: bool, policy: &NotificationPolicy) -> bool {
    match kind {
        AgentEventKind::PermissionRequested | AgentEventKind::InputRequired => true,
        AgentEventKind::Completed if terminal => policy.completion_notifications,
        AgentEventKind::Failed if terminal => true,
        AgentEventKind::Completed | AgentEventKind::Failed | AgentEventKind::Informational => {
            policy.informational_notifications
        }
    }
}

/// The Priority a noteworthy `kind` maps to. Only meaningful when [`is_noteworthy`] is true for the
/// same `(kind, terminal)` pair — callers must check that first.
pub fn priority_for(kind: AgentEventKind, terminal: bool, policy: &NotificationPolicy) -> Priority {
    match kind {
        AgentEventKind::PermissionRequested => policy.permission_priority,
        AgentEventKind::InputRequired => policy.input_priority,
        AgentEventKind::Completed if terminal => policy.completion_priority,
        AgentEventKind::Failed if terminal => policy.failure_priority,
        AgentEventKind::Completed | AgentEventKind::Failed | AgentEventKind::Informational => {
            Priority::Medium
        }
    }
}

fn runtime_display_name(runtime: AgentRuntime) -> &'static str {
    match runtime {
        AgentRuntime::ClaudeCode => "Claude Code",
        AgentRuntime::Codex => "Codex",
        AgentRuntime::Kimi => "Kimi",
        AgentRuntime::OpenCode => "OpenCode",
    }
}

/// Short, kind-specific title — `runtime_display_name` plus a fixed verb per kind, never derived
/// from `summary`: notification text is templated, never sniffed from provider payloads.
fn title_for(runtime: AgentRuntime, kind: AgentEventKind, terminal: bool) -> String {
    let name = runtime_display_name(runtime);
    match kind {
        AgentEventKind::PermissionRequested => format!("{name} needs permission"),
        AgentEventKind::InputRequired => format!("{name} needs input"),
        AgentEventKind::Completed if terminal => format!("{name} finished"),
        AgentEventKind::Completed => format!("{name} finished a turn"),
        AgentEventKind::Failed if terminal => format!("{name} failed"),
        AgentEventKind::Failed => format!("{name} hit a tool error"),
        AgentEventKind::Informational => format!("{name} update"),
    }
}

/// Fallback body when the wire event carried no `summary` (it is optional) — a Notification's
/// `body` is a required, non-optional `String` (`event.rs::EventPayload`).
fn default_body_for(kind: AgentEventKind, terminal: bool) -> String {
    match kind {
        AgentEventKind::PermissionRequested => "Approval needed to continue.".to_string(),
        AgentEventKind::InputRequired => "Waiting for your input.".to_string(),
        AgentEventKind::Completed if terminal => "Session completed.".to_string(),
        AgentEventKind::Completed => "Turn completed; the session is still open.".to_string(),
        AgentEventKind::Failed if terminal => "The session ended with an error.".to_string(),
        AgentEventKind::Failed => "A tool call failed; the session is still working.".to_string(),
        AgentEventKind::Informational => "Session update.".to_string(),
    }
}

pub struct NotificationContent<'a> {
    pub summary: Option<&'a str>,
    /// The project NAME (`AgentProject.name`), never the cwd — see [`build_notification`]'s own
    /// doc.
    pub project_name: Option<&'a str>,
    pub details: &'a [AgentDetail],
}

/// Returns no event when the kind is not noteworthy under the supplied policy.
pub fn build_notification(
    session_key: &AgentSessionKey,
    kind: AgentEventKind,
    terminal: bool,
    content: NotificationContent<'_>,
    ttl_secs: u64,
    policy: &NotificationPolicy,
) -> Option<Event> {
    let NotificationContent {
        summary,
        project_name,
        details,
    } = content;

    if !is_noteworthy(kind, terminal, policy) {
        return None;
    }

    let priority = priority_for(kind, terminal, policy);
    let title = title_for(session_key.runtime, kind, terminal);
    let body = summary
        .map(str::to_string)
        .unwrap_or_else(|| default_body_for(kind, terminal));

    Some(Event {
        id: Uuid::new_v4(),
        event_type: EventType::AgentEvent,
        priority,
        rotation: RotationSpec::OneShot { ttl_secs },
        topic: None,
        payload: EventPayload { title, body },
        meta: EventMeta {
            agent: Some(AgentSignal {
                runtime: runtime_wire_label(session_key.runtime).to_string(),
                kind: kind_wire_label(kind).to_string(),
                session_hash: session_hash_hex(session_key),
                summary: summary.map(str::to_string),
            }),
            subtitle: project_name.map(str::to_string),
            details: details
                .iter()
                .map(|d| DetailItem {
                    label: d.label.clone(),
                    value: d.value.clone(),
                })
                .collect(),
            ..EventMeta::default()
        },
        signal: WireSignal::Generic,
        origin: SourceKind::Agent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(runtime: AgentRuntime) -> AgentSessionKey {
        AgentSessionKey::new(runtime, "sess-1").unwrap()
    }

    #[test]
    fn permission_requested_is_high_one_shot() {
        let policy = NotificationPolicy::default();
        assert!(is_noteworthy(
            AgentEventKind::PermissionRequested,
            false,
            &policy
        ));
        assert_eq!(
            priority_for(AgentEventKind::PermissionRequested, false, &policy),
            Priority::High
        );
        let event = build_notification(
            &key(AgentRuntime::Codex),
            AgentEventKind::PermissionRequested,
            false,
            NotificationContent {
                summary: Some("Approval needed"),
                project_name: None,
                details: &[],
            },
            8,
            &policy,
        )
        .expect("permission_requested must produce a card");
        assert_eq!(event.priority, Priority::High);
        assert!(matches!(
            event.rotation,
            RotationSpec::OneShot { ttl_secs: 8 }
        ));
        assert_eq!(event.event_type, EventType::AgentEvent);
        assert_eq!(event.origin, SourceKind::Agent);
    }

    #[test]
    fn input_required_is_high_one_shot() {
        let policy = NotificationPolicy::default();
        assert!(is_noteworthy(AgentEventKind::InputRequired, false, &policy));
        assert_eq!(
            priority_for(AgentEventKind::InputRequired, false, &policy),
            Priority::High
        );
        let event = build_notification(
            &key(AgentRuntime::Kimi),
            AgentEventKind::InputRequired,
            false,
            NotificationContent {
                summary: None,
                project_name: None,
                details: &[],
            },
            8,
            &policy,
        )
        .expect("input_required must produce a card");
        assert_eq!(event.priority, Priority::High);
    }

    #[test]
    fn terminal_failed_is_high_one_shot() {
        let policy = NotificationPolicy::default();
        assert!(is_noteworthy(AgentEventKind::Failed, true, &policy));
        assert_eq!(
            priority_for(AgentEventKind::Failed, true, &policy),
            Priority::High
        );
        let event = build_notification(
            &key(AgentRuntime::OpenCode),
            AgentEventKind::Failed,
            true,
            NotificationContent {
                summary: None,
                project_name: None,
                details: &[],
            },
            8,
            &policy,
        )
        .expect("a terminal failure must produce a card");
        assert_eq!(event.priority, Priority::High);
    }

    #[test]
    fn completed_is_medium_one_shot() {
        let policy = NotificationPolicy::default();
        assert!(is_noteworthy(AgentEventKind::Completed, true, &policy));
        assert_eq!(
            priority_for(AgentEventKind::Completed, true, &policy),
            Priority::Medium
        );
        let event = build_notification(
            &key(AgentRuntime::ClaudeCode),
            AgentEventKind::Completed,
            true,
            NotificationContent {
                summary: Some("All tests passed"),
                project_name: None,
                details: &[],
            },
            8,
            &policy,
        )
        .expect("completed must produce a card");
        assert_eq!(event.priority, Priority::Medium);
    }

    #[test]
    fn non_terminal_completed_is_quiet_under_the_default_policy() {
        let policy = NotificationPolicy::default();
        assert!(!policy.informational_notifications);
        assert!(!is_noteworthy(AgentEventKind::Completed, false, &policy));
        assert!(build_notification(
            &key(AgentRuntime::ClaudeCode),
            AgentEventKind::Completed,
            false,
            NotificationContent {
                summary: Some("Ready for your next message"),
                project_name: None,
                details: &[],
            },
            8,
            &policy,
        )
        .is_none());
    }

    #[test]
    fn non_terminal_completed_becomes_a_medium_card_when_informational_is_on() {
        let policy = NotificationPolicy {
            informational_notifications: true,
            completion_notifications: false,
            completion_priority: Priority::High,
            ..NotificationPolicy::default()
        };
        assert!(is_noteworthy(AgentEventKind::Completed, false, &policy));
        assert_eq!(
            priority_for(AgentEventKind::Completed, false, &policy),
            Priority::Medium
        );
        let event = build_notification(
            &key(AgentRuntime::ClaudeCode),
            AgentEventKind::Completed,
            false,
            NotificationContent {
                summary: Some("Ready for your next message"),
                project_name: None,
                details: &[],
            },
            8,
            &policy,
        )
        .expect("a per-turn completed must card once the informational gate is on");
        assert_eq!(event.priority, Priority::Medium);
    }

    #[test]
    fn terminal_completed_is_suppressed_when_completion_notifications_is_off() {
        let policy = NotificationPolicy {
            completion_notifications: false,
            ..NotificationPolicy::default()
        };
        assert!(!is_noteworthy(AgentEventKind::Completed, true, &policy));
        assert!(build_notification(
            &key(AgentRuntime::ClaudeCode),
            AgentEventKind::Completed,
            true,
            NotificationContent {
                summary: Some("All tests passed"),
                project_name: None,
                details: &[],
            },
            8,
            &policy,
        )
        .is_none());

        assert!(is_noteworthy(
            AgentEventKind::PermissionRequested,
            false,
            &policy
        ));
        assert!(is_noteworthy(AgentEventKind::InputRequired, false, &policy));
        assert!(is_noteworthy(AgentEventKind::Failed, true, &policy));
    }

    #[test]
    fn terminal_completed_is_noteworthy_when_completion_notifications_is_on() {
        let policy = NotificationPolicy {
            completion_notifications: true,
            ..NotificationPolicy::default()
        };
        assert!(NotificationPolicy::default().completion_notifications);
        assert!(is_noteworthy(AgentEventKind::Completed, true, &policy));
        let event = build_notification(
            &key(AgentRuntime::ClaudeCode),
            AgentEventKind::Completed,
            true,
            NotificationContent {
                summary: Some("All tests passed"),
                project_name: None,
                details: &[],
            },
            8,
            &policy,
        )
        .expect("a terminal completed must produce a card while the gate is on");
        assert_eq!(event.priority, Priority::Medium);
    }

    #[test]
    fn completion_notifications_off_still_lets_a_per_turn_stop_through_the_informational_gate() {
        let policy = NotificationPolicy {
            completion_notifications: false,
            informational_notifications: true,
            ..NotificationPolicy::default()
        };
        assert!(!is_noteworthy(AgentEventKind::Completed, true, &policy));
        assert!(is_noteworthy(AgentEventKind::Completed, false, &policy));
    }

    #[test]
    fn completed_titles_and_bodies_split_on_terminal() {
        let policy = NotificationPolicy {
            informational_notifications: true,
            ..NotificationPolicy::default()
        };
        let session_end = build_notification(
            &key(AgentRuntime::ClaudeCode),
            AgentEventKind::Completed,
            true,
            NotificationContent {
                summary: None,
                project_name: None,
                details: &[],
            },
            8,
            &policy,
        )
        .unwrap();
        assert_eq!(session_end.payload.title, "Claude Code finished");
        assert_eq!(session_end.payload.body, "Session completed.");

        let per_turn = build_notification(
            &key(AgentRuntime::ClaudeCode),
            AgentEventKind::Completed,
            false,
            NotificationContent {
                summary: None,
                project_name: None,
                details: &[],
            },
            8,
            &policy,
        )
        .unwrap();
        assert_eq!(per_turn.payload.title, "Claude Code finished a turn");
        assert_eq!(
            per_turn.payload.body,
            "Turn completed; the session is still open."
        );
        assert_ne!(per_turn.payload.title, session_end.payload.title);
        assert_ne!(per_turn.payload.body, session_end.payload.body);
    }

    #[test]
    fn informational_is_suppressed_by_default() {
        let policy = NotificationPolicy::default();
        assert!(!is_noteworthy(
            AgentEventKind::Informational,
            false,
            &policy
        ));
        assert!(build_notification(
            &key(AgentRuntime::Codex),
            AgentEventKind::Informational,
            false,
            NotificationContent {
                summary: Some("Running tests"),
                project_name: None,
                details: &[],
            },
            8,
            &policy,
        )
        .is_none());
    }

    #[test]
    fn informational_becomes_medium_one_shot_when_policy_enables_it() {
        let policy = NotificationPolicy {
            informational_notifications: true,
            ..NotificationPolicy::default()
        };
        assert!(is_noteworthy(AgentEventKind::Informational, false, &policy));
        let event = build_notification(
            &key(AgentRuntime::Codex),
            AgentEventKind::Informational,
            false,
            NotificationContent {
                summary: Some("Running tests"),
                project_name: None,
                details: &[],
            },
            8,
            &policy,
        )
        .expect("informational must produce a card once the policy is on");
        assert_eq!(event.priority, Priority::Medium);
    }

    #[test]
    fn non_terminal_failed_is_gated_like_informational_not_high() {
        let default_policy = NotificationPolicy::default();
        assert!(!is_noteworthy(
            AgentEventKind::Failed,
            false,
            &default_policy
        ));
        assert!(build_notification(
            &key(AgentRuntime::Codex),
            AgentEventKind::Failed,
            false,
            NotificationContent {
                summary: Some("shell tool exited 1"),
                project_name: None,
                details: &[],
            },
            8,
            &default_policy,
        )
        .is_none());

        let enabled_policy = NotificationPolicy {
            informational_notifications: true,
            ..NotificationPolicy::default()
        };
        assert!(is_noteworthy(
            AgentEventKind::Failed,
            false,
            &enabled_policy
        ));
        assert_eq!(
            priority_for(AgentEventKind::Failed, false, &enabled_policy),
            Priority::Medium
        );
        let event = build_notification(
            &key(AgentRuntime::Codex),
            AgentEventKind::Failed,
            false,
            NotificationContent {
                summary: Some("shell tool exited 1"),
                project_name: None,
                details: &[],
            },
            8,
            &enabled_policy,
        )
        .expect("a non-terminal failure must produce a card once the shared gate is on");
        assert_eq!(event.priority, Priority::Medium);
    }

    #[test]
    fn agent_signal_carries_wire_tokens_and_hashed_session_not_raw_id() {
        let raw_key = AgentSessionKey::new(AgentRuntime::Codex, "super-secret-native-id").unwrap();
        let event = build_notification(
            &raw_key,
            AgentEventKind::PermissionRequested,
            false,
            NotificationContent {
                summary: Some("Approval needed to run a command"),
                project_name: None,
                details: &[],
            },
            8,
            &NotificationPolicy::default(),
        )
        .unwrap();
        let agent = event.meta.agent.expect("agent meta must be populated");
        assert_eq!(agent.runtime, "codex");
        assert_eq!(agent.kind, "permission_requested");
        assert_eq!(
            agent.summary.as_deref(),
            Some("Approval needed to run a command")
        );
        assert_eq!(agent.session_hash, session_hash_hex(&raw_key));
        assert_ne!(agent.session_hash, raw_key.native_session_id);
        assert!(!agent.session_hash.contains("super-secret-native-id"));
    }

    #[test]
    fn agent_signal_session_hash_is_stable_across_calls() {
        let raw_key = AgentSessionKey::new(AgentRuntime::ClaudeCode, "sess-42").unwrap();
        let a = build_notification(
            &raw_key,
            AgentEventKind::Completed,
            true,
            NotificationContent {
                summary: None,
                project_name: None,
                details: &[],
            },
            8,
            &NotificationPolicy::default(),
        )
        .unwrap();
        let b = build_notification(
            &raw_key,
            AgentEventKind::Completed,
            true,
            NotificationContent {
                summary: None,
                project_name: None,
                details: &[],
            },
            8,
            &NotificationPolicy::default(),
        )
        .unwrap();
        assert_eq!(
            a.meta.agent.unwrap().session_hash,
            b.meta.agent.unwrap().session_hash
        );
    }

    #[test]
    fn missing_summary_falls_back_to_a_kind_specific_body() {
        let event = build_notification(
            &key(AgentRuntime::Kimi),
            AgentEventKind::InputRequired,
            false,
            NotificationContent {
                summary: None,
                project_name: None,
                details: &[],
            },
            8,
            &NotificationPolicy::default(),
        )
        .unwrap();
        assert_eq!(event.payload.body, "Waiting for your input.");
    }

    #[test]
    fn agent_details_carry_verbatim_as_detail_items() {
        let details = vec![
            AgentDetail {
                label: "Tool".to_string(),
                value: "Bash".to_string(),
            },
            AgentDetail {
                label: "Command".to_string(),
                value: "git push".to_string(),
            },
        ];
        let event = build_notification(
            &key(AgentRuntime::Codex),
            AgentEventKind::PermissionRequested,
            false,
            NotificationContent {
                summary: Some("Approval needed"),
                project_name: None,
                details: &details,
            },
            8,
            &NotificationPolicy::default(),
        )
        .unwrap();
        assert_eq!(event.meta.details.len(), 2);
        assert_eq!(event.meta.details[0].label, "Tool");
        assert_eq!(event.meta.details[0].value, "Bash");
        assert_eq!(event.meta.details[1].label, "Command");
        assert_eq!(event.meta.details[1].value, "git push");
    }
}
