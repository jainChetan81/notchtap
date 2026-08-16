//! The `⌃⇧A` Open/Focus Session shortcut. Security invariants:
//!
//! - supported Host bundle IDs and activation strategies are owned by
//!   notchtap code, keyed by the [`Host`] enum — never read off the
//!   wire;
//! - unknown Host metadata is advisory text only, never actionable;
//! - a provider-native deep link is allowed only from the code-owned
//!   [`DEEP_LINK_ALLOWLIST`] and only when it matches the session's
//!   provider;
//! - no `sh -c`, arbitrary executable path, or adapter-provided
//!   argument ever reaches an exec boundary — [`activate`] takes the
//!   [`Host`] enum, not a string, so the signature enforces this;
//! - failure is logged as a quiet status, never a shell fallback.
//!
//! Pure decision ([`decide_focus`]) is separated from the activation
//! call ([`activate`]) so the decision stays unit-testable.

use std::process::Command;

use super::model::{AgentHost, AgentRuntime, AgentState};

/// The small, code-owned set of Host applications notchtap knows how
/// to activate. Every variant must carry a REAL, verified macOS bundle
/// id — never a guess. No real T3 Code bundle id has been verified, so
/// it is deliberately NOT a variant yet:
///
/// ```ignore
/// // T3Code, // TODO: add once a real bundle id is
/// // verified — do not guess one in.
/// ```
///
/// Adding a variant means adding a REAL bundle id plus updating
/// [`Host::from_name`]'s alias list; never infer one from wire data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Terminal,
    ITerm2,
}

impl Host {
    /// The bundle id notchtap itself owns for this Host — never the
    /// wire's `AgentHost::bundle_id`, which is advisory-only and never
    /// read by this module.
    pub const fn bundle_id(self) -> &'static str {
        match self {
            Host::Terminal => "com.apple.Terminal",
            Host::ITerm2 => "com.googlecode.iterm2",
        }
    }

    /// Recognizes a Host from the wire's advisory `name` field, if it
    /// matches a known alias. Any other name (including empty/`None`)
    /// is unknown and therefore not actionable — the caller must not
    /// invent a fallback.
    fn from_name(name: &str) -> Option<Host> {
        match name.trim().to_ascii_lowercase().as_str() {
            "terminal" | "terminal.app" => Some(Host::Terminal),
            "iterm2" | "iterm 2" | "iterm.app" => Some(Host::ITerm2),
            _ => None,
        }
    }

    /// Recognizes a Host from an [`AgentHost`]'s advisory metadata.
    /// Deliberately reads only `name` — the adapter-supplied
    /// `bundle_id` is never trusted for anything, including
    /// recognition; activation always uses [`Host::bundle_id`].
    fn from_agent_host(host: &AgentHost) -> Option<Host> {
        host.name.as_deref().and_then(Host::from_name)
    }
}

/// One code-owned provider-native deep link scheme, gated to a
/// specific [`AgentRuntime`].
#[derive(Debug, Clone, Copy)]
pub struct DeepLinkEntry {
    pub runtime: AgentRuntime,
    pub scheme: &'static str,
}

/// The provider-native deep link allowlist. Empty — no scheme verified
/// against a real runtime yet — but shipped as a structure so the
/// matching rule (runtime match AND scheme match, both against
/// code-owned values) is proven; a verified scheme is a one-line
/// addition here, not new logic.
pub const DEEP_LINK_ALLOWLIST: &[DeepLinkEntry] = &[];

/// Looks up a code-owned deep link scheme for `runtime`, returned only
/// if `requested_scheme` matches the allowlisted entry's own scheme:
/// unlisted schemes and listed-but-wrong-runtime schemes both reject.
/// The returned `&'static str`, not `requested_scheme`, is what an
/// activation call would use — a caller never hands its own copy to an
/// exec boundary.
pub fn deep_link_for(runtime: AgentRuntime, requested_scheme: &str) -> Option<&'static str> {
    deep_link_for_allowlist(DEEP_LINK_ALLOWLIST, runtime, requested_scheme)
}

fn deep_link_for_allowlist(
    allowlist: &[DeepLinkEntry],
    runtime: AgentRuntime,
    requested_scheme: &str,
) -> Option<&'static str> {
    allowlist
        .iter()
        .find(|entry| entry.runtime == runtime && entry.scheme == requested_scheme)
        .map(|entry| entry.scheme)
}

/// Why [`decide_focus`] chose not to activate anything. Purely
/// informational (logged) — never rendered in the overlay, which stays
/// receive-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoFocusReason {
    /// No Agent Sessions are tracked at all.
    EmptyRegistry,
    /// The highest-ranked session exists but its Host metadata is
    /// missing or doesn't match a known [`Host`].
    UnknownHost,
}

/// The pure outcome of [`decide_focus`]. Carrying [`Host`] (an enum)
/// rather than a bundle id string makes "no wire string reaches the
/// exec call" true by construction — the only strings that can reach
/// `Command` are the literals in [`Host::bundle_id`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDecision {
    Activate(Host),
    NoAction(NoFocusReason),
}

/// Selects the highest-ranked Agent Session's Host and decides whether
/// it's actionable. `ordered_states` must already be in Board order
/// (`AgentRegistry::ordered_states`) — the first element is the
/// highest-ranked; this function does no ranking of its own.
pub fn decide_focus(ordered_states: &[AgentState]) -> FocusDecision {
    let Some(top) = ordered_states.first() else {
        return FocusDecision::NoAction(NoFocusReason::EmptyRegistry);
    };
    match top.host.as_ref().and_then(Host::from_agent_host) {
        Some(host) => FocusDecision::Activate(host),
        None => FocusDecision::NoAction(NoFocusReason::UnknownHost),
    }
}

/// Activates `host` via `open -b <bundle-id>` — a fixed two-element
/// arg array, never a shell string. `host.bundle_id()` is the only
/// string this ever passes to `Command`, and it can only be a
/// [`Host::bundle_id`] literal because `host` is the enum. Failure is
/// logged and swallowed — never converted into a shell fallback.
pub fn activate(host: Host) {
    match Command::new("open").args(["-b", host.bundle_id()]).status() {
        Ok(status) if status.success() => {
            tracing::info!(bundle_id = host.bundle_id(), "focus: activated host");
        }
        Ok(status) => {
            tracing::warn!(
                bundle_id = host.bundle_id(),
                code = ?status.code(),
                "focus: `open -b` exited non-zero"
            );
        }
        Err(error) => {
            tracing::warn!(
                bundle_id = host.bundle_id(),
                %error,
                "focus: failed to spawn `open -b`"
            );
        }
    }
}

/// Runs [`decide_focus`] against `ordered_states` and, on
/// [`FocusDecision::Activate`], calls [`activate`]. `NoAction` is a
/// quiet no-op: logged at debug level, no shell fallback, no overlay
/// involvement (the overlay stays receive-only).
pub fn focus_highest_ranked(ordered_states: &[AgentState]) {
    match decide_focus(ordered_states) {
        FocusDecision::Activate(host) => activate(host),
        FocusDecision::NoAction(reason) => {
            tracing::debug!(?reason, "focus: no actionable session");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::agents::model::{AgentSessionKey, AgentSessionState};

    fn state_with_host(runtime: AgentRuntime, host: Option<AgentHost>) -> AgentState {
        let now = Instant::now();
        AgentState {
            key: AgentSessionKey::new(runtime, "session-1").unwrap(),
            state: AgentSessionState::Working,
            capabilities: Vec::new(),
            summary: None,
            details: Vec::new(),
            project: None,
            host,
            subagent: None,
            history: Vec::new(),
            first_seen_at: now,
            state_entered_at: now,
            last_seen_at_ms: 0,
            elapsed_ms: 0,
            retention_remaining_ms: None,
        }
    }

    // --- Host recognition ------------------------------------------------

    #[test]
    fn known_host_names_recognized_case_insensitively() {
        assert_eq!(Host::from_name("Terminal"), Some(Host::Terminal));
        assert_eq!(Host::from_name("terminal.app"), Some(Host::Terminal));
        assert_eq!(Host::from_name("iTerm2"), Some(Host::ITerm2));
        assert_eq!(Host::from_name("ITERM 2"), Some(Host::ITerm2));
    }

    #[test]
    fn unknown_host_name_is_not_recognized() {
        assert_eq!(Host::from_name("T3 Code"), None);
        assert_eq!(Host::from_name(""), None);
        assert_eq!(Host::from_name("Visual Studio Code"), None);
    }

    #[test]
    fn bundle_ids_are_the_real_verified_values() {
        assert_eq!(Host::Terminal.bundle_id(), "com.apple.Terminal");
        assert_eq!(Host::ITerm2.bundle_id(), "com.googlecode.iterm2");
    }

    // --- decide_focus ------------------------------------------------------

    #[test]
    fn empty_registry_is_no_action() {
        let decision = decide_focus(&[]);
        assert_eq!(
            decision,
            FocusDecision::NoAction(NoFocusReason::EmptyRegistry)
        );
    }

    #[test]
    fn unknown_host_metadata_is_no_action() {
        let states = vec![state_with_host(
            AgentRuntime::Codex,
            Some(AgentHost {
                name: Some("Visual Studio Code".to_string()),
                bundle_id: Some("com.microsoft.VSCode".to_string()),
            }),
        )];
        assert_eq!(
            decide_focus(&states),
            FocusDecision::NoAction(NoFocusReason::UnknownHost)
        );
    }

    #[test]
    fn missing_host_metadata_is_no_action() {
        let states = vec![state_with_host(AgentRuntime::ClaudeCode, None)];
        assert_eq!(
            decide_focus(&states),
            FocusDecision::NoAction(NoFocusReason::UnknownHost)
        );
    }

    #[test]
    fn highest_ranked_known_host_is_activated() {
        // `decide_focus` trusts its input is already Board-ordered — this
        // proves it reads only the FIRST element as "highest ranked",
        // never re-sorting or scanning past it.
        let states = vec![
            state_with_host(
                AgentRuntime::ClaudeCode,
                Some(AgentHost {
                    name: Some("Terminal".to_string()),
                    bundle_id: Some("com.apple.Terminal".to_string()),
                }),
            ),
            state_with_host(
                AgentRuntime::Codex,
                Some(AgentHost {
                    name: Some("iTerm2".to_string()),
                    bundle_id: Some("com.googlecode.iterm2".to_string()),
                }),
            ),
        ];
        assert_eq!(
            decide_focus(&states),
            FocusDecision::Activate(Host::Terminal)
        );
    }

    #[test]
    fn wire_bundle_id_is_never_trusted_for_recognition_or_activation() {
        // The wire claims a bogus bundle id for a recognized name; the
        // decision must still resolve to the enum (and thus notchtap's
        // OWN bundle id constant), never the wire's string.
        let states = vec![state_with_host(
            AgentRuntime::Kimi,
            Some(AgentHost {
                name: Some("Terminal".to_string()),
                bundle_id: Some("com.evil.definitely-not-terminal".to_string()),
            }),
        )];
        let FocusDecision::Activate(host) = decide_focus(&states) else {
            panic!("expected Activate");
        };
        assert_eq!(host.bundle_id(), "com.apple.Terminal");
    }

    // --- deep link allowlist ------------------------------------------------

    const TEST_ALLOWLIST: &[DeepLinkEntry] = &[DeepLinkEntry {
        runtime: AgentRuntime::ClaudeCode,
        scheme: "claude-code",
    }];

    #[test]
    fn shipped_allowlist_is_empty() {
        assert!(DEEP_LINK_ALLOWLIST.is_empty());
    }

    #[test]
    fn listed_scheme_matching_runtime_is_returned() {
        assert_eq!(
            deep_link_for_allowlist(TEST_ALLOWLIST, AgentRuntime::ClaudeCode, "claude-code"),
            Some("claude-code")
        );
    }

    #[test]
    fn unlisted_scheme_is_rejected() {
        assert_eq!(
            deep_link_for_allowlist(
                TEST_ALLOWLIST,
                AgentRuntime::ClaudeCode,
                "not-a-real-scheme"
            ),
            None
        );
    }

    #[test]
    fn listed_scheme_with_mismatched_runtime_is_rejected() {
        assert_eq!(
            deep_link_for_allowlist(TEST_ALLOWLIST, AgentRuntime::Codex, "claude-code"),
            None
        );
    }

    #[test]
    fn empty_production_allowlist_rejects_everything() {
        assert_eq!(deep_link_for(AgentRuntime::ClaudeCode, "claude-code"), None);
    }

    // --- focus_highest_ranked (integration of the two pure halves) --------

    #[test]
    fn focus_highest_ranked_is_a_quiet_no_op_on_empty_registry() {
        // No panic, no I/O beyond a debug log line — this just proves the
        // wiring doesn't call `activate` when `decide_focus` says no.
        focus_highest_ranked(&[]);
    }

    #[test]
    fn decide_focus_ignores_lower_ranked_sessions_entirely() {
        let states = vec![
            state_with_host(AgentRuntime::ClaudeCode, None),
            state_with_host(
                AgentRuntime::Codex,
                Some(AgentHost {
                    name: Some("Terminal".to_string()),
                    bundle_id: None,
                }),
            ),
        ];
        // First (highest-ranked) has no recognizable Host, so this is
        // UnknownHost even though a lower-ranked entry WOULD resolve.
        assert_eq!(
            decide_focus(&states),
            FocusDecision::NoAction(NoFocusReason::UnknownHost)
        );
    }
}
