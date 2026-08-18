//! Maps a noteworthy Agent Event into the domain [`Event`] — the ONE
//! seam letting a Session's permission/input/failure/completion moments
//! enter the Notification Slot under every Queue/Slot rule. Pure: no
//! clock read, no registry access, no HTTP. The caller (`http.rs`'s
//! `agent_events_handler`) applies the registry mutation first, and a
//! `QueueError::QueueFull` from `Engine::accept` must never unwind it.

use uuid::Uuid;

use crate::event::{
    AgentSignal, DetailItem, Event, EventMeta, EventPayload, EventSignal as WireSignal, EventType,
    Priority, RotationSpec, SourceKind,
};

use super::adapter::{kind_wire_label, runtime_wire_label};
use super::model::{session_hash_hex, AgentDetail, AgentEventKind, AgentRuntime, AgentSessionKey};

/// Priority/gating knobs for the registry→Notification mapping. Every
/// call site (`http.rs`'s `agent_events_handler`, `settings.rs`'s agent
/// preview arm) uses [`NotificationPolicy::default`]. `Informational`'s
/// priority is deliberately NOT a field here: it is pinned at a fixed
/// Medium, unlike the other four kinds, which each get their own
/// configurable priority.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotificationPolicy {
    /// `agents.informational_notifications` (default `false`). Gates
    /// BOTH the wire `Informational` kind AND a non-terminal `Failed` —
    /// see [`is_noteworthy`] for why the two share one gate.
    pub informational_notifications: bool,
    /// `agents.completion_notifications` (default `true`). Gates a
    /// TERMINAL `Completed` — a real session end — only. A non-terminal
    /// `Completed` (the per-turn stop every runtime fires after each
    /// response) is not covered by this key at all: it rides
    /// `informational_notifications` instead, so this can default ON
    /// without carding the operator once per turn.
    pub completion_notifications: bool,
    pub permission_priority: Priority,
    pub input_priority: Priority,
    pub failure_priority: Priority,
    pub completion_priority: Priority,
}

impl Default for NotificationPolicy {
    /// `permission_priority = input_priority = failure_priority =
    /// "high"`, `completion_priority = "medium"`,
    /// `informational_notifications = false`,
    /// `completion_notifications = true`.
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

/// Whether `kind` (+ `terminal`) is ever eligible to become a
/// Notification, independent of queue capacity.
///
/// `Failed` and `Completed` share ONE terminal split. A terminal failure
/// or completion is an outcome: `Failed` is always-on High, `Completed`
/// reads `policy.completion_notifications` (default ON). A NON-terminal
/// one is progress — a tool failure while the session keeps working, or
/// the per-turn stop every runtime fires after each response — so both
/// fall through into `policy.informational_notifications` (default OFF)
/// at Informational's fixed `Priority::Medium`. `registry::next_state`
/// encodes the identical split for the registry's own state machine, so
/// the Agent Board still shows every turn boundary; only the card is
/// suppressed.
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

/// The Priority a noteworthy `kind` maps to. Only meaningful when
/// [`is_noteworthy`] is true for the same `(kind, terminal)` pair —
/// callers must check that first, since this function still returns a
/// value (Medium) for a policy-suppressed kind.
pub fn priority_for(kind: AgentEventKind, terminal: bool, policy: &NotificationPolicy) -> Priority {
    match kind {
        AgentEventKind::PermissionRequested => policy.permission_priority,
        AgentEventKind::InputRequired => policy.input_priority,
        AgentEventKind::Completed if terminal => policy.completion_priority,
        AgentEventKind::Failed if terminal => policy.failure_priority,
        // Non-terminal Failed and non-terminal Completed both read as
        // Informational ([`is_noteworthy`]), which is pinned at a fixed
        // Medium rather than a configurable priority.
        AgentEventKind::Completed | AgentEventKind::Failed | AgentEventKind::Informational => {
            Priority::Medium
        }
    }
}

/// Human-facing runtime name for the generated title (Settings/card
/// display concern — NOT the wire token; see [`runtime_wire_label`] for
/// that).
fn runtime_display_name(runtime: AgentRuntime) -> &'static str {
    match runtime {
        AgentRuntime::ClaudeCode => "Claude Code",
        AgentRuntime::Codex => "Codex",
        AgentRuntime::Kimi => "Kimi",
        AgentRuntime::OpenCode => "OpenCode",
    }
}

/// Short, kind-specific title — `runtime_display_name` plus a fixed verb
/// per kind, never derived from `summary`: notification text is
/// templated, never sniffed from provider payloads.
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

/// Fallback body when the wire event carried no `summary` (it is
/// optional) — a Notification's `body` is a required, non-optional
/// `String` (`event.rs::EventPayload`), so this module must always have
/// something to show.
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

/// Everything [`build_notification`] needs about what an Agent Event's
/// card should SAY — as opposed to its routing/policy facts
/// (`session_key`/`kind`/`terminal`/`ttl_secs`/`policy`, which stay their
/// own positional params). Bundling `summary`/`project_name`/`details`
/// keeps `build_notification` under clippy's `too_many_arguments`.
pub struct NotificationContent<'a> {
    pub summary: Option<&'a str>,
    /// The project NAME (`AgentProject.name`), never the cwd — see
    /// [`build_notification`]'s own doc.
    pub project_name: Option<&'a str>,
    pub details: &'a [AgentDetail],
}

/// The one constructor for an Agent-originated Notification `Event`.
/// Returns `None` when this `(kind, terminal)` pair isn't
/// noteworthy under `policy` — Starting/Working/tool/subagent progress
/// (wire `Informational`, `terminal: false`, `informational_notifications`
/// off), a non-terminal `Failed`, and a non-terminal `Completed` (the
/// per-turn stop) all resolve to `None` here under that same gate, same
/// as they never update anything past the registry. A TERMINAL
/// `Completed` — a real session end — resolves to `None` only once
/// `completion_notifications` is off (that gate defaults on, so this is
/// opt-in silence, not the default).
///
/// `ttl_secs` is the caller's own one-shot rotation window (`http.rs`
/// passes the `agent_ttl_secs` config field); this module stays agnostic
/// to where the value came from.
///
/// `project_name`/`details` are the already-sanitized/capped
/// `AgentProject.name`/`Vec<AgentDetail>` the registry accepted off the
/// same wire event — NEVER the cwd; only the name is
/// display-appropriate. They ride onto `EventMeta.subtitle`/`.details`,
/// the same two fields the manual `/notify` rich-relay path populates,
/// so an agent card renders identically to a manual one of the same
/// shape. Absent project or empty details leave those fields at
/// `EventMeta::default()`'s None/empty.
///
/// Bundled into [`NotificationContent`] (rather than three more
/// positional params) to stay under clippy's `too_many_arguments` —
/// `session_key`/`kind`/`terminal`/`ttl_secs`/`policy` are the event's
/// own routing/policy facts, `NotificationContent` is everything that
/// only affects what the card SAYS.
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
        // Agent cards have no dedicated icon/animation signal — same
        // `Generic` choice `/notify`'s manual path makes (`http.rs`'s
        // `NotifyRequest::signal` default), since `EventSignal` is a
        // football-live-match-centric enum (goal/card/kickoff/...) with no
        // agent-shaped variant to reach for.
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

    // --- every AgentEventKind → expected priority/one-shot or no-card,
    // under the default policy (informational off). ---

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

    // --- the `Completed` terminal split. Four (terminal, policy)
    // combinations, pinned exhaustively. ---

    #[test]
    fn non_terminal_completed_is_quiet_under_the_default_policy() {
        // A per-turn Stop (kind Completed, terminal:false) is progress,
        // not an outcome: it rides `informational_notifications` (OFF by
        // default), so the operator is NOT carded once per turn.
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
        // It shares Informational's gate AND Informational's fixed
        // Medium priority — never `completion_priority`, and never
        // `completion_notifications`.
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
    fn terminal_completed_ignores_the_informational_gate() {
        // The mirror of the test above: a real session end is NOT gated
        // behind `informational_notifications`, only behind
        // `completion_notifications`.
        let policy = NotificationPolicy {
            informational_notifications: false,
            completion_notifications: true,
            ..NotificationPolicy::default()
        };
        assert!(is_noteworthy(AgentEventKind::Completed, true, &policy));
    }

    // --- `completion_notifications`: the session-end off switch.
    // Default ON, and it reaches the terminal shape only. ---

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

        // The other kinds are untouched by this gate.
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
        // The two gates are independent: turning the session-end card off
        // must not also silence a per-turn stop the operator explicitly
        // opted into via `informational_notifications`.
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
        // The whole point of the split: a per-turn stop must never read
        // as the session ending.
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

    // --- the non-terminal-Failed resolution (see `is_noteworthy`) ---

    #[test]
    fn non_terminal_failed_is_gated_like_informational_not_high() {
        let default_policy = NotificationPolicy::default();
        // Off by default, same as Informational — NOT an automatic High
        // card just because the wire `kind` says `Failed`.
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

        // Enabling the SAME `informational_notifications` toggle that
        // gates Informational also lets a non-terminal Failed through, at
        // the same Medium priority — proving it shares Informational's
        // gate/priority rather than Failed's High/always-on row.
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

    // --- Starting/Working/tool/subagent progress never create cards ---

    #[test]
    fn non_terminal_informational_progress_creates_no_card() {
        // Starting/Working/tool/subagent progress all arrive as wire
        // `Informational` with `terminal: false` — there is no dedicated
        // "progress" kind — so they take the same suppression path as
        // `informational_is_suppressed_by_default`.
        let policy = NotificationPolicy::default();
        assert!(build_notification(
            &key(AgentRuntime::OpenCode),
            AgentEventKind::Informational,
            false,
            NotificationContent {
                summary: Some("Running `pnpm test`"),
                project_name: None,
                details: &[],
            },
            8,
            &policy,
        )
        .is_none());
    }

    // --- AgentSignal shape: wire tokens, hashed (never raw) session id ---

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

    // --- notification parity — project name -> subtitle,
    // AgentDetail -> DetailItem, matching the manual `/notify` rich-relay
    // shape ---

    #[test]
    fn project_name_becomes_subtitle() {
        let event = build_notification(
            &key(AgentRuntime::ClaudeCode),
            AgentEventKind::PermissionRequested,
            false,
            NotificationContent {
                summary: Some("Approval needed"),
                project_name: Some("mac-notification-nudge"),
                details: &[],
            },
            8,
            &NotificationPolicy::default(),
        )
        .unwrap();
        // The project NAME, not the cwd — build_notification never sees a
        // cwd at all (the caller only passes the name through).
        assert_eq!(
            event.meta.subtitle.as_deref(),
            Some("mac-notification-nudge")
        );
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

    #[test]
    fn absent_project_and_details_default_to_none_and_empty() {
        let event = build_notification(
            &key(AgentRuntime::OpenCode),
            AgentEventKind::PermissionRequested,
            false,
            NotificationContent {
                summary: Some("Approval needed"),
                project_name: None,
                details: &[],
            },
            8,
            &NotificationPolicy::default(),
        )
        .unwrap();
        assert_eq!(event.meta.subtitle, None);
        assert!(event.meta.details.is_empty());
    }
}
