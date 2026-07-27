//! Plan 135 (v7 ticket 3 of 13, spec §5): maps a noteworthy Agent Event
//! into the existing-domain [`Event`], the ONE seam that lets an Agent
//! Session's permission/input/failure/completion moments enter the
//! Notification Slot and obey every existing Queue/Slot rule (Priority,
//! Rotation Order, tier caps, Paused, Promotion) without a side path.
//!
//! This module is pure: [`build_notification`] takes already-registry-
//! accepted facts (session key, kind, terminal flag, sanitized summary)
//! plus a [`NotificationPolicy`] and returns `Option<Event>` — no clock
//! read, no registry access, no HTTP. The caller (`http.rs`'s
//! `agent_events_handler`) is the one that calls
//! `AgentRegistry::apply_event` first and `Engine::accept` after, and
//! that owns the "registry update is accepted even when the Notification
//! is rejected for a full queue tier" independence spec §5 requires (a
//! `QueueError::QueueFull` from `Engine::accept` must never unwind back
//! into the registry mutation that already happened).
//!
//! ## Resolving the non-terminal-`Failed` question (spec §2.1 vs §5's table)
//!
//! Spec §5's table lists `Failed -> High, one-shot` with no terminal/
//! non-terminal split written into the row itself. But §2.1 says, in the
//! very same breath as it defines the registry's own state machine: "a
//! non-terminal tool failure is an informational/failure Notification
//! while the session remains `Working`" — i.e. it explicitly reclassifies
//! a non-terminal `Failed` as living under the *Informational* row
//! (Medium, off by default), not the `Failed` row's High/one-shot. This
//! module reads §2.1 as authoritative over §5's table for that one
//! sub-case (§2.1 is the more specific rule, and it uses the word
//! "informational" on purpose): [`is_noteworthy`]/[`priority_for`] both
//! branch on `kind == Failed && terminal` for the High/always-on
//! treatment, and fall through non-terminal `Failed` into exactly the
//! same `policy.informational_notifications` gate and `Priority::Medium`
//! that a wire `Informational` kind gets. `registry::next_state` already
//! encodes the identical split for the registry's own state machine (see
//! its own doc) — this module's tests
//! (`non_terminal_failed_is_gated_like_informational_not_high`) pin that
//! the notification layer agrees with it.

use uuid::Uuid;

use crate::event::{
    AgentSignal, Event, EventMeta, EventPayload, EventSignal as WireSignal, EventType, Priority,
    RotationSpec, SourceKind,
};

use super::adapter::{kind_wire_label, runtime_wire_label};
use super::model::{session_hash_hex, AgentEventKind, AgentRuntime, AgentSessionKey};

/// Priority/gating knobs for the registry→Notification mapping (spec §5's
/// table + spec §7's future `[agents]` config block). Plan 137 wires these
/// to real config (`agents.informational_notifications`,
/// `agents.permission_priority`, etc.) — until then every call site
/// (`http.rs`'s `agent_events_handler`, `settings.rs`'s agent preview arm)
/// uses [`NotificationPolicy::default`], which hardcodes the spec's own
/// defaults. `Informational`'s priority is deliberately NOT a field here:
/// spec §5's table pins it at a fixed Medium ("Informational | Medium |
/// off by default"), unlike the other four kinds, which each get their
/// own configurable priority in §7's toml block — there is no
/// `informational_priority` key to wire in plan 137, so this struct
/// doesn't invent one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotificationPolicy {
    /// Spec §7's `agents.informational_notifications` (default `false`).
    /// Gates BOTH the wire `Informational` kind AND a non-terminal
    /// `Failed` — see this module's top doc for why the two share one
    /// gate.
    pub informational_notifications: bool,
    pub permission_priority: Priority,
    pub input_priority: Priority,
    pub failure_priority: Priority,
    pub completion_priority: Priority,
}

impl Default for NotificationPolicy {
    /// Spec §7's own defaults, verbatim: `permission_priority =
    /// input_priority = failure_priority = "high"`, `completion_priority
    /// = "medium"`, `informational_notifications = false`.
    fn default() -> Self {
        Self {
            informational_notifications: false,
            permission_priority: Priority::High,
            input_priority: Priority::High,
            failure_priority: Priority::High,
            completion_priority: Priority::Medium,
        }
    }
}

/// Whether `kind` (+ `terminal`) is ever eligible to become a
/// Notification, independent of queue capacity (spec §5's table, with the
/// §2.1 non-terminal-`Failed` carve-out — see this module's top doc).
pub fn is_noteworthy(kind: AgentEventKind, terminal: bool, policy: &NotificationPolicy) -> bool {
    match kind {
        AgentEventKind::PermissionRequested
        | AgentEventKind::InputRequired
        | AgentEventKind::Completed => true,
        AgentEventKind::Failed if terminal => true,
        AgentEventKind::Failed | AgentEventKind::Informational => {
            policy.informational_notifications
        }
    }
}

/// The Priority a noteworthy `kind` maps to (spec §5's table). Only
/// meaningful when [`is_noteworthy`] is true for the same `(kind,
/// terminal)` pair — callers must check that first, since this function
/// still returns a value (Medium) for a policy-suppressed kind.
pub fn priority_for(kind: AgentEventKind, terminal: bool, policy: &NotificationPolicy) -> Priority {
    match kind {
        AgentEventKind::PermissionRequested => policy.permission_priority,
        AgentEventKind::InputRequired => policy.input_priority,
        AgentEventKind::Completed => policy.completion_priority,
        AgentEventKind::Failed if terminal => policy.failure_priority,
        // Non-terminal Failed reads as Informational (this module's top
        // doc) — spec §5's table pins Informational at a fixed Medium,
        // not a configurable priority.
        AgentEventKind::Failed | AgentEventKind::Informational => Priority::Medium,
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
/// per kind, never derived from `summary` (spec's own house style:
/// notification text is templated, not sniffed from provider payloads).
fn title_for(runtime: AgentRuntime, kind: AgentEventKind, terminal: bool) -> String {
    let name = runtime_display_name(runtime);
    match kind {
        AgentEventKind::PermissionRequested => format!("{name} needs permission"),
        AgentEventKind::InputRequired => format!("{name} needs input"),
        AgentEventKind::Completed => format!("{name} finished"),
        AgentEventKind::Failed if terminal => format!("{name} failed"),
        AgentEventKind::Failed => format!("{name} hit a tool error"),
        AgentEventKind::Informational => format!("{name} update"),
    }
}

/// Fallback body when the wire event carried no `summary` (spec §3.1:
/// `summary` is optional) — a Notification's `body` is a required,
/// non-optional `String` (`event.rs::EventPayload`), so this module must
/// always have something to show.
fn default_body_for(kind: AgentEventKind, terminal: bool) -> String {
    match kind {
        AgentEventKind::PermissionRequested => "Approval needed to continue.".to_string(),
        AgentEventKind::InputRequired => "Waiting for your input.".to_string(),
        AgentEventKind::Completed => "Session completed.".to_string(),
        AgentEventKind::Failed if terminal => "The session ended with an error.".to_string(),
        AgentEventKind::Failed => "A tool call failed; the session is still working.".to_string(),
        AgentEventKind::Informational => "Session update.".to_string(),
    }
}

/// The one constructor for an Agent-originated Notification `Event`
/// (spec §5). Returns `None` when this `(kind, terminal)` pair isn't
/// noteworthy under `policy` — Starting/Working/tool/subagent progress
/// (wire `Informational`, `terminal: false`, `informational_notifications`
/// off) and a non-terminal `Failed` under that same gate both resolve to
/// `None` here, same as they never update anything past the registry.
///
/// `ttl_secs` is the caller's own one-shot rotation window — plan 137
/// wired `http.rs`'s call site to the real `agent_ttl_secs` config field
/// (renamed from `cmux_ttl_secs`, itself a migration target for that
/// same v6.1 flat field), this module itself stays agnostic to where the
/// value came from.
pub fn build_notification(
    session_key: &AgentSessionKey,
    kind: AgentEventKind,
    terminal: bool,
    summary: Option<&str>,
    ttl_secs: u64,
    policy: &NotificationPolicy,
) -> Option<Event> {
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
            ..EventMeta::default()
        },
        // v7 has no dedicated icon/animation signal of its own yet — same
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

    // --- spec §5 table: every AgentEventKind → expected priority/one-shot
    // or no-card, under the default policy (informational off). ---

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
            Some("Approval needed"),
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
            None,
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
            None,
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
            Some("All tests passed"),
            8,
            &policy,
        )
        .expect("completed must produce a card");
        assert_eq!(event.priority, Priority::Medium);
    }

    #[test]
    fn non_terminal_completed_is_also_medium_one_shot() {
        // Operator decision 2026-07-26 (spec §2.1/§5): a per-turn Stop
        // (kind Completed, terminal:false) still posts the same Medium
        // one-shot "completed" card as a terminal session end — the
        // notification layer doesn't gate on `terminal` for this kind.
        let policy = NotificationPolicy::default();
        assert!(is_noteworthy(AgentEventKind::Completed, false, &policy));
        assert_eq!(
            priority_for(AgentEventKind::Completed, false, &policy),
            Priority::Medium
        );
        let event = build_notification(
            &key(AgentRuntime::ClaudeCode),
            AgentEventKind::Completed,
            false,
            Some("Ready for your next message"),
            8,
            &policy,
        )
        .expect("a non-terminal (per-turn) completed event must still produce a card");
        assert_eq!(event.priority, Priority::Medium);
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
            Some("Running tests"),
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
            Some("Running tests"),
            8,
            &policy,
        )
        .expect("informational must produce a card once the policy is on");
        assert_eq!(event.priority, Priority::Medium);
    }

    // --- the non-terminal-Failed resolution (this module's top doc) ---

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
            Some("shell tool exited 1"),
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
            Some("shell tool exited 1"),
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
        // `Informational` with `terminal: false` (spec §4.2/§4.3's hook
        // lists have no dedicated "progress" kind) — same suppression
        // path as `informational_is_suppressed_by_default`, pinned again
        // here under this ticket's own checkbox wording.
        let policy = NotificationPolicy::default();
        assert!(build_notification(
            &key(AgentRuntime::OpenCode),
            AgentEventKind::Informational,
            false,
            Some("Running `pnpm test`"),
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
            Some("Approval needed to run a command"),
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
            None,
            8,
            &NotificationPolicy::default(),
        )
        .unwrap();
        let b = build_notification(
            &raw_key,
            AgentEventKind::Completed,
            true,
            None,
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
            None,
            8,
            &NotificationPolicy::default(),
        )
        .unwrap();
        assert_eq!(event.payload.body, "Waiting for your input.");
    }
}
