//! Open/Focus Session support. Activation uses code-owned bundle IDs, never adapter-provided IDs.

use std::process::Command;

use super::model::{AgentHost, AgentRuntime, AgentState};

/// Every variant must carry a REAL, verified macOS bundle id — never a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Terminal,
    ITerm2,
}

impl Host {
    /// The bundle id notchtap itself owns for this Host — never the wire's `AgentHost::bundle_id`,
    /// which is advisory-only and never read by this module.
    pub const fn bundle_id(self) -> &'static str {
        match self {
            Host::Terminal => "com.apple.Terminal",
            Host::ITerm2 => "com.googlecode.iterm2",
        }
    }

    /// Any other name (including empty/`None`) is unknown and therefore not actionable — the caller
    /// must not invent a fallback.
    fn from_name(name: &str) -> Option<Host> {
        match name.trim().to_ascii_lowercase().as_str() {
            "terminal" | "terminal.app" => Some(Host::Terminal),
            "iterm2" | "iterm 2" | "iterm.app" => Some(Host::ITerm2),
            _ => None,
        }
    }

    /// Deliberately reads only `name` — the adapter-supplied `bundle_id` is never trusted for
    /// anything, including recognition; activation always uses [`Host::bundle_id`].
    fn from_agent_host(host: &AgentHost) -> Option<Host> {
        host.name.as_deref().and_then(Host::from_name)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DeepLinkEntry {
    pub runtime: AgentRuntime,
    pub scheme: &'static str,
}

/// The provider-native deep link allowlist.
pub const DEEP_LINK_ALLOWLIST: &[DeepLinkEntry] = &[];

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

/// Purely informational (logged) — never rendered in the overlay, which stays receive-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoFocusReason {
    EmptyRegistry,
    UnknownHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDecision {
    Activate(Host),
    NoAction(NoFocusReason),
}

/// Uses only the first state; callers must supply Agent Board ordering.
pub fn decide_focus(ordered_states: &[AgentState]) -> FocusDecision {
    let Some(top) = ordered_states.first() else {
        return FocusDecision::NoAction(NoFocusReason::EmptyRegistry);
    };
    match top.host.as_ref().and_then(Host::from_agent_host) {
        Some(host) => FocusDecision::Activate(host),
        None => FocusDecision::NoAction(NoFocusReason::UnknownHost),
    }
}

/// Activates `host` via `open -b <bundle-id>` — a fixed two-element arg array, never a shell
/// string. Failure is logged and swallowed — never converted into a shell fallback.
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
        assert_eq!(
            decide_focus(&states),
            FocusDecision::NoAction(NoFocusReason::UnknownHost)
        );
    }
}
